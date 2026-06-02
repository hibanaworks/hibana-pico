#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(not(target_os = "none"))]
fn main() {
    println!("uno-q-m33-native-kernel is a bare-metal STM32U585 image");
}

#[cfg(any(target_os = "none", test))]
#[path = "uno_q_m33_native_kernel/matrix.rs"]
mod matrix;

#[cfg(target_os = "none")]
const UNO_Q_M33_SYSTICK_RELOAD: u32 = 800 - 1;

#[cfg(test)]
mod animation_timing_tests {
    #[test]
    fn uno_q_native_kernel_keeps_face_output_below_endpoint_authority() {
        fn text(chars: &[char]) -> String {
            chars.iter().collect()
        }

        let source = include_str!("uno_q_m33_native_kernel.rs");
        for forbidden in [
            text(&['A', 't', 'o', 'm', 'i', 'c']),
            text(&['O', 'r', 'd', 'e', 'r', 'i', 'n', 'g']),
            text(&[
                'B', 'o', 'a', 'r', 'd', 'C', 'h', 'o', 'r', 'e', 'o', 'g', 'r', 'a', 'p', 'h',
                'i', 'c', 'K', 'e', 'r', 'n', 'e', 'l',
            ]),
            text(&[
                'F', 'a', 'c', 'e', 'C', 'a', 'n', 'd', 'i', 'd', 'a', 't', 'e', 'F', 'a', 'c', 't',
            ]),
            text(&[
                'F', 'A', 'C', 'E', '_', 'A', 'N', 'I', 'M', 'A', 'T', 'I', 'O', 'N',
            ]),
        ] {
            assert!(
                !source.contains(&forbidden),
                "M33 face output must stay below Endpoint authority; remove {forbidden}"
            );
        }
    }

    #[test]
    fn happy_normal_and_speaking_eyes_are_two_by_three_rectangles() {
        let source = include_str!("uno_q_m33_native_kernel.rs");
        assert!(source.contains(
            "const FACE_NEUTRAL: [&[u8; 13]; 8] = [\n        b\".............\",\n        b\"..##.....##..\",\n        b\"..##.....##..\",\n        b\"..##.....##..\","
        ));
        assert!(source.contains(
            "const FACE_HAPPY: [&[u8; 13]; 8] = [\n        b\".............\",\n        b\"..##.....##..\",\n        b\"..##.....##..\",\n        b\"..##.....##..\","
        ));
        for forbidden in [
            "const FACE_NEUTRAL: [&[u8; 13]; 8] = [\n        b\".............\",\n        b\"..###...###..\"",
            "const FACE_HAPPY: [&[u8; 13]; 8] = [\n        b\".###.....###.\",",
            "const FACE_NEUTRAL: [&[u8; 13]; 8] = [\n        b\".............\",\n        b\".####...####.\"",
            "const FACE_SPEAK_1: [&[u8; 13]; 8] = [\n        b\".............\",\n        b\"..###...###..\"",
            "const FACE_SPEAK_2: [&[u8; 13]; 8] = [\n        b\".............\",\n        b\"..###...###..\"",
            "const FACE_SPEAK_3: [&[u8; 13]; 8] = [\n        b\".............\",\n        b\"..###...###..\"",
        ] {
            assert!(
                !source.contains(forbidden),
                "normal/speaking eyes must stay 2x3 rectangles"
            );
        }
    }
}

#[cfg(target_os = "none")]
mod firmware {
    use core::{arch::asm, hint::spin_loop, ptr};

    use hibana_pico::{appkit, appkit::ArtifactBundle, site};
    use uno_q_heterogeneous::protocol;
    use uno_q_heterogeneous::{UnoQCapsule, image};

    use crate::matrix::{CHARLIE_PAIRS, MATRIX_BYTES, NUM_MATRIX_LEDS};

    const STACK_TOP: usize = 0x200c0000;

    const PWR: usize = 0x4602_0800;
    const PWR_SVMCR: usize = PWR + 0x10;

