#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(not(target_os = "none"))]
fn main() {
    println!("uno-q-m33-native-kernel is a bare-metal STM32U585 image");
}

#[cfg(target_os = "none")]
mod firmware {
    use core::{
        hint::spin_loop,
        ptr,
        sync::atomic::{AtomicU32, Ordering},
    };

    use hibana_pico::{appkit, appkit::ArtifactBundle, site};
    use uno_q_heterogeneous::protocol;
    use uno_q_heterogeneous::{UnoQCapsule, image};

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
    const USART_RDR: usize = 0x24;
    const USART_TDR: usize = 0x28;
    const USART_CR1_UE: u32 = 1 << 0;
    const USART_CR1_RE: u32 = 1 << 2;
    const USART_CR1_TE: u32 = 1 << 3;
    const USART_CR3_RTSE: u32 = 1 << 8;
    const USART_ISR_RXNE: u32 = 1 << 5;
    const USART_ISR_TXE: u32 = 1 << 7;

    const SYST_CSR: usize = 0xe000_e010;
    const SYST_RVR: usize = 0xe000_e014;
    const SYST_CVR: usize = 0xe000_e018;
    const SYSTICK_RELOAD_100US: u32 = 1_600 - 1;

    const HEARTBEAT_TICKS: u32 = 10_000;
    const MOUTH_TICKS: u32 = 5_000;

    const FACE_NEUTRAL: [&[u8; 13]; 8] = [
        b".............",
        b"..##.....##..",
        b"..##.....##..",
        b".............",
        b"....#####....",
        b".............",
        b".............",
        b".............",
    ];
    const FACE_HAPPY: [&[u8; 13]; 8] = [
        b".............",
        b"..##.....##..",
        b"..##.....##..",
        b".............",
        b"...#.....#...",
        b"....#...#....",
        b".....###.....",
        b".............",
    ];
    const FACE_SAD: [&[u8; 13]; 8] = [
        b".............",
        b"..##.....##..",
        b"..##.....##..",
        b".............",
        b".....###.....",
        b"....#...#....",
        b"...#.....#...",
        b".............",
    ];
    const FACE_ANGRY: [&[u8; 13]; 8] = [
        b".###.....###.",
        b"...##...##...",
        b"..##.....##..",
        b".............",
        b"...#######...",
        b".............",
        b".............",
        b".............",
    ];
    const FACE_SURPRISED: [&[u8; 13]; 8] = [
        b".............",
        b"..##.....##..",
        b"..##.....##..",
        b".............",
        b".....###.....",
        b"....#...#....",
        b".....###.....",
        b".............",
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
        b".............",
        b"....#####....",
        b"....#...#....",
        b"....#####....",
        b".............",
    ];
    const FACE_SPEAK_2: [&[u8; 13]; 8] = [
        b".............",
        b"..##.....##..",
        b"..##.....##..",
        b".............",
        b"...#######...",
        b"...#.....#...",
        b"...#######...",
        b".............",
    ];

    const CHARLIE_PAIRS: [(u8, u8); 104] = build_charlie_pairs();
    static TIMER_TICKS: AtomicU32 = AtomicU32::new(0);
    static mut BOARD: BoardChoreographicKernel = BoardChoreographicKernel::new();
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_RX_BYTES: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_RX_FRAMES: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_TX_FRAMES: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LAST_RX: u32 = 0;
    #[unsafe(no_mangle)]
    pub static mut HIBANA_M33_LAST_TX: u32 = 0;

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

    #[panic_handler]
    fn panic_handler(info: &core::panic::PanicInfo<'_>) -> ! {
        let _ = info;
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
        TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
    }

    unsafe extern "C" fn reset_handler() -> ! {
        unsafe {
            init_memory();
        }
        main()
    }

