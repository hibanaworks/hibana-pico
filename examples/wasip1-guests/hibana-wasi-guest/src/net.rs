//! Guest-side NetworkObject helpers.
//!
//! This module is a narrow safe facade over ChoreoFS-minted NetworkObject fds.
//! It does not expose socket addresses, route labels, raw fds, or transport
//! policy. Raw WASI Preview 1 imports stay in `sys`.

use crate::{Error, Result, choreofs, sys};

pub struct Datagram {
    fd: u32,
}

pub struct Stream {
    fd: u32,
}

pub struct Listener {
    fd: u32,
}

impl Datagram {
    pub const MAX_PAYLOAD: usize = 30;

    pub fn ping_pong() -> Result<Self> {
        Self::open_endpoint(DatagramEndpoint::PingPong)
    }

    pub fn gateway() -> Result<Self> {
        Self::open_endpoint(DatagramEndpoint::Gateway)
    }

    pub fn send(&self, payload: &[u8]) -> Result<()> {
        validate_payload_len(payload.len(), Self::MAX_PAYLOAD)?;
        sys::sock_send_exact(self.fd, payload)
    }

    pub fn recv(&self, out: &mut [u8]) -> Result<usize> {
        let limit = out.len().min(Self::MAX_PAYLOAD);
        sys::sock_recv_checked(self.fd, &mut out[..limit])
    }

    pub fn shutdown(self) -> Result<()> {
        sys::sock_shutdown_quiesce(self.fd)
    }

    fn open_endpoint(endpoint: DatagramEndpoint) -> Result<Self> {
        let fd = sys::open_path(
            choreofs::default_root_fd(),
            endpoint.path().as_bytes(),
            sys::FD_READ_RIGHT | sys::FD_WRITE_RIGHT,
        )?;
        Ok(Self { fd })
    }
}

impl Stream {
    pub const MAX_CHUNK: usize = 30;

    pub fn control() -> Result<Self> {
        Self::open_endpoint(StreamEndpoint::Control)
    }

    pub fn write_chunk(&self, bytes: &[u8]) -> Result<usize> {
        validate_payload_len(bytes.len(), Self::MAX_CHUNK)?;
        sys::sock_send_once(self.fd, bytes)
    }

    pub fn read_chunk(&self, out: &mut [u8]) -> Result<usize> {
        let limit = out.len().min(Self::MAX_CHUNK);
        sys::sock_recv_checked(self.fd, &mut out[..limit])
    }

    pub fn shutdown(self) -> Result<()> {
        sys::sock_shutdown_quiesce(self.fd)
    }

    fn accepted(fd: u32) -> Self {
        Self { fd }
    }

    fn open_endpoint(endpoint: StreamEndpoint) -> Result<Self> {
        let fd = sys::open_path(
            choreofs::default_root_fd(),
            endpoint.path().as_bytes(),
            sys::FD_READ_RIGHT | sys::FD_WRITE_RIGHT,
        )?;
        Ok(Self { fd })
    }
}

impl Listener {
    pub fn control() -> Result<Self> {
        Self::open_endpoint(ListenerEndpoint::Control)
    }

    pub fn accept_stream(&self) -> Result<Stream> {
        Ok(Stream::accepted(sys::sock_accept_stream(self.fd)?))
    }

    fn open_endpoint(endpoint: ListenerEndpoint) -> Result<Self> {
        let fd = sys::open_path(
            choreofs::default_root_fd(),
            endpoint.path().as_bytes(),
            sys::FD_READ_RIGHT,
        )?;
        Ok(Self { fd })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DatagramEndpoint {
    PingPong,
    Gateway,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StreamEndpoint {
    Control,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ListenerEndpoint {
    Control,
}

impl DatagramEndpoint {
    const fn path(self) -> &'static str {
        match self {
            Self::PingPong => "network/datagram/ping-pong",
            Self::Gateway => "network/datagram/gateway",
        }
    }
}

impl StreamEndpoint {
    const fn path(self) -> &'static str {
        match self {
            Self::Control => "network/stream/control",
        }
    }
}

impl ListenerEndpoint {
    const fn path(self) -> &'static str {
        match self {
            Self::Control => "network/listener/control",
        }
    }
}

fn validate_payload_len(actual: usize, max: usize) -> Result<()> {
    if actual > max {
        return Err(Error::PayloadTooLarge { max, actual });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datagram_endpoint_paths_are_private_choreofs_selectors() {
        assert_eq!(
            DatagramEndpoint::PingPong.path(),
            "network/datagram/ping-pong"
        );
        assert_eq!(DatagramEndpoint::Gateway.path(), "network/datagram/gateway");
    }

    #[test]
    fn datagram_payload_limit_matches_current_network_object_chunk() {
        assert_eq!(Datagram::MAX_PAYLOAD, 30);
    }

    #[test]
    fn datagram_send_rejects_payload_before_wasi_import() {
        assert_eq!(
            validate_payload_len(Datagram::MAX_PAYLOAD + 1, Datagram::MAX_PAYLOAD),
            Err(Error::PayloadTooLarge {
                max: Datagram::MAX_PAYLOAD,
                actual: Datagram::MAX_PAYLOAD + 1,
            })
        );
    }

    #[test]
    fn stream_and_listener_endpoint_paths_are_private_choreofs_selectors() {
        assert_eq!(StreamEndpoint::Control.path(), "network/stream/control");
        assert_eq!(ListenerEndpoint::Control.path(), "network/listener/control");
    }

    #[test]
    fn stream_chunk_limit_matches_current_wasip1_network_chunk() {
        assert_eq!(Stream::MAX_CHUNK, 30);
    }
}