    const RCC: usize = 0x4602_0c00;
    const RCC_CR: usize = RCC;
    const RCC_CFGR1: usize = RCC + 0x1c;
    const RCC_AHB2ENR1: usize = RCC + 0x8c;
    const RCC_AHB3ENR: usize = RCC + 0x94;
    const RCC_APB2ENR: usize = RCC + 0xa4;
    const RCC_APB3ENR: usize = RCC + 0xa8;
    const RCC_CCIPR3: usize = RCC + 0xe0;

    const GPIOB: usize = 0x4202_0400;
    const GPIOF: usize = 0x4202_1400;
    const GPIOG: usize = 0x4202_1800;

    const GPIO_MODER: usize = 0x00;
    const GPIO_OTYPER: usize = 0x04;
    const GPIO_OSPEEDR: usize = 0x08;
    const GPIO_PUPDR: usize = 0x0c;
    const GPIO_AFRL: usize = 0x20;
    const GPIO_AFRH: usize = 0x24;
    const GPIO_BSRR: usize = 0x18;

    const USART1: usize = 0x4001_3800;
    const LPUART1: usize = 0x4600_2400;

    const USART_CR1: usize = 0x00;
    const USART_CR3: usize = 0x08;
    const USART_BRR: usize = 0x0c;
    const USART_ISR: usize = 0x1c;
    const USART_ICR: usize = 0x20;
    const USART_RDR: usize = 0x24;
    const USART_TDR: usize = 0x28;
    const USART_CR1_UE: u32 = 1 << 0;
    const USART_CR1_RE: u32 = 1 << 2;
    const USART_CR1_TE: u32 = 1 << 3;
    const USART_CR3_RTSE: u32 = 1 << 8;
    const USART_ISR_FE: u32 = 1 << 1;
    const USART_ISR_NE: u32 = 1 << 2;
    const USART_ISR_ORE: u32 = 1 << 3;
    const USART_ISR_RXNE: u32 = 1 << 5;
    const USART_ISR_TXE: u32 = 1 << 7;
    const USART_ERROR_FLAGS: u32 = USART_ISR_FE | USART_ISR_NE | USART_ISR_ORE;
    const USART_ICR_CLEAR_ERRORS: u32 = USART_ERROR_FLAGS;

    const RX_RING_CAP: usize = 256;

    const SYST_CSR: usize = 0xe000_e010;
    const SYST_RVR: usize = 0xe000_e014;
    const SYST_CVR: usize = 0xe000_e018;

