#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::{
    arch::asm,
    ptr::{read_volatile, write_volatile},
};

#[cfg(all(target_arch = "arm", target_os = "none"))]
use crate::port::exec::signal;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_BASE: usize = 0x4005_4000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_ALARM0: *mut u32 = (TIMER_BASE + 0x10) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMERAWH: *const u32 = (TIMER_BASE + 0x24) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMERAWL: *const u32 = (TIMER_BASE + 0x28) as *const u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_DBGPAUSE: *mut u32 = (TIMER_BASE + 0x2c) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_INTR: *mut u32 = (TIMER_BASE + 0x34) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_INTE: *mut u32 = (TIMER_BASE + 0x38) as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_ALARM0_INTR: u32 = 1 << 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const NVIC_ISER: *mut u32 = 0xE000_E100 as *mut u32;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const TIMER_IRQ0_BIT: u32 = 1 << 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut TIMER0_RAW_READY: u32 = 0;
#[cfg(all(target_arch = "arm", target_os = "none"))]
static mut TIMER0_IRQ_COUNT: u32 = 0;

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub type IrqHandler = unsafe extern "C" fn();

#[cfg(all(target_arch = "arm", target_os = "none"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Alarm0Ready {
    irq_count: u32,
    captured_tick: u64,
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl Alarm0Ready {
    pub const fn new(irq_count: u32, captured_tick: u64) -> Self {
        Self {
            irq_count,
            captured_tick,
        }
    }

    pub const fn irq_count(self) -> u32 {
        self.irq_count
    }

    pub const fn captured_tick(self) -> u64 {
        self.captured_tick
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe extern "C" fn default_irq_handler() {
    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe extern "C" fn timer0_irq_handler() {
    unsafe {
        write_volatile(TIMER_INTR, TIMER_ALARM0_INTR);
        TIMER0_IRQ_COUNT = TIMER0_IRQ_COUNT.wrapping_add(1);
        TIMER0_RAW_READY = 1;
    }
    signal();
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn alarm0_irq_count() -> u32 {
    unsafe { read_volatile(core::ptr::addr_of!(TIMER0_IRQ_COUNT)) }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn now_ticks() -> u64 {
    loop {
        let hi0 = unsafe { read_volatile(TIMERAWH) };
        let lo = unsafe { read_volatile(TIMERAWL) };
        let hi1 = unsafe { read_volatile(TIMERAWH) };
        if hi0 == hi1 {
            return ((hi0 as u64) << 32) | lo as u64;
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn arm_alarm0_after_ticks(delay_ticks: u32) {
    let alarm = (now_ticks() as u32).wrapping_add(delay_ticks);
    unsafe {
        TIMER0_RAW_READY = 0;
        write_volatile(TIMER_DBGPAUSE, 0);
        write_volatile(TIMER_INTR, TIMER_ALARM0_INTR);
        write_volatile(TIMER_INTE, read_volatile(TIMER_INTE) | TIMER_ALARM0_INTR);
        write_volatile(NVIC_ISER, TIMER_IRQ0_BIT);
        write_volatile(TIMER_ALARM0, alarm);
        asm!("cpsie i");
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn alarm0_ready() -> bool {
    unsafe { read_volatile(core::ptr::addr_of!(TIMER0_RAW_READY)) != 0 }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn take_alarm0_ready() -> Option<Alarm0Ready> {
    unsafe {
        if read_volatile(core::ptr::addr_of!(TIMER0_RAW_READY)) == 0 {
            return None;
        }
        let irq_count = read_volatile(core::ptr::addr_of!(TIMER0_IRQ_COUNT));
        TIMER0_RAW_READY = 0;
        Some(Alarm0Ready::new(irq_count, now_ticks()))
    }
}
