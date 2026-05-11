use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

#[cfg(all(target_arch = "arm", target_os = "none"))]
use core::arch::asm;

fn noop_raw_waker() -> RawWaker {
    fn clone(_: *const ()) -> RawWaker {
        noop_raw_waker()
    }
    fn wake(_: *const ()) {}
    fn wake_by_ref(_: *const ()) {}
    fn drop(_: *const ()) {}

    RawWaker::new(
        core::ptr::null(),
        &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
    )
}

#[inline(always)]
fn wait_pending() {
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    unsafe {
        // The outer firmware harness uses direct localside `.await` calls with
        // a tiny raw waker. `Pending` means "poll the projected role again",
        // not hidden scheduler sleep authority. Real sleeps are explicit
        // timer choreography steps.
        asm!("yield", options(nomem, nostack, preserves_flags));
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    core::hint::spin_loop();
}

#[inline(always)]
fn wait_idle() {
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    unsafe {
        asm!("wfi", options(nomem, nostack, preserves_flags));
    }

    #[cfg(not(all(target_arch = "arm", target_os = "none")))]
    core::hint::spin_loop();
}

#[inline(always)]
pub fn signal() {
    #[cfg(all(target_arch = "arm", target_os = "none"))]
    unsafe {
        asm!("sev", options(nomem, nostack, preserves_flags));
    }
}

pub fn wait_until(mut ready: impl FnMut() -> bool) {
    while !ready() {
        wait_pending();
    }
}

pub fn run_current_task<F: Future>(future: F) -> F::Output {
    // Harness glue only: localside operations stay as direct `.await` calls
    // inside the outer async task.
    let waker = unsafe { Waker::from_raw(noop_raw_waker()) };
    let mut cx = Context::from_waker(&waker);
    let mut future = future;
    let mut future = unsafe { Pin::new_unchecked(&mut future) };
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => wait_pending(),
        }
    }
}

pub fn park() -> ! {
    loop {
        wait_idle();
    }
}