    const FACE_NEUTRAL: [&[u8; 13]; 8] = [
        b".............",
        b"..##.....##..",
        b"..##.....##..",
        b"..##.....##..",
        b".............",
        b"...#######...",
        b".............",
        b".............",
    ];
    const FACE_HAPPY: [&[u8; 13]; 8] = [
        b".............",
        b"..##.....##..",
        b"..##.....##..",
        b"..##.....##..",
        b".............",
        b"..#.......#..",
        b"...#######...",
        b".............",
    ];
    const FACE_SAD: [&[u8; 13]; 8] = [
        b"...##...##...",
        b"..###...###..",
        b".###.....###.",
        b".............",
        b"....#####....",
        b"....#...#....",
        b"...#.....#...",
        b".............",
    ];
    const FACE_ANGRY: [&[u8; 13]; 8] = [
        b".###.....###.",
        b"..###...###..",
        b"...##...##...",
        b".............",
        b"..#########..",
        b".............",
        b".............",
        b".............",
    ];
    const FACE_SURPRISED: [&[u8; 13]; 8] = [
        b"..###...###..",
        b".#...#.#...#.",
        b".#...#.#...#.",
        b"..###...###..",
        b".............",
        b".....###.....",
        b"....#...#....",
        b".....###.....",
    ];
    const FACE_THINKING: [&[u8; 13]; 8] = [
        b".............",
        b"..##.....##..",
        b"...#.....#...",
        b".............",
        b".....###.....",
        b".......#.....",
        b"......#......",
        b".............",
    ];
    const FACE_SPEAK_0: [&[u8; 13]; 8] = FACE_NEUTRAL;
    const FACE_SPEAK_1: [&[u8; 13]; 8] = [
        b".............",
        b"..##.....##..",
        b"..##.....##..",
        b"..##.....##..",
        b".............",
        b"....#####....",
        b"....#####....",
        b".............",
    ];
    const FACE_SPEAK_2: [&[u8; 13]; 8] = [
        b".............",
        b"..##.....##..",
        b"..##.....##..",
        b"..##.....##..",
        b"...#######...",
        b"...#.....#...",
        b"...#######...",
        b".............",
    ];
    const FACE_SPEAK_3: [&[u8; 13]; 8] = [
        b".............",
        b"..##.....##..",
        b"..##.....##..",
        b"..##.....##..",
        b"....#####....",
        b"...#.....#...",
        b"....#####....",
        b".............",
    ];

    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_BOOT_STAGE: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_TIMER_TICKS: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_SCAN_TICKS: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_SCAN_INDEX: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LIT_TICKS: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LAST_LIT_LED: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_MATRIX_WORD0: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_MATRIX_BITS: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_FACE_UPDATES: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LAST_FACE: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_BOARD_POLLS: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_ROLE_STEP: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_PANIC_LINE: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_PANIC_COLUMN: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_USART1_RX_BYTES: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LPUART1_RX_BYTES: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_USART1_TX_BYTES: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LPUART1_TX_BYTES: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LAST_RX_UART: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_TX_READY_MASK: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_USART1_ISR: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LPUART1_ISR: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_USART1_ORE: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LPUART1_ORE: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_RX_RING_PUMPED: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_RX_RING_DROPS: u32 = 0;
    static mut RENDERER: LedRendererState = LedRendererState::new();
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_HINT_POLLS: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LAST_HINT_LANE: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_RX_BYTES: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_RX_FRAMES: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_PARSED_FRAMES: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_TX_FRAMES: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LAST_RX: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LAST_RX_PAYLOAD01: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LAST_TX: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_OPEN_SESSION: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_OPEN_PORT: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_REJECT_REASON: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_REJECT_EXPECT_SESSION: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_REJECT_FRAME_SESSION: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_REJECT_META: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_TRANSPORT_DEADLINE: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_TRANSPORT_DEADLINE_TICKS: u32 = 0;

    static mut RX_RING: [u16; RX_RING_CAP] = [0; RX_RING_CAP];
    static mut RX_RING_HEAD: usize = 0;
    static mut RX_RING_TAIL: usize = 0;

    #[repr(C)]
    struct VectorTable {
        stack_top: usize,
        reset: unsafe extern "C" fn() -> !,
        exceptions: [unsafe extern "C" fn(); 47],
    }

    #[used]
    #[unsafe(link_section = ".vector_table.reset_vector")]
    static VECTOR_TABLE: VectorTable = VectorTable {
        stack_top: STACK_TOP,
        reset: reset_handler,
        exceptions: build_exceptions(),
    };

    unsafe extern "C" {
        static mut _sidata: u32;
        static mut _sdata: u32;
        static mut _edata: u32;
        static mut _sbss: u32;
        static mut _ebss: u32;
    }

    #[inline(always)]
    fn marker_load(slot: *const u32) -> u32 {
        unsafe { ptr::read_volatile(slot) }
    }

    #[inline(always)]
    fn marker_store(slot: *mut u32, value: u32) {
        unsafe {
            ptr::write_volatile(slot, value);
        }
    }

    #[inline(always)]
    fn marker_add(slot: *mut u32, value: u32) -> u32 {
        let next = marker_load(slot).wrapping_add(value);
        marker_store(slot, next);
        next
    }

    #[inline(always)]
    fn disable_irq() -> u32 {
        let primask: u32;
        unsafe {
            asm!(
                "mrs {primask}, PRIMASK",
                "cpsid i",
                primask = out(reg) primask,
                options(nomem, nostack, preserves_flags),
            );
        }
        primask
    }

    #[inline(always)]
    fn restore_irq(primask: u32) {
        if primask & 1 == 0 {
            unsafe {
                asm!("cpsie i", options(nomem, nostack, preserves_flags));
            }
        }
    }

    fn with_renderer<R>(f: impl FnOnce(&mut LedRendererState) -> R) -> R {
        let primask = disable_irq();
        let out = unsafe { f(&mut *ptr::addr_of_mut!(RENDERER)) };
        restore_irq(primask);
        out
    }