    fn main() -> ! {
        init_clocks_and_pins();
        init_uarts();
        init_matrix();
        init_systick();

        unsafe {
            (&mut *core::ptr::addr_of_mut!(BOARD)).resolve_boot();
        }

        type Image = site::Local<image::M33LedKernelImage>;
        let report =
            appkit::run::<Image, UnoQCapsule>(uno_q_heterogeneous::ARTIFACTS.for_image::<Image>());
        core::hint::black_box(report);
        loop {
            unsafe {
                (&mut *core::ptr::addr_of_mut!(BOARD)).resolve_timer_interrupt();
            }
            spin_loop();
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_write(byte: u8) {
        write_usart(USART1, byte);
        write_usart(LPUART1, byte);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_read() -> i16 {
        read_carrier_byte().map_or(-1, |byte| {
            unsafe {
                HIBANA_M33_RX_BYTES = HIBANA_M33_RX_BYTES.wrapping_add(1);
            }
            i16::from(byte)
        })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_observe_frame(source: u8, peer: u8, label: u8, len: u8) {
        unsafe {
            HIBANA_M33_RX_FRAMES = HIBANA_M33_RX_FRAMES.wrapping_add(1);
            HIBANA_M33_LAST_RX = ((source as u32) << 24)
                | ((peer as u32) << 16)
                | ((label as u32) << 8)
                | len as u32;
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_carrier_observe_tx(peer: u8, label: u8, len: u8) {
        unsafe {
            HIBANA_M33_TX_FRAMES = HIBANA_M33_TX_FRAMES.wrapping_add(1);
            HIBANA_M33_LAST_TX = ((peer as u32) << 16) | ((label as u32) << 8) | len as u32;
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_board_ready() {
        write_all(b"HIBANA_M33:APPKIT_READY\r\n");
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_board_poll() {
        unsafe {
            (&mut *core::ptr::addr_of_mut!(BOARD)).resolve_timer_interrupt();
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_board_accept_candidate(face: u8, mouth_frames: u8) {
        unsafe {
            (&mut *core::ptr::addr_of_mut!(BOARD)).accept_projected_candidate(face, mouth_frames);
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn uno_q_m33_board_commit_face(face: u8) {
        unsafe {
            (&mut *core::ptr::addr_of_mut!(BOARD)).commit_projected_face(face);
        }
    }

    #[derive(Clone, Copy)]
    enum FaceMode {
        Idle,
        Speaking,
    }

    #[derive(Clone, Copy)]
    struct FaceCandidateFact {
        face: u8,
        mouth_frames: u8,
    }

    struct BoardChoreographicKernel {
        candidate: Option<FaceCandidateFact>,
        face: FaceMode,
        scan: usize,
        observed_ticks: u32,
        heartbeat_tick: u32,
        mouth_tick: u32,
        speech_frame: u8,
    }

    impl BoardChoreographicKernel {
        const fn new() -> Self {
            Self {
                candidate: None,
                face: FaceMode::Idle,
                scan: 0,
                observed_ticks: 0,
                heartbeat_tick: 0,
                mouth_tick: 0,
                speech_frame: 0,
            }
        }

        fn resolve_boot(&mut self) {
            draw_face(FACE_NEUTRAL);
        }

        fn resolve_timer_interrupt(&mut self) {
            let ticks = TIMER_TICKS.load(Ordering::Relaxed);
            while self.observed_ticks != ticks {
                self.observed_ticks = self.observed_ticks.wrapping_add(1);
                self.resolve_timer_tick();
            }
        }

        fn resolve_timer_tick(&mut self) {
            refresh_one(self.scan);
            self.scan = (self.scan + 1) % 104;

            if self.observed_ticks.wrapping_sub(self.heartbeat_tick) >= HEARTBEAT_TICKS {
                self.heartbeat_tick = self.observed_ticks;
            }

            if let FaceMode::Speaking = self.face {
                if self.observed_ticks.wrapping_sub(self.mouth_tick) >= MOUTH_TICKS {
                    self.mouth_tick = self.observed_ticks;
                    self.speech_frame = (self.speech_frame + 1) % 4;
                    self.resolve_mouth_frame();
                }
            }
        }

        fn accept_projected_candidate(&mut self, face: u8, mouth_frames: u8) {
            self.resolve_face_candidate([face, mouth_frames]);
        }

        fn commit_projected_face(&mut self, face: u8) {
            let Some(candidate) = self.candidate else {
                self.reject_to_safe_state();
                return;
            };
            if face != candidate.face || !valid_face(face) {
                self.reject_to_safe_state();
                return;
            }
            self.apply_committed_face(face);
        }

        fn resolve_face_candidate(&mut self, payload: [u8; 2]) {
            let candidate = FaceCandidateFact {
                face: payload[0],
                mouth_frames: payload[1],
            };
            if !valid_face(candidate.face)
                || candidate.face == protocol::FACE_SPEAKING && candidate.mouth_frames < 3
            {
                self.reject_to_safe_state();
                return;
            }
            self.candidate = Some(candidate);
        }

        fn apply_committed_face(&mut self, face: u8) {
            self.speech_frame = 0;
            self.mouth_tick = self.observed_ticks;
            match face {
                protocol::FACE_NEUTRAL => {
                    draw_face(FACE_NEUTRAL);
                    self.face = FaceMode::Idle;
                }
                protocol::FACE_HAPPY => {
                    draw_face(FACE_HAPPY);
                    self.face = FaceMode::Idle;
                }
                protocol::FACE_SAD => {
                    draw_face(FACE_SAD);
                    self.face = FaceMode::Idle;
                }
                protocol::FACE_ANGRY => {
                    draw_face(FACE_ANGRY);
                    self.face = FaceMode::Idle;
                }
                protocol::FACE_SURPRISED => {
                    draw_face(FACE_SURPRISED);
                    self.face = FaceMode::Idle;
                }
                protocol::FACE_THINKING => {
                    draw_face(FACE_THINKING);
                    self.face = FaceMode::Idle;
                }
                protocol::FACE_SPEAKING => {
                    draw_face(FACE_SPEAK_0);
                    self.face = FaceMode::Speaking;
                }
                _ => self.reject_to_safe_state(),
            }
        }

        fn resolve_mouth_frame(&mut self) {
            match self.speech_frame {
                0 => draw_face(FACE_SPEAK_0),
                1 => draw_face(FACE_SPEAK_1),
                2 => draw_face(FACE_SPEAK_2),
                _ => draw_face(FACE_SPEAK_1),
            }
        }

        fn reject_to_safe_state(&mut self) {
            self.candidate = None;
            self.face = FaceMode::Idle;
            draw_face(FACE_NEUTRAL);
        }
    }

    fn valid_face(face: u8) -> bool {
        matches!(
            face,
            protocol::FACE_NEUTRAL
                | protocol::FACE_HAPPY
                | protocol::FACE_SAD
                | protocol::FACE_ANGRY
                | protocol::FACE_SURPRISED
                | protocol::FACE_THINKING
                | protocol::FACE_SPEAKING
        )
    }

    static mut MATRIX: [u8; 13] = [0; 13];

    fn draw_face(rows: [&'static [u8; 13]; 8]) {
        let mut packed = [0u8; 13];
        let mut row = 0usize;
        while row < 8 {
            let mut col = 0usize;
            while col < 13 {
                if rows[row][col] == b'#' {
                    let bit = row * 13 + col;
                    packed[bit / 8] |= 1 << (bit % 8);
                }
                col += 1;
            }
            row += 1;
        }
        unsafe {
            MATRIX = packed;
        }
    }

    fn refresh_one(index: usize) {
        let byte = unsafe { MATRIX[index / 8] };
        let on = ((byte >> (index % 8)) & 1) != 0;
        turn_led(index, on);
        delay(30);
        gpiof_all_input();
    }

    fn init_matrix() {
        modify_reg(RCC_AHB2ENR1, |value| value | (1 << 5));
        delay(1000);
        gpiof_all_input();
    }

    fn turn_led(index: usize, on: bool) {
        gpiof_all_input();
        if !on {
            return;
        }
        let (high, low) = CHARLIE_PAIRS[index];
        write_reg(GPIOF + GPIO_BSRR, (1 << high) | (1 << (low + 16)));
        modify_reg(GPIOF + GPIO_MODER, |value| {
            value | (1 << (high * 2)) | (1 << (low * 2))
        });
    }

    fn gpiof_all_input() {
        modify_reg(GPIOF + GPIO_MODER, |value| value & 0xff00_0000);
    }

    const fn build_charlie_pairs() -> [(u8, u8); 104] {
        let mut pairs = [(0u8, 1u8); 104];
        let mut out = 0usize;
        let mut high = 0u8;
        while high < 11 {
            let mut low = 0u8;
            while low < 11 {
                if high != low && out < 104 {
                    pairs[out] = (high, low);
                    out += 1;
                }
                low += 1;
            }
            high += 1;
        }
        pairs
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
        write_reg(SYST_RVR, SYSTICK_RELOAD_100US);
        write_reg(SYST_CVR, 0);
        write_reg(SYST_CSR, 0b111);
    }

    fn read_carrier_byte() -> Option<u8> {
        read_usart(USART1).or_else(|| read_usart(LPUART1))
    }

    fn read_usart(base: usize) -> Option<u8> {
        if read_reg(base + USART_ISR) & USART_ISR_RXNE == 0 {
            return None;
        }
        Some(read_reg(base + USART_RDR) as u8)
    }

    fn write_all(bytes: &[u8]) {
        for &byte in bytes {
            write_usart(USART1, byte);
            write_usart(LPUART1, byte);
        }
    }

    fn write_usart(base: usize, byte: u8) {
        let mut guard = 0u32;
        while read_reg(base + USART_ISR) & USART_ISR_TXE == 0 && guard < 1_000_000 {
            guard += 1;
            spin_loop();
        }
        write_reg(base + USART_TDR, byte as u32);
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
}
