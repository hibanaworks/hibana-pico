/*
 * Infineon CYW43439 Wi-Fi device model for Raspberry Pi Pico 2 W
 *
 * This is a bounded SPI-facing datagram model for Hibana/Pico swarm tests.
 * It models the board-visible CYW43439 as an SSI peripheral and preserves the
 * important guest contract: the RP2350 talks to the Wi-Fi chip over SPI, and
 * the chip owns a bounded radio-side receive queue. It does not emulate the
 * proprietary fullmac firmware, RF, WPA, or Bluetooth controllers.
 *
 * Copyright (c) 2026
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#include "qemu/osdep.h"
#include "qapi/error.h"
#include "hw/core/qdev-properties.h"
#include "hw/misc/cyw43439_wifi.h"
#include "migration/vmstate.h"
#include "qemu/log.h"
#include "qemu/main-loop.h"
#include "qemu/module.h"
#include "qemu/sockets.h"

#define CYW_CMD_SELECT_NODE 0x10
#define CYW_CMD_STATUS      0x20
#define CYW_CMD_TX_FRAME    0x30
#define CYW_CMD_RX_FRAME    0x40
#define CYW_CMD_RESET       0x50
#define CYW_CMD_CLEAR_STATUS 0x51
#define CYW_CMD_POWER_ON    0x52
#define CYW_CMD_RESET_ASSERT 0x53
#define CYW_CMD_RESET_RELEASE 0x54
#define CYW_CMD_PEEK_LABEL  0x60
#define CYW_CMD_NODE_ROLE   0x70
#define CYW_CMD_NODE_ID     0x71
#define CYW_CMD_RADIO_MODE  0x72
#define CYW_CMD_NODE_COUNT  0x73
#define CYW_CMD_FW_BEGIN    0x80
#define CYW_CMD_FW_CHUNK    0x81
#define CYW_CMD_FW_COMMIT   0x82
#define CYW_CMD_CLM_BEGIN   0x83
#define CYW_CMD_CLM_CHUNK   0x84
#define CYW_CMD_CLM_COMMIT  0x85
#define CYW_CMD_NVRAM_APPLY 0x86
#define CYW_CMD_BOOT        0x87
#define CYW_CMD_FW_STATE    0x88
#define CYW_CMD_IDENT       0x9f

#define CYW_STATUS_RX_READY BIT(0)
#define CYW_STATUS_TX_READY BIT(1)
#define CYW_STATUS_JOINED   BIT(2)
#define CYW_STATUS_OVERFLOW BIT(4)

#define CYW_FW_STATE_FW_STARTED   BIT(0)
#define CYW_FW_STATE_FW_COMMITTED BIT(1)
#define CYW_FW_STATE_CLM_STARTED  BIT(2)
#define CYW_FW_STATE_CLM_COMMITTED BIT(3)
#define CYW_FW_STATE_NVRAM_APPLIED BIT(4)
#define CYW_FW_STATE_READY        BIT(5)
#define CYW_FW_STATE_ERROR        BIT(7)

#define CYW_FNV1A32_OFFSET 0x811c9dc5U
#define CYW_FNV1A32_PRIME  0x01000193U

enum {
    CYW_MODE_COMMAND = 0,
    CYW_MODE_SELECT_NODE,
    CYW_MODE_TX_DST,
    CYW_MODE_TX_LEN,
    CYW_MODE_TX_PAYLOAD,
    CYW_MODE_RX_PAYLOAD,
    CYW_MODE_LOAD_BEGIN,
    CYW_MODE_LOAD_CHUNK_HEADER,
    CYW_MODE_LOAD_CHUNK_PAYLOAD,
    CYW_MODE_NVRAM_APPLY,
};

enum {
    CYW_LOAD_NONE = 0,
    CYW_LOAD_FW,
    CYW_LOAD_CLM,
};

static bool cyw43439_queue_full(const CYW43439WifiState *s)
{
    return s->queue_len == CYW43439_WIFI_QUEUE_DEPTH;
}

static bool cyw43439_radio_enabled(const CYW43439WifiState *s)
{
    return s->radio_fd >= 0;
}

static bool cyw43439_radio_mesh_enabled(const CYW43439WifiState *s)
{
    return s->radio_port_base != 0;
}

static uint32_t cyw43439_read_le32(const uint8_t bytes[4])
{
    return (uint32_t)bytes[0] |
           ((uint32_t)bytes[1] << 8) |
           ((uint32_t)bytes[2] << 16) |
           ((uint32_t)bytes[3] << 24);
}

static uint16_t cyw43439_read_be16(const uint8_t bytes[2])
{
    return ((uint16_t)bytes[0] << 8) | (uint16_t)bytes[1];
}

static uint32_t cyw43439_fnv1a32_update(uint32_t hash, uint8_t byte)
{
    hash ^= byte;
    return hash * CYW_FNV1A32_PRIME;
}

static void cyw43439_load_error(CYW43439WifiState *s, const char *reason)
{
    qemu_log_mask(LOG_GUEST_ERROR, "cyw43439: firmware load failed: %s\n",
                  reason);
    s->fw_error = true;
    s->fw_ready = false;
    s->mode = CYW_MODE_COMMAND;
}

static void cyw43439_load_begin(CYW43439WifiState *s, uint8_t kind)
{
    uint32_t len = cyw43439_read_le32(&s->load_header[0]);
    uint32_t hash = cyw43439_read_le32(&s->load_header[4]);

    if (kind == CYW_LOAD_FW) {
        if (!s->powered || s->reset_asserted) {
            cyw43439_load_error(s, "firmware begin before power/reset release");
            return;
        }
        if (!s->probed) {
            cyw43439_load_error(s, "firmware begin before probe");
            return;
        }
        s->fw_started = true;
        s->fw_committed = false;
        s->fw_ready = false;
        s->fw_error = false;
        s->fw_expected_len = len;
        s->fw_expected_hash = hash;
        s->fw_received_len = 0;
        s->fw_hash = CYW_FNV1A32_OFFSET;
    } else if (kind == CYW_LOAD_CLM) {
        if (!s->fw_committed) {
            cyw43439_load_error(s, "CLM begin before firmware commit");
            return;
        }
        s->clm_started = true;
        s->clm_committed = false;
        s->clm_expected_len = len;
        s->clm_expected_hash = hash;
        s->clm_received_len = 0;
        s->clm_hash = CYW_FNV1A32_OFFSET;
    } else {
        cyw43439_load_error(s, "unknown load kind");
        return;
    }

    s->mode = CYW_MODE_COMMAND;
}

static uint32_t *cyw43439_load_expected_len(CYW43439WifiState *s)
{
    return s->load_kind == CYW_LOAD_FW ?
           &s->fw_expected_len :
           &s->clm_expected_len;
}

static uint32_t *cyw43439_load_received_len(CYW43439WifiState *s)
{
    return s->load_kind == CYW_LOAD_FW ?
           &s->fw_received_len :
           &s->clm_received_len;
}

static uint32_t *cyw43439_load_hash(CYW43439WifiState *s)
{
    return s->load_kind == CYW_LOAD_FW ? &s->fw_hash : &s->clm_hash;
}

static bool cyw43439_load_started(const CYW43439WifiState *s)
{
    return s->load_kind == CYW_LOAD_FW ? s->fw_started : s->clm_started;
}

static bool cyw43439_load_committed(const CYW43439WifiState *s)
{
    return s->load_kind == CYW_LOAD_FW ? s->fw_committed : s->clm_committed;
}

static bool cyw43439_load_chunk_ready(CYW43439WifiState *s)
{
    uint32_t offset = cyw43439_read_le32(&s->load_chunk_header[0]);
    uint8_t len = s->load_chunk_header[4];
    uint32_t *expected_len = cyw43439_load_expected_len(s);
    uint32_t *received_len = cyw43439_load_received_len(s);

    if (s->load_kind != CYW_LOAD_FW && s->load_kind != CYW_LOAD_CLM) {
        cyw43439_load_error(s, "chunk without load kind");
        return false;
    }
    if (!cyw43439_load_started(s) || cyw43439_load_committed(s)) {
        cyw43439_load_error(s, "chunk outside active load");
        return false;
    }
    if (offset != *received_len) {
        cyw43439_load_error(s, "out-of-order firmware chunk");
        return false;
    }
    if (*received_len > *expected_len ||
        (uint32_t)len > *expected_len - *received_len) {
        cyw43439_load_error(s, "firmware chunk exceeds expected length");
        return false;
    }

    s->load_chunk_len = len;
    s->load_chunk_pos = 0;
    if (len == 0) {
        s->mode = CYW_MODE_COMMAND;
    } else {
        s->mode = CYW_MODE_LOAD_CHUNK_PAYLOAD;
    }
    return true;
}

static void cyw43439_load_chunk_byte(CYW43439WifiState *s, uint8_t byte)
{
    uint32_t *received_len = cyw43439_load_received_len(s);
    uint32_t *hash = cyw43439_load_hash(s);

    *hash = cyw43439_fnv1a32_update(*hash, byte);
    (*received_len)++;
    s->load_chunk_pos++;
    if (s->load_chunk_pos == s->load_chunk_len) {
        s->mode = CYW_MODE_COMMAND;
    }
}

static uint8_t cyw43439_load_commit(CYW43439WifiState *s, uint8_t kind)
{
    uint32_t expected_len;
    uint32_t expected_hash;
    uint32_t received_len;
    uint32_t hash;

    if (kind == CYW_LOAD_FW) {
        if (!s->fw_started) {
            cyw43439_load_error(s, "firmware commit before begin");
            return 0xee;
        }
        expected_len = s->fw_expected_len;
        expected_hash = s->fw_expected_hash;
        received_len = s->fw_received_len;
        hash = s->fw_hash;
    } else {
        if (!s->fw_committed || !s->clm_started) {
            cyw43439_load_error(s, "CLM commit before begin");
            return 0xee;
        }
        expected_len = s->clm_expected_len;
        expected_hash = s->clm_expected_hash;
        received_len = s->clm_received_len;
        hash = s->clm_hash;
    }

    if (received_len != expected_len || hash != expected_hash) {
        cyw43439_load_error(s, "firmware length/hash mismatch");
        return 0xee;
    }

    if (kind == CYW_LOAD_FW) {
        s->fw_committed = true;
    } else {
        s->clm_committed = true;
    }
    return 0xac;
}

static uint8_t cyw43439_fw_state(const CYW43439WifiState *s)
{
    uint8_t state = 0;

    if (s->fw_started) {
        state |= CYW_FW_STATE_FW_STARTED;
    }
    if (s->fw_committed) {
        state |= CYW_FW_STATE_FW_COMMITTED;
    }
    if (s->clm_started) {
        state |= CYW_FW_STATE_CLM_STARTED;
    }
    if (s->clm_committed) {
        state |= CYW_FW_STATE_CLM_COMMITTED;
    }
    if (s->nvram_applied) {
        state |= CYW_FW_STATE_NVRAM_APPLIED;
    }
    if (s->fw_ready) {
        state |= CYW_FW_STATE_READY;
    }
    if (s->fw_error) {
        state |= CYW_FW_STATE_ERROR;
    }
    return state;
}

static bool cyw43439_has_frame_for(const CYW43439WifiState *s, uint8_t node)
{
    for (uint8_t offset = 0; offset < s->queue_len; offset++) {
        uint8_t index = (s->queue_head + offset) % CYW43439_WIFI_QUEUE_DEPTH;

        if (s->queue[index].dst_node == node) {
            return true;
        }
    }
    return false;
}

static uint8_t cyw43439_local_rx_node(const CYW43439WifiState *s)
{
    return s->node_id != 0 ? s->node_id : s->selected_node;
}

static void cyw43439_signal_cpus(CYW43439WifiState *s)
{
    for (int i = 0; i < ARRAY_SIZE(s->cpu); i++) {
        if (s->cpu[i]) {
            armv7m_set_event(s->cpu[i]);
        }
    }
}

static void cyw43439_signal_if_rx_ready(CYW43439WifiState *s)
{
    uint8_t node = cyw43439_local_rx_node(s);

    if (s->fw_ready && node != 0 && cyw43439_has_frame_for(s, node)) {
        cyw43439_signal_cpus(s);
    }
}

static uint8_t cyw43439_peek_label_for(const CYW43439WifiState *s, uint8_t node)
{
    if (!s->fw_ready) {
        return 0xff;
    }

    for (uint8_t offset = 0; offset < s->queue_len; offset++) {
        uint8_t index = (s->queue_head + offset) % CYW43439_WIFI_QUEUE_DEPTH;
        const CYW43439WifiFrame *frame = &s->queue[index];

        if (frame->dst_node == node && frame->len > 17) {
            return frame->bytes[17];
        }
    }
    return 0xff;
}

static void cyw43439_queue_push(CYW43439WifiState *s, uint8_t dst_node,
                                const uint8_t *bytes, uint8_t len)
{
    CYW43439WifiFrame *frame;
    uint8_t index;

    if (cyw43439_queue_full(s)) {
        s->overflow = true;
        return;
    }

    index = (s->queue_head + s->queue_len) % CYW43439_WIFI_QUEUE_DEPTH;
    frame = &s->queue[index];
    frame->dst_node = dst_node;
    frame->len = len;
    memcpy(frame->bytes, bytes, len);
    s->queue_len++;
    cyw43439_signal_if_rx_ready(s);
}

static void cyw43439_radio_poll(CYW43439WifiState *s)
{
    uint8_t packet[2 + CYW43439_WIFI_FRAME_SIZE];
    struct sockaddr_in source_addr;
    socklen_t source_len;

    if (!s->fw_ready || !cyw43439_radio_enabled(s)) {
        return;
    }

    for (;;) {
        ssize_t got;
        uint8_t frame_len;

        source_len = sizeof(source_addr);
        got = recvfrom(s->radio_fd, packet, sizeof(packet), 0,
                       (struct sockaddr *)&source_addr, &source_len);
        if (got < 0) {
            if (errno != EAGAIN && errno != EWOULDBLOCK) {
                qemu_log_mask(LOG_GUEST_ERROR,
                              "cyw43439: UDP receive failed: %s\n",
                              strerror(errno));
            }
            return;
        }
        if (got < 2) {
            continue;
        }

        frame_len = packet[1];
        if (frame_len > CYW43439_WIFI_FRAME_SIZE || got != 2 + frame_len) {
            qemu_log_mask(LOG_GUEST_ERROR,
                          "cyw43439: malformed UDP frame length %zd/%u\n",
                          got, frame_len);
            continue;
        }
        if (cyw43439_radio_mesh_enabled(s)) {
            uint16_t source_port;
            uint16_t source_node;
            uint16_t frame_src_node;
            uint16_t frame_dst_node;

            if (source_len < sizeof(source_addr) ||
                source_addr.sin_family != AF_INET) {
                qemu_log_mask(LOG_GUEST_ERROR,
                              "cyw43439: malformed mesh source address\n");
                continue;
            }
            source_port = ntohs(source_addr.sin_port);
            if (source_port <= s->radio_port_base) {
                qemu_log_mask(LOG_GUEST_ERROR,
                              "cyw43439: mesh source port below base %u\n",
                              source_port);
                continue;
            }
            source_node = source_port - s->radio_port_base;
            if (source_node == 0 || source_node > s->node_count) {
                qemu_log_mask(LOG_GUEST_ERROR,
                              "cyw43439: mesh source node outside range %u\n",
                              source_node);
                continue;
            }
            if (packet[0] != s->node_id) {
                qemu_log_mask(LOG_GUEST_ERROR,
                              "cyw43439: mesh packet dst %u does not match local node %u\n",
                              packet[0], s->node_id);
                continue;
            }
            if (frame_len < 6) {
                qemu_log_mask(LOG_GUEST_ERROR,
                              "cyw43439: mesh frame too short for node binding %u\n",
                              frame_len);
                continue;
            }
            frame_src_node = cyw43439_read_be16(&packet[4]);
            frame_dst_node = cyw43439_read_be16(&packet[6]);
            if (frame_src_node != source_node || frame_dst_node != s->node_id) {
                qemu_log_mask(LOG_GUEST_ERROR,
                              "cyw43439: mesh frame node mismatch src %u/%u dst %u/%u\n",
                              frame_src_node, source_node,
                              frame_dst_node, s->node_id);
                continue;
            }
        }

        cyw43439_queue_push(s, packet[0], &packet[2], frame_len);
    }
}

static void cyw43439_radio_fd_ready(void *opaque)
{
    cyw43439_radio_poll(CYW43439_WIFI(opaque));
}

static void cyw43439_radio_send(CYW43439WifiState *s, uint8_t dst_node,
                                const uint8_t *bytes, uint8_t len)
{
    uint8_t packet[2 + CYW43439_WIFI_FRAME_SIZE];
    struct sockaddr_in peer_addr;
    const struct sockaddr_in *peer_addr_ptr = &s->radio_peer_addr;
    ssize_t sent;

    if (!s->fw_ready) {
        qemu_log_mask(LOG_GUEST_ERROR,
                      "cyw43439: transmit before firmware ready\n");
        s->overflow = true;
        return;
    }

    if (!cyw43439_radio_enabled(s)) {
        cyw43439_queue_push(s, dst_node, bytes, len);
        return;
    }

    if (cyw43439_radio_mesh_enabled(s)) {
        uint32_t peer_port;

        if (dst_node == s->node_id) {
            cyw43439_queue_push(s, dst_node, bytes, len);
            return;
        }
        if (dst_node == 0 || dst_node > s->node_count) {
            qemu_log_mask(LOG_GUEST_ERROR,
                          "cyw43439: invalid mesh destination node %u\n",
                          dst_node);
            s->overflow = true;
            return;
        }
        peer_port = (uint32_t)s->radio_port_base + dst_node;
        if (peer_port > UINT16_MAX) {
            qemu_log_mask(LOG_GUEST_ERROR,
                          "cyw43439: mesh destination port overflow %u\n",
                          peer_port);
            s->overflow = true;
            return;
        }
        memset(&peer_addr, 0, sizeof(peer_addr));
        peer_addr.sin_family = AF_INET;
        peer_addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        peer_addr.sin_port = htons((uint16_t)peer_port);
        peer_addr_ptr = &peer_addr;
    }

    packet[0] = dst_node;
    packet[1] = len;
    memcpy(&packet[2], bytes, len);

    sent = sendto(s->radio_fd, packet, 2 + len, 0,
                  (const struct sockaddr *)peer_addr_ptr,
                  sizeof(*peer_addr_ptr));
    if (sent != 2 + len) {
        qemu_log_mask(LOG_GUEST_ERROR,
                      "cyw43439: UDP transmit failed: %s\n",
                      strerror(errno));
        s->overflow = true;
    }
}

static bool cyw43439_queue_pop_for(CYW43439WifiState *s, uint8_t node,
                                   uint8_t *out, uint8_t *len)
{
    for (uint8_t offset = 0; offset < s->queue_len; offset++) {
        uint8_t index = (s->queue_head + offset) % CYW43439_WIFI_QUEUE_DEPTH;
        CYW43439WifiFrame *frame = &s->queue[index];

        if (frame->dst_node != node) {
            continue;
        }

        *len = frame->len;
        memcpy(out, frame->bytes, frame->len);

        for (uint8_t cursor = offset; cursor + 1 < s->queue_len; cursor++) {
            uint8_t from = (s->queue_head + cursor + 1) %
                           CYW43439_WIFI_QUEUE_DEPTH;
            uint8_t to = (s->queue_head + cursor) % CYW43439_WIFI_QUEUE_DEPTH;

            s->queue[to] = s->queue[from];
        }

        s->queue_len--;
        if (s->queue_len == 0) {
            s->queue_head = 0;
        }
        return true;
    }
    return false;
}

static void cyw43439_reset_state(CYW43439WifiState *s)
{
    s->selected_node = 0;
    s->mode = CYW_MODE_COMMAND;
    s->tx_dst_node = 0;
    s->tx_len = 0;
    s->tx_pos = 0;
    s->rx_len = 0;
    s->rx_pos = 0;
    s->queue_head = 0;
    s->queue_len = 0;
    s->load_kind = CYW_LOAD_NONE;
    s->load_header_pos = 0;
    s->load_chunk_header_pos = 0;
    s->load_chunk_len = 0;
    s->load_chunk_pos = 0;
    s->fw_expected_len = 0;
    s->fw_expected_hash = 0;
    s->fw_received_len = 0;
    s->fw_hash = CYW_FNV1A32_OFFSET;
    s->clm_expected_len = 0;
    s->clm_expected_hash = 0;
    s->clm_received_len = 0;
    s->clm_hash = CYW_FNV1A32_OFFSET;
    s->nvram_len = 0;
    s->nvram_hash = 0;
    s->overflow = false;
    s->powered = false;
    s->reset_asserted = true;
    s->probed = false;
    s->fw_started = false;
    s->fw_committed = false;
    s->clm_started = false;
    s->clm_committed = false;
    s->nvram_applied = false;
    s->fw_ready = false;
    s->fw_error = false;
    memset(s->load_header, 0, sizeof(s->load_header));
    memset(s->load_chunk_header, 0, sizeof(s->load_chunk_header));
    memset(s->tx_buf, 0, sizeof(s->tx_buf));
    memset(s->rx_buf, 0, sizeof(s->rx_buf));
    memset(s->queue, 0, sizeof(s->queue));
}

static uint8_t cyw43439_status(CYW43439WifiState *s)
{
    uint8_t status;

    if (!s->powered || s->reset_asserted || !s->fw_ready) {
        return s->overflow ? CYW_STATUS_OVERFLOW : 0;
    }

    status = CYW_STATUS_TX_READY | CYW_STATUS_JOINED;

    cyw43439_radio_poll(s);

    if (cyw43439_has_frame_for(s, s->selected_node)) {
        status |= CYW_STATUS_RX_READY;
    }
    if (s->overflow) {
        status |= CYW_STATUS_OVERFLOW;
    }
    return status;
}

static uint8_t cyw43439_start_rx(CYW43439WifiState *s)
{
    if (!s->fw_ready) {
        return 0;
    }

    cyw43439_radio_poll(s);

    s->rx_len = 0;
    s->rx_pos = 0;
    memset(s->rx_buf, 0, sizeof(s->rx_buf));

    if (!cyw43439_queue_pop_for(s, s->selected_node, s->rx_buf, &s->rx_len)) {
        return 0;
    }

    s->mode = CYW_MODE_RX_PAYLOAD;
    return s->rx_len;
}

static uint32_t cyw43439_transfer(SSIPeripheral *dev, uint32_t tx)
{
    CYW43439WifiState *s = CYW43439_WIFI(dev);
    uint8_t byte = tx & 0xff;
    uint8_t ret = 0xff;

    switch (s->mode) {
    case CYW_MODE_SELECT_NODE:
        s->selected_node = byte;
        s->mode = CYW_MODE_COMMAND;
        return 0xac;
    case CYW_MODE_TX_DST:
        s->tx_dst_node = byte;
        s->mode = CYW_MODE_TX_LEN;
        return 0xac;
    case CYW_MODE_TX_LEN:
        if (byte > CYW43439_WIFI_FRAME_SIZE) {
            qemu_log_mask(LOG_GUEST_ERROR,
                          "cyw43439: oversized TX frame %u\n", byte);
            s->overflow = true;
            s->mode = CYW_MODE_COMMAND;
            return 0xee;
        }
        s->tx_len = byte;
        s->tx_pos = 0;
        if (s->tx_len == 0) {
            cyw43439_radio_send(s, s->tx_dst_node, s->tx_buf, 0);
            s->mode = CYW_MODE_COMMAND;
        } else {
            s->mode = CYW_MODE_TX_PAYLOAD;
        }
        return 0xac;
    case CYW_MODE_TX_PAYLOAD:
        s->tx_buf[s->tx_pos++] = byte;
        if (s->tx_pos == s->tx_len) {
            cyw43439_radio_send(s, s->tx_dst_node, s->tx_buf, s->tx_len);
            s->mode = CYW_MODE_COMMAND;
        }
        return 0xac;
    case CYW_MODE_RX_PAYLOAD:
        if (s->rx_pos < s->rx_len) {
            ret = s->rx_buf[s->rx_pos++];
        }
        if (s->rx_pos >= s->rx_len) {
            s->mode = CYW_MODE_COMMAND;
        }
        return ret;
    case CYW_MODE_LOAD_BEGIN:
        s->load_header[s->load_header_pos++] = byte;
        if (s->load_header_pos == CYW43439_WIFI_LOAD_HEADER_SIZE) {
            cyw43439_load_begin(s, s->load_kind);
        }
        return 0xac;
    case CYW_MODE_LOAD_CHUNK_HEADER:
        s->load_chunk_header[s->load_chunk_header_pos++] = byte;
        if (s->load_chunk_header_pos == CYW43439_WIFI_LOAD_CHUNK_HEADER_SIZE) {
            cyw43439_load_chunk_ready(s);
        }
        return s->fw_error ? 0xee : 0xac;
    case CYW_MODE_LOAD_CHUNK_PAYLOAD:
        cyw43439_load_chunk_byte(s, byte);
        return 0xac;
    case CYW_MODE_NVRAM_APPLY:
        s->load_header[s->load_header_pos++] = byte;
        if (s->load_header_pos == CYW43439_WIFI_LOAD_HEADER_SIZE) {
            if (!s->fw_committed || !s->clm_committed) {
                cyw43439_load_error(s, "NVRAM apply before firmware/CLM commit");
                return 0xee;
            }
            s->nvram_len = cyw43439_read_le32(&s->load_header[0]);
            s->nvram_hash = cyw43439_read_le32(&s->load_header[4]);
            s->nvram_applied = true;
            s->mode = CYW_MODE_COMMAND;
        }
        return 0xac;
    case CYW_MODE_COMMAND:
    default:
        break;
    }

    switch (byte) {
    case CYW_CMD_IDENT:
        if (!s->powered || s->reset_asserted) {
            return 0;
        }
        s->probed = true;
        return 0x43;
    case CYW_CMD_SELECT_NODE:
        s->mode = CYW_MODE_SELECT_NODE;
        return 0xac;
    case CYW_CMD_STATUS:
        return cyw43439_status(s);
    case CYW_CMD_TX_FRAME:
        s->mode = CYW_MODE_TX_DST;
        s->tx_len = 0;
        s->tx_pos = 0;
        memset(s->tx_buf, 0, sizeof(s->tx_buf));
        return 0xac;
    case CYW_CMD_RX_FRAME:
        return cyw43439_start_rx(s);
    case CYW_CMD_RESET:
        cyw43439_reset_state(s);
        return 0xac;
    case CYW_CMD_CLEAR_STATUS:
        s->overflow = false;
        return 0xac;
    case CYW_CMD_POWER_ON:
        cyw43439_reset_state(s);
        s->powered = true;
        s->reset_asserted = true;
        return 0xac;
    case CYW_CMD_RESET_ASSERT:
        if (!s->powered) {
            cyw43439_load_error(s, "reset assert before power on");
            return 0xee;
        }
        cyw43439_reset_state(s);
        s->powered = true;
        s->reset_asserted = true;
        return 0xac;
    case CYW_CMD_RESET_RELEASE:
        if (!s->powered || !s->reset_asserted) {
            cyw43439_load_error(s, "reset release without asserted reset");
            return 0xee;
        }
        s->reset_asserted = false;
        s->fw_error = false;
        return 0xac;
    case CYW_CMD_PEEK_LABEL:
        cyw43439_radio_poll(s);
        return cyw43439_peek_label_for(s, s->selected_node);
    case CYW_CMD_NODE_ROLE:
        return s->node_role;
    case CYW_CMD_NODE_ID:
        return s->node_id;
    case CYW_CMD_RADIO_MODE:
        return cyw43439_radio_enabled(s) ? 1 : 0;
    case CYW_CMD_NODE_COUNT:
        return s->node_count;
    case CYW_CMD_FW_BEGIN:
        s->load_kind = CYW_LOAD_FW;
        s->load_header_pos = 0;
        memset(s->load_header, 0, sizeof(s->load_header));
        s->mode = CYW_MODE_LOAD_BEGIN;
        return 0xac;
    case CYW_CMD_FW_CHUNK:
        s->load_kind = CYW_LOAD_FW;
        s->load_chunk_header_pos = 0;
        memset(s->load_chunk_header, 0, sizeof(s->load_chunk_header));
        s->mode = CYW_MODE_LOAD_CHUNK_HEADER;
        return 0xac;
    case CYW_CMD_FW_COMMIT:
        return cyw43439_load_commit(s, CYW_LOAD_FW);
    case CYW_CMD_CLM_BEGIN:
        s->load_kind = CYW_LOAD_CLM;
        s->load_header_pos = 0;
        memset(s->load_header, 0, sizeof(s->load_header));
        s->mode = CYW_MODE_LOAD_BEGIN;
        return 0xac;
    case CYW_CMD_CLM_CHUNK:
        s->load_kind = CYW_LOAD_CLM;
        s->load_chunk_header_pos = 0;
        memset(s->load_chunk_header, 0, sizeof(s->load_chunk_header));
        s->mode = CYW_MODE_LOAD_CHUNK_HEADER;
        return 0xac;
    case CYW_CMD_CLM_COMMIT:
        return cyw43439_load_commit(s, CYW_LOAD_CLM);
    case CYW_CMD_NVRAM_APPLY:
        s->load_header_pos = 0;
        memset(s->load_header, 0, sizeof(s->load_header));
        s->mode = CYW_MODE_NVRAM_APPLY;
        return 0xac;
    case CYW_CMD_BOOT:
        if (!s->fw_committed || !s->clm_committed || !s->nvram_applied) {
            cyw43439_load_error(s, "boot before firmware, CLM, and NVRAM");
            return 0xee;
        }
        s->fw_ready = true;
        s->fw_error = false;
        cyw43439_signal_if_rx_ready(s);
        return 0xac;
    case CYW_CMD_FW_STATE:
        return cyw43439_fw_state(s);
    default:
        qemu_log_mask(LOG_GUEST_ERROR,
                      "cyw43439: unknown SPI command 0x%02x\n", byte);
        return 0xee;
    }
}

static void cyw43439_reset(DeviceState *dev)
{
    cyw43439_reset_state(CYW43439_WIFI(dev));
}

static void cyw43439_instance_init(Object *obj)
{
    CYW43439WifiState *s = CYW43439_WIFI(obj);

    s->radio_fd = -1;
    s->node_role = 0xff;
    s->node_count = 2;
}

static void cyw43439_realize(SSIPeripheral *dev, Error **errp)
{
    CYW43439WifiState *s = CYW43439_WIFI(dev);
    struct sockaddr_in local_addr = { 0 };
    uint32_t local_port;

    if ((s->radio_local_port == 0) != (s->radio_peer_port == 0)) {
        error_setg(errp,
                   "cyw43439: radio-local-port and radio-peer-port must be set together");
        return;
    }
    if (s->radio_port_base != 0 &&
        (s->radio_local_port != 0 || s->radio_peer_port != 0)) {
        error_setg(errp,
                   "cyw43439: radio-port-base cannot be combined with explicit peer ports");
        return;
    }
    if (s->radio_port_base != 0 && s->node_id == 0) {
        error_setg(errp, "cyw43439: radio-port-base requires node-id");
        return;
    }
    if (s->radio_port_base != 0 &&
        (s->node_count < 2 || s->node_id > s->node_count)) {
        error_setg(errp,
                   "cyw43439: node-id %u outside mesh node-count %u",
                   s->node_id, s->node_count);
        return;
    }
    if (s->radio_port_base != 0 &&
        s->radio_port_base > UINT16_MAX - s->node_count) {
        error_setg(errp,
                   "cyw43439: radio-port-base %u exceeds mesh port range for node-count %u",
                   s->radio_port_base, s->node_count);
        return;
    }
    if (s->radio_local_port == 0 && s->radio_port_base == 0) {
        return;
    }

    local_port = cyw43439_radio_mesh_enabled(s) ?
                 s->radio_port_base + s->node_id :
                 s->radio_local_port;

    s->radio_fd = qemu_socket(AF_INET, SOCK_DGRAM, 0);
    if (s->radio_fd < 0) {
        error_setg_errno(errp, errno, "cyw43439: cannot create UDP socket");
        return;
    }

    socket_set_fast_reuse(s->radio_fd);

    local_addr.sin_family = AF_INET;
    local_addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    local_addr.sin_port = htons((uint16_t)local_port);
    if (bind(s->radio_fd, (struct sockaddr *)&local_addr,
             sizeof(local_addr)) < 0) {
        error_setg_errno(errp, errno,
                         "cyw43439: cannot bind UDP port %u",
                         local_port);
        close(s->radio_fd);
        s->radio_fd = -1;
        return;
    }

    if (!qemu_set_blocking(s->radio_fd, false, errp)) {
        close(s->radio_fd);
        s->radio_fd = -1;
        return;
    }

    memset(&s->radio_peer_addr, 0, sizeof(s->radio_peer_addr));
    s->radio_peer_addr.sin_family = AF_INET;
    s->radio_peer_addr.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
    s->radio_peer_addr.sin_port = htons(s->radio_peer_port);
    qemu_set_fd_handler(s->radio_fd, cyw43439_radio_fd_ready, NULL, s);
}

static const VMStateDescription vmstate_cyw43439_frame = {
    .name = "cyw43439-frame",
    .version_id = 1,
    .minimum_version_id = 1,
    .fields = (const VMStateField[]) {
        VMSTATE_UINT8(dst_node, CYW43439WifiFrame),
        VMSTATE_UINT8(len, CYW43439WifiFrame),
        VMSTATE_UINT8_ARRAY(bytes, CYW43439WifiFrame,
                            CYW43439_WIFI_FRAME_SIZE),
        VMSTATE_END_OF_LIST()
    },
};

static const VMStateDescription vmstate_cyw43439_wifi = {
    .name = "cyw43439-wifi",
    .version_id = 1,
    .minimum_version_id = 1,
    .fields = (const VMStateField[]) {
        VMSTATE_SSI_PERIPHERAL(parent_obj, CYW43439WifiState),
        VMSTATE_UINT8(selected_node, CYW43439WifiState),
        VMSTATE_UINT8(mode, CYW43439WifiState),
        VMSTATE_UINT8(tx_dst_node, CYW43439WifiState),
        VMSTATE_UINT8(tx_len, CYW43439WifiState),
        VMSTATE_UINT8(tx_pos, CYW43439WifiState),
        VMSTATE_UINT8_ARRAY(tx_buf, CYW43439WifiState,
                            CYW43439_WIFI_FRAME_SIZE),
        VMSTATE_UINT8(rx_len, CYW43439WifiState),
        VMSTATE_UINT8(rx_pos, CYW43439WifiState),
        VMSTATE_UINT8_ARRAY(rx_buf, CYW43439WifiState,
                            CYW43439_WIFI_FRAME_SIZE),
        VMSTATE_UINT8(queue_head, CYW43439WifiState),
        VMSTATE_UINT8(queue_len, CYW43439WifiState),
        VMSTATE_UINT8(node_role, CYW43439WifiState),
        VMSTATE_UINT8(node_id, CYW43439WifiState),
        VMSTATE_UINT8(node_count, CYW43439WifiState),
        VMSTATE_UINT8(load_kind, CYW43439WifiState),
        VMSTATE_UINT8(load_header_pos, CYW43439WifiState),
        VMSTATE_UINT8(load_chunk_header_pos, CYW43439WifiState),
        VMSTATE_UINT8(load_chunk_len, CYW43439WifiState),
        VMSTATE_UINT8(load_chunk_pos, CYW43439WifiState),
        VMSTATE_UINT8_ARRAY(load_header, CYW43439WifiState,
                            CYW43439_WIFI_LOAD_HEADER_SIZE),
        VMSTATE_UINT8_ARRAY(load_chunk_header, CYW43439WifiState,
                            CYW43439_WIFI_LOAD_CHUNK_HEADER_SIZE),
        VMSTATE_UINT32(fw_expected_len, CYW43439WifiState),
        VMSTATE_UINT32(fw_expected_hash, CYW43439WifiState),
        VMSTATE_UINT32(fw_received_len, CYW43439WifiState),
        VMSTATE_UINT32(fw_hash, CYW43439WifiState),
        VMSTATE_UINT32(clm_expected_len, CYW43439WifiState),
        VMSTATE_UINT32(clm_expected_hash, CYW43439WifiState),
        VMSTATE_UINT32(clm_received_len, CYW43439WifiState),
        VMSTATE_UINT32(clm_hash, CYW43439WifiState),
        VMSTATE_UINT32(nvram_len, CYW43439WifiState),
        VMSTATE_UINT32(nvram_hash, CYW43439WifiState),
        VMSTATE_BOOL(overflow, CYW43439WifiState),
        VMSTATE_BOOL(powered, CYW43439WifiState),
        VMSTATE_BOOL(reset_asserted, CYW43439WifiState),
        VMSTATE_BOOL(probed, CYW43439WifiState),
        VMSTATE_BOOL(fw_started, CYW43439WifiState),
        VMSTATE_BOOL(fw_committed, CYW43439WifiState),
        VMSTATE_BOOL(clm_started, CYW43439WifiState),
        VMSTATE_BOOL(clm_committed, CYW43439WifiState),
        VMSTATE_BOOL(nvram_applied, CYW43439WifiState),
        VMSTATE_BOOL(fw_ready, CYW43439WifiState),
        VMSTATE_BOOL(fw_error, CYW43439WifiState),
        VMSTATE_STRUCT_ARRAY(queue, CYW43439WifiState,
                             CYW43439_WIFI_QUEUE_DEPTH, 0,
                             vmstate_cyw43439_frame, CYW43439WifiFrame),
        VMSTATE_END_OF_LIST()
    },
};

static const Property cyw43439_properties[] = {
    DEFINE_PROP_UINT16("radio-local-port", CYW43439WifiState,
                       radio_local_port, 0),
    DEFINE_PROP_UINT16("radio-peer-port", CYW43439WifiState,
                       radio_peer_port, 0),
    DEFINE_PROP_UINT16("radio-port-base", CYW43439WifiState,
                       radio_port_base, 0),
    DEFINE_PROP_UINT8("node-role", CYW43439WifiState, node_role, 0xff),
    DEFINE_PROP_UINT8("node-id", CYW43439WifiState, node_id, 0),
    DEFINE_PROP_UINT8("node-count", CYW43439WifiState, node_count, 2),
    DEFINE_PROP_LINK("cpu0", CYW43439WifiState, cpu[0], TYPE_ARMV7M,
                     ARMv7MState *),
    DEFINE_PROP_LINK("cpu1", CYW43439WifiState, cpu[1], TYPE_ARMV7M,
                     ARMv7MState *),
};

static void cyw43439_class_init(ObjectClass *klass, const void *data)
{
    DeviceClass *dc = DEVICE_CLASS(klass);
    SSIPeripheralClass *ssc = SSI_PERIPHERAL_CLASS(klass);

    dc->vmsd = &vmstate_cyw43439_wifi;
    device_class_set_props(dc, cyw43439_properties);
    device_class_set_legacy_reset(dc, cyw43439_reset);
    ssc->realize = cyw43439_realize;
    ssc->transfer = cyw43439_transfer;
    ssc->cs_polarity = SSI_CS_NONE;
}

static const TypeInfo cyw43439_info = {
    .name = TYPE_CYW43439_WIFI,
    .parent = TYPE_SSI_PERIPHERAL,
    .instance_size = sizeof(CYW43439WifiState),
    .instance_init = cyw43439_instance_init,
    .class_init = cyw43439_class_init,
};

static void cyw43439_register_types(void)
{
    type_register_static(&cyw43439_info);
}

type_init(cyw43439_register_types)