    #[panic_handler]
    fn panic_handler(info: &core::panic::PanicInfo<'_>) -> ! {
        if let Some(location) = info.location() {
            marker_store(ptr::addr_of_mut!(HIBANA_M33_PANIC_LINE), location.line());
            marker_store(
                ptr::addr_of_mut!(HIBANA_M33_PANIC_COLUMN),
                location.column(),
            );
        }
        mark_stage(0xffff_0001);
        write_all(b"HIBANA_M33:PANIC\r\n");
        loop {
            spin_loop();
        }
    }

    const fn build_exceptions() -> [unsafe extern "C" fn(); 47] {
        let mut exceptions = [default_handler as unsafe extern "C" fn(); 47];
        exceptions[13] = systick_handler as unsafe extern "C" fn();
        exceptions
    }

    unsafe extern "C" fn default_handler() {
        loop {
            spin_loop();
        }
    }

    unsafe extern "C" fn systick_handler() {
        marker_add(ptr::addr_of_mut!(HIBANA_M33_TIMER_TICKS), 1);
        let renderer = unsafe { &mut *ptr::addr_of_mut!(RENDERER) };
        renderer.refresh_matrix();
    }

    unsafe extern "C" fn reset_handler() -> ! {
        unsafe {
            init_memory();
        }
        mark_stage(1);
        main()
    }

    fn main() -> ! {
        mark_stage(2);
        init_clocks_and_pins();
        mark_stage(3);
        init_uarts();
        mark_stage(4);
        init_matrix();
        mark_stage(5);
        init_systick();
        mark_stage(6);

        renderer_show_face(protocol::FACE_NEUTRAL);
        mark_stage(7);

        type Image = site::Local<image::M33LedKernelImage>;
        mark_stage(8);
        let report =
            appkit::run::<Image, UnoQCapsule>(uno_q_heterogeneous::ARTIFACTS.for_image::<Image>());
        mark_stage(9);
        core::hint::black_box(report);
        loop {
            spin_loop();
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_write(byte: u8) {
        mark_stage(0x200);
        let mut ready = 0u32;
        if write_usart(USART1, byte) {
            marker_add(ptr::addr_of_mut!(HIBANA_M33_USART1_TX_BYTES), 1);
            ready |= 1;
        }
        if write_usart(LPUART1, byte) {
            marker_add(ptr::addr_of_mut!(HIBANA_M33_LPUART1_TX_BYTES), 1);
            ready |= 2;
        }
        marker_store(ptr::addr_of_mut!(HIBANA_M33_TX_READY_MASK), ready);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_read() -> i16 {
        read_carrier_byte().map_or(-1, |(byte, source)| {
            marker_add(ptr::addr_of_mut!(HIBANA_M33_RX_BYTES), 1);
            marker_store(ptr::addr_of_mut!(HIBANA_M33_LAST_RX_UART), source);
            mark_stage(0x201 | (source << 8));
            i16::from(byte)
        })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_observe_frame(source: u8, peer: u8, label: u8, len: u8) {
        marker_add(ptr::addr_of_mut!(HIBANA_M33_RX_FRAMES), 1);
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_LAST_RX),
            ((source as u32) << 24) | ((peer as u32) << 16) | ((label as u32) << 8) | len as u32,
        );
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_observe_open(role: u8, lane: u8, session_id: u32) {
        marker_store(ptr::addr_of_mut!(HIBANA_M33_OPEN_SESSION), session_id);
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_OPEN_PORT),
            (u32::from(role) << 16) | (u32::from(lane) << 8),
        );
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_observe_parsed(
        session_id: u32,
        source: u8,
        peer: u8,
        label: u8,
        len: u8,
    ) {
        marker_add(ptr::addr_of_mut!(HIBANA_M33_PARSED_FRAMES), 1);
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_REJECT_FRAME_SESSION),
            session_id,
        );
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_REJECT_META),
            (u32::from(source) << 24)
                | (u32::from(peer) << 16)
                | (u32::from(label) << 8)
                | u32::from(len),
        );
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_observe_reject(
        reason: u8,
        expected_session_id: u32,
        frame_session_id: u32,
        source: u8,
        peer: u8,
        label: u8,
        len: u8,
    ) {
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_REJECT_REASON),
            u32::from(reason),
        );
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_REJECT_EXPECT_SESSION),
            expected_session_id,
        );
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_REJECT_FRAME_SESSION),
            frame_session_id,
        );
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_REJECT_META),
            (u32::from(source) << 24)
                | (u32::from(peer) << 16)
                | (u32::from(label) << 8)
                | u32::from(len),
        );
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_observe_payload(label: u8, len: u8, byte0: u8, byte1: u8) {
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_LAST_RX_PAYLOAD01),
            ((label as u32) << 24) | ((len as u32) << 16) | ((byte0 as u32) << 8) | byte1 as u32,
        );
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_observe_tx(peer: u8, label: u8, len: u8) {
        marker_add(ptr::addr_of_mut!(HIBANA_M33_TX_FRAMES), 1);
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_LAST_TX),
            ((peer as u32) << 16) | ((label as u32) << 8) | len as u32,
        );
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_observe_hint(lane: u8) {
        marker_add(ptr::addr_of_mut!(HIBANA_M33_HINT_POLLS), 1);
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_LAST_HINT_LANE),
            u32::from(lane),
        );
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_observe_deadline(op: u8, role: u8, lane: u8, elapsed: u32) {
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_TRANSPORT_DEADLINE),
            (u32::from(op) << 24) | (u32::from(role) << 16) | (u32::from(lane) << 8),
        );
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_TRANSPORT_DEADLINE_TICKS),
            elapsed,
        );
        mark_stage(0xffff_0002);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_timer_ticks() -> u32 {
        unsafe { ptr::read_volatile(ptr::addr_of!(HIBANA_M33_TIMER_TICKS)) }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_board_ready() {
        mark_stage(0x100);
        write_all(b"HIBANA_M33:APPKIT_READY\r\n");
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_board_poll() {
        marker_add(ptr::addr_of_mut!(HIBANA_M33_BOARD_POLLS), 1);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_role_step(step: u32) {
        marker_store(ptr::addr_of_mut!(HIBANA_M33_ROLE_STEP), step);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_board_show_face(face: u8) {
        marker_store(ptr::addr_of_mut!(HIBANA_M33_LAST_FACE), u32::from(face));
        marker_add(ptr::addr_of_mut!(HIBANA_M33_FACE_UPDATES), 1);
        mark_stage(0x500 | u32::from(face));
        renderer_show_face(face);
    }

    #[derive(Clone, Copy)]
    struct LedRendererState {
        packed: [u8; MATRIX_BYTES],
        scan_index: u8,
    }

    impl LedRendererState {
        const fn new() -> Self {
            Self {
                packed: [0; MATRIX_BYTES],
                scan_index: 0,
            }
        }

        fn show_face(&mut self, face: u8) {
            self.write_face(face_rows(face));
        }

        fn write_face(&mut self, rows: [&'static [u8; 13]; 8]) {
            self.packed = [0; MATRIX_BYTES];
            let mut bits = 0u32;
            let mut row = 0usize;
            while row < 8 {
                let mut col = 0usize;
                while col < 13 {
                    if rows[row][col] == b'#' {
                        let bit = row * 13 + col;
                        self.packed[bit / 8] |= 1 << (bit % 8);
                        bits += 1;
                    }
                    col += 1;
                }
                row += 1;
            }
            self.publish_matrix_markers(bits);
        }

        fn publish_matrix_markers(&self, bits: u32) {
            let word0 = unsafe {
                u32::from(*self.packed.as_ptr())
                    | (u32::from(*self.packed.as_ptr().add(1)) << 8)
                    | (u32::from(*self.packed.as_ptr().add(2)) << 16)
                    | (u32::from(*self.packed.as_ptr().add(3)) << 24)
            };
            marker_store(ptr::addr_of_mut!(HIBANA_M33_MATRIX_WORD0), word0);
            marker_store(ptr::addr_of_mut!(HIBANA_M33_MATRIX_BITS), bits);
        }

        fn refresh_matrix(&mut self) {
            marker_add(ptr::addr_of_mut!(HIBANA_M33_SCAN_TICKS), 1);

            let start = usize::from(self.scan_index);
            let start = if start < NUM_MATRIX_LEDS { start } else { 0 };
            let Some(index) = crate::matrix::next_lit_index(&self.packed, start) else {
                gpiof_all_input();
                self.scan_index = 0;
                marker_store(ptr::addr_of_mut!(HIBANA_M33_SCAN_INDEX), 0);
                return;
            };
            turn_led(index);
            let next = (index + 1) % NUM_MATRIX_LEDS;
            self.scan_index = next as u8;
            marker_store(ptr::addr_of_mut!(HIBANA_M33_SCAN_INDEX), next as u32);
        }
    }

    fn renderer_show_face(face: u8) {
        with_renderer(|renderer| renderer.show_face(face));
    }

    fn face_rows(face: u8) -> [&'static [u8; 13]; 8] {
        match face {
            protocol::FACE_HAPPY => FACE_HAPPY,
            protocol::FACE_SAD => FACE_SAD,
            protocol::FACE_ANGRY => FACE_ANGRY,
            protocol::FACE_SURPRISED => FACE_SURPRISED,
            protocol::FACE_THINKING => FACE_THINKING,
            protocol::FACE_MOUTH_CLOSED => FACE_SPEAK_0,
            protocol::FACE_MOUTH_SMALL => FACE_SPEAK_1,
            protocol::FACE_MOUTH_WIDE => FACE_SPEAK_2,
            protocol::FACE_MOUTH_ROUND => FACE_SPEAK_3,
            _ => FACE_NEUTRAL,
        }
    }

    fn init_matrix() {
        modify_reg(RCC_AHB2ENR1, |value| value | (1 << 5));
        delay(1000);
        modify_reg(GPIOF + GPIO_OTYPER, |value| value & !0x07ff);
        modify_reg(GPIOF + GPIO_OSPEEDR, |value| value | 0x003f_ffff);
        modify_reg(GPIOF + GPIO_PUPDR, |value| value & !0x003f_ffff);
        gpiof_all_input();
    }

    fn turn_led(index: usize) {
        gpiof_all_input();
        let (high, low) = CHARLIE_PAIRS[index];
        marker_add(ptr::addr_of_mut!(HIBANA_M33_LIT_TICKS), 1);
        marker_store(
            ptr::addr_of_mut!(HIBANA_M33_LAST_LIT_LED),
            ((index as u32) << 16) | ((high as u32) << 8) | ((low as u32) << 1) | 1,
        );
        write_reg(GPIOF + GPIO_BSRR, (1 << high) | (1 << (low + 16)));
        modify_reg(GPIOF + GPIO_MODER, |value| {
            value | (1 << (high * 2)) | (1 << (low * 2))
        });
    }

    fn gpiof_all_input() {
        modify_reg(GPIOF + GPIO_MODER, |value| value & 0xff00_0000);
    }

    fn init_clocks_and_pins() {
        select_hsi16_sysclk();
        modify_reg(RCC_AHB3ENR, |value| value | (1 << 2));
        delay(1000);
        enable_vddio2();
        select_lpuart1_hsi16();
        modify_reg(RCC_AHB2ENR1, |value| value | (1 << 1) | (1 << 5) | (1 << 6));
        modify_reg(RCC_APB2ENR, |value| value | (1 << 14));
        modify_reg(RCC_APB3ENR, |value| value | (1 << 6));
        delay(1000);

        configure_af(GPIOB, 6, 7);
        configure_af(GPIOB, 7, 7);
        configure_af(GPIOG, 5, 8);
        configure_af(GPIOG, 6, 8);
        configure_af(GPIOG, 7, 8);
        configure_af(GPIOG, 8, 8);
    }

    fn select_hsi16_sysclk() {
        modify_reg(RCC_CR, |value| value | (1 << 8));
        let mut guard = 0u32;
        while read_reg(RCC_CR) & (1 << 10) == 0 && guard < 1_000_000 {
            guard += 1;
            spin_loop();
        }
        modify_reg(RCC_CFGR1, |value| (value & !0b11) | 0b01);
        guard = 0;
        while read_reg(RCC_CFGR1) & 0b1100 != 0b0100 && guard < 1_000_000 {
            guard += 1;
            spin_loop();
        }
    }

    fn enable_vddio2() {
        modify_reg(PWR_SVMCR, |value| value | (1 << 29));
        delay(10_000);
    }

    fn select_lpuart1_hsi16() {
        modify_reg(RCC_CCIPR3, |value| (value & !0b111) | 0b010);
    }

    fn configure_af(port: usize, pin: u8, af: u8) {
        let shift = pin * 2;
        modify_reg(port + GPIO_MODER, |value| {
            (value & !(0b11 << shift)) | (0b10 << shift)
        });
        modify_reg(port + GPIO_OTYPER, |value| value & !(1 << pin));
        modify_reg(port + GPIO_OSPEEDR, |value| value | (0b11 << shift));
        modify_reg(port + GPIO_PUPDR, |value| {
            (value & !(0b11 << shift)) | (0b01 << shift)
        });

        let afr = if pin < 8 { GPIO_AFRL } else { GPIO_AFRH };
        let afr_shift = (pin % 8) * 4;
        modify_reg(port + afr, |value| {
            (value & !(0b1111 << afr_shift)) | ((af as u32) << afr_shift)
        });
    }

    fn init_uarts() {
        init_usart(USART1, 139);
        init_usart(LPUART1, 35_556);
    }

    fn init_usart(base: usize, brr: u32) {
        write_reg(base + USART_CR1, 0);
        write_reg(
            base + USART_CR3,
            if base == LPUART1 { USART_CR3_RTSE } else { 0 },
        );
        write_reg(base + USART_BRR, brr);
        write_reg(base + USART_CR1, USART_CR1_UE | USART_CR1_TE | USART_CR1_RE);
    }

    fn init_systick() {
        write_reg(SYST_RVR, crate::UNO_Q_M33_SYSTICK_RELOAD);
        write_reg(SYST_CVR, 0);
        write_reg(SYST_CSR, 0b111);
    }

    fn read_carrier_byte() -> Option<(u8, u32)> {
        unsafe { pop_rx_ring() }.or_else(|| {
            pump_rx_ring();
            unsafe { pop_rx_ring() }
        })
    }

    fn pump_rx_ring() {
        let mut guard = 0u32;
        while guard < 64 && pump_rx_ring_once() {
            guard += 1;
        }
    }

    fn pump_rx_ring_once() -> bool {
        let mut moved = false;
        if let Some((byte, source)) = read_usart_direct(USART1, 1) {
            unsafe {
                push_rx_ring(byte, source);
            }
            moved = true;
        }
        if let Some((byte, source)) = read_usart_direct(LPUART1, 2) {
            unsafe {
                push_rx_ring(byte, source);
            }
            moved = true;
        }
        moved
    }

    unsafe fn push_rx_ring(byte: u8, source: u32) {
        let head = unsafe { ptr::read_volatile(ptr::addr_of!(RX_RING_HEAD)) };
        let tail = unsafe { ptr::read_volatile(ptr::addr_of!(RX_RING_TAIL)) };
        let next = (head + 1) % RX_RING_CAP;
        if next == tail {
            marker_add(ptr::addr_of_mut!(HIBANA_M33_RX_RING_DROPS), 1);
            return;
        }
        let word = ((source as u16) << 8) | u16::from(byte);
        unsafe {
            ptr::write_volatile(ptr::addr_of_mut!(RX_RING).cast::<u16>().add(head), word);
            ptr::write_volatile(ptr::addr_of_mut!(RX_RING_HEAD), next);
        }
        marker_add(ptr::addr_of_mut!(HIBANA_M33_RX_RING_PUMPED), 1);
    }

    unsafe fn pop_rx_ring() -> Option<(u8, u32)> {
        let head = unsafe { ptr::read_volatile(ptr::addr_of!(RX_RING_HEAD)) };
        let tail = unsafe { ptr::read_volatile(ptr::addr_of!(RX_RING_TAIL)) };
        if tail == head {
            return None;
        }
        let word = unsafe { ptr::read_volatile(ptr::addr_of!(RX_RING).cast::<u16>().add(tail)) };
        unsafe {
            ptr::write_volatile(ptr::addr_of_mut!(RX_RING_TAIL), (tail + 1) % RX_RING_CAP);
        }
        Some(((word & 0xff) as u8, u32::from(word >> 8)))
    }

    fn read_usart_direct(base: usize, source: u32) -> Option<(u8, u32)> {
        let isr = read_reg(base + USART_ISR);
        if source == 1 {
            marker_store(ptr::addr_of_mut!(HIBANA_M33_USART1_ISR), isr);
        } else {
            marker_store(ptr::addr_of_mut!(HIBANA_M33_LPUART1_ISR), isr);
        }
        if isr & USART_ISR_ORE != 0 {
            if source == 1 {
                marker_add(ptr::addr_of_mut!(HIBANA_M33_USART1_ORE), 1);
            } else {
                marker_add(ptr::addr_of_mut!(HIBANA_M33_LPUART1_ORE), 1);
            }
        }
        let byte = if isr & USART_ISR_RXNE == 0 {
            None
        } else {
            Some(read_reg(base + USART_RDR) as u8)
        };
        if isr & USART_ERROR_FLAGS != 0 {
            write_reg(base + USART_ICR, USART_ICR_CLEAR_ERRORS);
        }
        let byte = byte?;
        if source == 1 {
            marker_add(ptr::addr_of_mut!(HIBANA_M33_USART1_RX_BYTES), 1);
        } else {
            marker_add(ptr::addr_of_mut!(HIBANA_M33_LPUART1_RX_BYTES), 1);
        }
        Some((byte, source))
    }

    fn write_all(bytes: &[u8]) {
        for &byte in bytes {
            pump_rx_ring();
            if write_usart(USART1, byte) {
                marker_add(ptr::addr_of_mut!(HIBANA_M33_USART1_TX_BYTES), 1);
            }
            if write_usart(LPUART1, byte) {
                marker_add(ptr::addr_of_mut!(HIBANA_M33_LPUART1_TX_BYTES), 1);
            }
        }
    }

    fn write_usart(base: usize, byte: u8) -> bool {
        let mut guard = 0u32;
        while read_reg(base + USART_ISR) & USART_ISR_TXE == 0 && guard < 1_000_000 {
            pump_rx_ring();
            guard += 1;
            spin_loop();
        }
        if read_reg(base + USART_ISR) & USART_ISR_TXE == 0 {
            return false;
        }
        write_reg(base + USART_TDR, byte as u32);
        pump_rx_ring();
        true
    }

    unsafe fn init_memory() {
        let mut src = ptr::addr_of!(_sidata);
        let mut dst = ptr::addr_of_mut!(_sdata);
        let end = ptr::addr_of_mut!(_edata);
        while dst < end {
            unsafe {
                ptr::write_volatile(dst, ptr::read_volatile(src));
                dst = dst.add(1);
                src = src.add(1);
            }
        }

        let mut bss = ptr::addr_of_mut!(_sbss);
        let bss_end = ptr::addr_of_mut!(_ebss);
        while bss < bss_end {
            unsafe {
                ptr::write_volatile(bss, 0);
                bss = bss.add(1);
            }
        }
    }

    fn read_reg(addr: usize) -> u32 {
        unsafe { ptr::read_volatile(addr as *const u32) }
    }

    fn write_reg(addr: usize, value: u32) {
        unsafe {
            ptr::write_volatile(addr as *mut u32, value);
        }
    }

    fn modify_reg(addr: usize, f: impl FnOnce(u32) -> u32) {
        let value = read_reg(addr);
        write_reg(addr, f(value));
    }

    fn delay(cycles: u32) {
        let mut remaining = cycles;
        while remaining > 0 {
            spin_loop();
            remaining -= 1;
        }
    }

    fn mark_stage(stage: u32) {
        marker_store(ptr::addr_of_mut!(HIBANA_M33_BOOT_STAGE), stage);
    }
}
