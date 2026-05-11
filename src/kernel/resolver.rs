use core::cell::Cell;

use crate::{
    choreography::protocol::{BudgetExpired, GpioEdge, GpioWait, TimerSleepDone, TimerSleepUntil},
    kernel::device::timer::{TimerError, TimerSleepId, TimerSleepTable},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolverError {
    EventQueueFull,
    GpioWaitTableFull,
    GpioWaitConflict,
    GpioWaitNotFound,
    StaleGpioWaitGeneration,
    UnsolicitedGpioEdge,
    Timer(TimerError),
}

impl From<TimerError> for ResolverError {
    fn from(value: TimerError) -> Self {
        Self::Timer(value)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResolverRejectionTelemetry {
    event_queue_full: u16,
    gpio_wait_table_full: u16,
    gpio_wait_conflict: u16,
    gpio_wait_not_found: u16,
    stale_generation: u16,
    unsolicited_event: u16,
    timer: u16,
    other: u16,
}

impl ResolverRejectionTelemetry {
    pub const fn new() -> Self {
        Self {
            event_queue_full: 0,
            gpio_wait_table_full: 0,
            gpio_wait_conflict: 0,
            gpio_wait_not_found: 0,
            stale_generation: 0,
            unsolicited_event: 0,
            timer: 0,
            other: 0,
        }
    }

    pub const fn event_queue_full(self) -> u16 {
        self.event_queue_full
    }

    pub const fn gpio_wait_table_full(self) -> u16 {
        self.gpio_wait_table_full
    }

    pub const fn gpio_wait_conflict(self) -> u16 {
        self.gpio_wait_conflict
    }

    pub const fn gpio_wait_not_found(self) -> u16 {
        self.gpio_wait_not_found
    }

    pub const fn stale_generation(self) -> u16 {
        self.stale_generation
    }

    pub const fn unsolicited_event(self) -> u16 {
        self.unsolicited_event
    }

    pub const fn timer(self) -> u16 {
        self.timer
    }

    pub const fn other(self) -> u16 {
        self.other
    }

    pub const fn total(self) -> u16 {
        self.event_queue_full
            .saturating_add(self.gpio_wait_table_full)
            .saturating_add(self.gpio_wait_conflict)
            .saturating_add(self.gpio_wait_not_found)
            .saturating_add(self.stale_generation)
            .saturating_add(self.unsolicited_event)
            .saturating_add(self.timer)
            .saturating_add(self.other)
    }

    fn record(&mut self, error: ResolverError) {
        let slot = match error {
            ResolverError::EventQueueFull => &mut self.event_queue_full,
            ResolverError::GpioWaitTableFull => &mut self.gpio_wait_table_full,
            ResolverError::GpioWaitConflict => &mut self.gpio_wait_conflict,
            ResolverError::GpioWaitNotFound => &mut self.gpio_wait_not_found,
            ResolverError::StaleGpioWaitGeneration => &mut self.stale_generation,
            ResolverError::UnsolicitedGpioEdge => &mut self.unsolicited_event,
            ResolverError::Timer(_) => &mut self.timer,
        };
        *slot = slot.saturating_add(1);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InterruptEvent {
    TimerTick { tick: u64 },
    GpioEdge { pin: u8, high: bool },
    TransportRxReady { role: u8, lane: u8, label_hint: u8 },
    BudgetTimerExpired { run_id: u16, generation: u16 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedInterrupt {
    TimerSleepDone(TimerSleepDone),
    GpioWaitSatisfied(GpioEdge),
    TransportRxReady { role: u8, lane: u8, label_hint: u8 },
    BudgetExpired(BudgetExpired),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GpioWaitRegistration {
    wait_id: u16,
    pin: u8,
    generation: u16,
}

impl GpioWaitRegistration {
    const fn from_wait(wait: GpioWait) -> Self {
        Self {
            wait_id: wait.wait_id(),
            pin: wait.pin(),
            generation: wait.generation(),
        }
    }
}

pub struct PicoInterruptResolver<const TIMERS: usize, const EVENTS: usize, const GPIO_WAITS: usize>
{
    timers: TimerSleepTable<TIMERS>,
    now_tick: u64,
    events: [Option<InterruptEvent>; EVENTS],
    event_head: usize,
    event_len: usize,
    gpio_waits: [Option<GpioWaitRegistration>; GPIO_WAITS],
    rejection_telemetry: Cell<ResolverRejectionTelemetry>,
}

impl<const TIMERS: usize, const EVENTS: usize, const GPIO_WAITS: usize>
    PicoInterruptResolver<TIMERS, EVENTS, GPIO_WAITS>
{
    pub const fn new() -> Self {
        Self {
            timers: TimerSleepTable::new(),
            now_tick: 0,
            events: [None; EVENTS],
            event_head: 0,
            event_len: 0,
            gpio_waits: [None; GPIO_WAITS],
            rejection_telemetry: Cell::new(ResolverRejectionTelemetry::new()),
        }
    }

    pub const fn now_tick(&self) -> u64 {
        self.now_tick
    }

    pub fn rejection_telemetry(&self) -> ResolverRejectionTelemetry {
        self.rejection_telemetry.get()
    }

    pub fn request_timer_sleep(
        &mut self,
        sleep: TimerSleepUntil,
    ) -> Result<TimerSleepId, ResolverError> {
        let result = self
            .timers
            .request_sleep(sleep)
            .map_err(ResolverError::from);
        self.record_result(result)
    }

    pub fn cancel_timer_sleep(&mut self, id: TimerSleepId) -> Result<(), ResolverError> {
        let result = self.timers.cancel(id).map_err(ResolverError::from);
        self.record_result(result)
    }

    pub fn request_gpio_edge(&mut self, pin: u8) -> Result<(), ResolverError> {
        self.request_gpio_wait(GpioWait::new(0, 1, pin, 1))
    }

    pub fn request_gpio_wait(&mut self, wait: GpioWait) -> Result<(), ResolverError> {
        let registration = GpioWaitRegistration::from_wait(wait);
        if let Some(active) = self
            .gpio_waits
            .iter()
            .flatten()
            .find(|active| active.wait_id == registration.wait_id)
        {
            if active.generation != registration.generation {
                return Err(self.record_rejection(ResolverError::StaleGpioWaitGeneration));
            }
            if active.pin != registration.pin {
                return Err(self.record_rejection(ResolverError::GpioWaitConflict));
            }
            return Ok(());
        }
        if self
            .gpio_waits
            .iter()
            .flatten()
            .any(|active| active.pin == registration.pin)
        {
            return Err(self.record_rejection(ResolverError::GpioWaitConflict));
        }
        let Some(slot) = self.gpio_waits.iter_mut().find(|slot| slot.is_none()) else {
            return Err(self.record_rejection(ResolverError::GpioWaitTableFull));
        };
        *slot = Some(registration);
        Ok(())
    }

    pub fn cancel_gpio_wait(&mut self, wait_id: u16, generation: u16) -> Result<(), ResolverError> {
        let Some(slot) = self
            .gpio_waits
            .iter_mut()
            .find(|slot| slot.is_some_and(|active| active.wait_id == wait_id))
        else {
            return Err(self.record_rejection(ResolverError::GpioWaitNotFound));
        };
        let active = (*slot).expect("matched gpio wait must exist");
        if active.generation != generation {
            return Err(self.record_rejection(ResolverError::StaleGpioWaitGeneration));
        }
        *slot = None;
        Ok(())
    }

    pub fn fence_gpio_waits(&mut self) -> usize {
        let mut revoked = 0;
        for slot in &mut self.gpio_waits {
            if slot.take().is_some() {
                revoked += 1;
            }
        }
        revoked
    }

    pub fn active_gpio_wait_count(&self) -> usize {
        self.gpio_waits.iter().flatten().count()
    }

    pub fn has_active_gpio_waits(&self) -> bool {
        self.active_gpio_wait_count() != 0
    }

    pub fn push_irq(&mut self, event: InterruptEvent) -> Result<(), ResolverError> {
        match event {
            InterruptEvent::TimerTick { tick } => {
                if tick > self.now_tick {
                    self.now_tick = tick;
                }
                Ok(())
            }
            _ => self.push_event(event),
        }
    }

    pub fn resolve_next(&mut self) -> Result<Option<ResolvedInterrupt>, ResolverError> {
        if let Some(done) = self.timers.poll(self.now_tick) {
            return Ok(Some(ResolvedInterrupt::TimerSleepDone(done)));
        }

        while let Some(event) = self.pop_event() {
            match event {
                InterruptEvent::TimerTick { tick } => {
                    if tick > self.now_tick {
                        self.now_tick = tick;
                    }
                    if let Some(done) = self.timers.poll(self.now_tick) {
                        return Ok(Some(ResolvedInterrupt::TimerSleepDone(done)));
                    }
                }
                InterruptEvent::GpioEdge { pin, high } => {
                    let Some(wait) = self
                        .gpio_waits
                        .iter_mut()
                        .find(|wait| wait.is_some_and(|active| active.pin == pin))
                    else {
                        return Err(self.record_rejection(ResolverError::UnsolicitedGpioEdge));
                    };
                    let active = (*wait).expect("matched gpio wait must exist");
                    *wait = None;
                    return Ok(Some(ResolvedInterrupt::GpioWaitSatisfied(GpioEdge::new(
                        active.wait_id,
                        pin,
                        high,
                        active.generation,
                    ))));
                }
                InterruptEvent::TransportRxReady {
                    role,
                    lane,
                    label_hint,
                } => {
                    return Ok(Some(ResolvedInterrupt::TransportRxReady {
                        role,
                        lane,
                        label_hint,
                    }));
                }
                InterruptEvent::BudgetTimerExpired { run_id, generation } => {
                    return Ok(Some(ResolvedInterrupt::BudgetExpired(BudgetExpired::new(
                        run_id, generation,
                    ))));
                }
            }
        }

        Ok(None)
    }

    fn push_event(&mut self, event: InterruptEvent) -> Result<(), ResolverError> {
        if self.event_len == EVENTS {
            return Err(self.record_rejection(ResolverError::EventQueueFull));
        }
        let index = (self.event_head + self.event_len) % EVENTS;
        self.events[index] = Some(event);
        self.event_len += 1;
        Ok(())
    }

    fn pop_event(&mut self) -> Option<InterruptEvent> {
        if self.event_len == 0 {
            return None;
        }
        let event = self.events[self.event_head].take();
        self.event_head = (self.event_head + 1) % EVENTS;
        self.event_len -= 1;
        if self.event_len == 0 {
            self.event_head = 0;
        }
        event
    }

    fn record_result<T>(&self, result: Result<T, ResolverError>) -> Result<T, ResolverError> {
        match result {
            Ok(value) => Ok(value),
            Err(error) => Err(self.record_rejection(error)),
        }
    }

    fn record_rejection(&self, error: ResolverError) -> ResolverError {
        let mut telemetry = self.rejection_telemetry.get();
        telemetry.record(error);
        self.rejection_telemetry.set(telemetry);
        error
    }
}

impl<const TIMERS: usize, const EVENTS: usize, const GPIO_WAITS: usize> Default
    for PicoInterruptResolver<TIMERS, EVENTS, GPIO_WAITS>
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{InterruptEvent, PicoInterruptResolver, ResolvedInterrupt, ResolverError};
    use crate::choreography::protocol::{
        BudgetExpired, GpioEdge, GpioWait, TimerSleepDone, TimerSleepUntil,
    };

    #[test]
    fn timer_irq_resolves_only_after_a_matching_sleep_request_is_due() {
        let mut resolver: PicoInterruptResolver<2, 2, 1> = PicoInterruptResolver::new();

        resolver
            .push_irq(InterruptEvent::TimerTick { tick: 9 })
            .expect("record tick without wait");
        assert_eq!(resolver.resolve_next(), Ok(None));

        let id = resolver
            .request_timer_sleep(TimerSleepUntil::new(10))
            .expect("register sleep");
        assert_eq!(id.raw(), 1);
        assert_eq!(resolver.resolve_next(), Ok(None));

        resolver
            .push_irq(InterruptEvent::TimerTick { tick: 10 })
            .expect("record due tick");
        assert_eq!(
            resolver.resolve_next(),
            Ok(Some(ResolvedInterrupt::TimerSleepDone(
                TimerSleepDone::new(10)
            )))
        );
        assert_eq!(resolver.resolve_next(), Ok(None));
    }

    #[test]
    fn gpio_edges_reject_until_a_wait_is_registered() {
        let mut resolver: PicoInterruptResolver<1, 2, 1> = PicoInterruptResolver::new();

        resolver
            .push_irq(InterruptEvent::GpioEdge { pin: 5, high: true })
            .expect("queue gpio edge");
        assert_eq!(
            resolver.resolve_next(),
            Err(ResolverError::UnsolicitedGpioEdge)
        );
        let telemetry = resolver.rejection_telemetry();
        assert_eq!(telemetry.unsolicited_event(), 1);
        assert_eq!(telemetry.total(), 1);

        resolver.request_gpio_edge(5).expect("wait for pin 5");
        resolver
            .push_irq(InterruptEvent::GpioEdge {
                pin: 5,
                high: false,
            })
            .expect("queue subscribed edge");
        assert_eq!(
            resolver.resolve_next(),
            Ok(Some(ResolvedInterrupt::GpioWaitSatisfied(GpioEdge::new(
                1, 5, false, 1
            ))))
        );
        let telemetry = resolver.rejection_telemetry();
        assert_eq!(telemetry.unsolicited_event(), 1);
        assert_eq!(telemetry.total(), 1);
    }

    #[test]
    fn gpio_wait_generation_mismatch_rejects_stale_registration() {
        let mut resolver: PicoInterruptResolver<1, 1, 2> = PicoInterruptResolver::new();
        resolver
            .request_gpio_wait(GpioWait::new(60, 7, 4, 2))
            .expect("register gpio wait");
        assert_eq!(
            resolver.request_gpio_wait(GpioWait::new(60, 7, 4, 1)),
            Err(ResolverError::StaleGpioWaitGeneration)
        );
        let telemetry = resolver.rejection_telemetry();
        assert_eq!(telemetry.stale_generation(), 1);
        assert_eq!(telemetry.total(), 1);
    }

    #[test]
    fn gpio_wait_table_full_and_conflict_are_bounded_rejections() {
        let mut full_resolver: PicoInterruptResolver<1, 1, 1> = PicoInterruptResolver::new();
        full_resolver
            .request_gpio_wait(GpioWait::new(60, 7, 4, 2))
            .expect("register first gpio wait");
        assert_eq!(
            full_resolver.request_gpio_wait(GpioWait::new(61, 8, 5, 2)),
            Err(ResolverError::GpioWaitTableFull)
        );
        let telemetry = full_resolver.rejection_telemetry();
        assert_eq!(telemetry.gpio_wait_table_full(), 1);
        assert_eq!(telemetry.total(), 1);

        let mut conflict_resolver: PicoInterruptResolver<1, 1, 2> = PicoInterruptResolver::new();
        conflict_resolver
            .request_gpio_wait(GpioWait::new(60, 7, 4, 2))
            .expect("register first gpio wait");
        assert_eq!(
            conflict_resolver.request_gpio_wait(GpioWait::new(61, 8, 4, 2)),
            Err(ResolverError::GpioWaitConflict)
        );
        let telemetry = conflict_resolver.rejection_telemetry();
        assert_eq!(telemetry.gpio_wait_conflict(), 1);
        assert_eq!(telemetry.total(), 1);
    }

    #[test]
    fn gpio_wait_fence_revokes_subscription_before_old_edge_can_progress() {
        let mut resolver: PicoInterruptResolver<1, 2, 2> = PicoInterruptResolver::new();
        resolver
            .request_gpio_wait(GpioWait::new(60, 7, 4, 2))
            .expect("register gpio wait");
        assert!(resolver.has_active_gpio_waits());
        assert_eq!(resolver.active_gpio_wait_count(), 1);

        assert_eq!(
            resolver.cancel_gpio_wait(7, 1),
            Err(ResolverError::StaleGpioWaitGeneration)
        );
        assert_eq!(
            resolver.cancel_gpio_wait(99, 2),
            Err(ResolverError::GpioWaitNotFound)
        );
        assert_eq!(resolver.fence_gpio_waits(), 1);
        assert!(!resolver.has_active_gpio_waits());

        resolver
            .push_irq(InterruptEvent::GpioEdge { pin: 4, high: true })
            .expect("queue stale edge after fence");
        assert_eq!(
            resolver.resolve_next(),
            Err(ResolverError::UnsolicitedGpioEdge)
        );
        let telemetry = resolver.rejection_telemetry();
        assert_eq!(telemetry.stale_generation(), 1);
        assert_eq!(telemetry.gpio_wait_not_found(), 1);
        assert_eq!(telemetry.unsolicited_event(), 1);
        assert_eq!(telemetry.total(), 3);
    }

    #[test]
    fn transport_ready_is_a_resolved_policy_signal_not_a_payload_authority() {
        let mut resolver: PicoInterruptResolver<1, 1, 1> = PicoInterruptResolver::new();
        resolver
            .push_irq(InterruptEvent::TransportRxReady {
                role: 1,
                lane: 2,
                label_hint: 65,
            })
            .expect("queue transport readiness");
        assert_eq!(
            resolver.resolve_next(),
            Ok(Some(ResolvedInterrupt::TransportRxReady {
                role: 1,
                lane: 2,
                label_hint: 65
            }))
        );
    }

    #[test]
    fn budget_timer_expiry_is_readiness_not_direct_kill() {
        let mut resolver: PicoInterruptResolver<1, 1, 1> = PicoInterruptResolver::new();
        resolver
            .push_irq(InterruptEvent::BudgetTimerExpired {
                run_id: 7,
                generation: 3,
            })
            .expect("queue budget timer expiry");

        assert_eq!(
            resolver.resolve_next(),
            Ok(Some(ResolvedInterrupt::BudgetExpired(BudgetExpired::new(
                7, 3
            ))))
        );
        assert_eq!(resolver.resolve_next(), Ok(None));
    }

    #[test]
    fn resolved_ready_facts_are_consumed_once() {
        let mut resolver: PicoInterruptResolver<2, 3, 1> = PicoInterruptResolver::new();

        resolver
            .request_timer_sleep(TimerSleepUntil::new(7))
            .expect("register timer wait");
        resolver
            .push_irq(InterruptEvent::TimerTick { tick: 7 })
            .expect("record due timer tick");
        assert_eq!(
            resolver.resolve_next(),
            Ok(Some(ResolvedInterrupt::TimerSleepDone(
                TimerSleepDone::new(7)
            )))
        );
        assert_eq!(resolver.resolve_next(), Ok(None));

        resolver.request_gpio_edge(4).expect("register GPIO wait");
        resolver
            .push_irq(InterruptEvent::GpioEdge { pin: 4, high: true })
            .expect("queue subscribed GPIO edge");
        assert_eq!(
            resolver.resolve_next(),
            Ok(Some(ResolvedInterrupt::GpioWaitSatisfied(GpioEdge::new(
                1, 4, true, 1
            ))))
        );
        assert_eq!(resolver.resolve_next(), Ok(None));

        resolver
            .push_irq(InterruptEvent::TransportRxReady {
                role: 2,
                lane: 16,
                label_hint: 65,
            })
            .expect("queue transport readiness");
        assert_eq!(
            resolver.resolve_next(),
            Ok(Some(ResolvedInterrupt::TransportRxReady {
                role: 2,
                lane: 16,
                label_hint: 65
            }))
        );
        assert_eq!(resolver.resolve_next(), Ok(None));
    }

    #[test]
    fn non_timer_irq_queue_is_bounded() {
        let mut resolver: PicoInterruptResolver<1, 1, 1> = PicoInterruptResolver::new();
        resolver
            .push_irq(InterruptEvent::TransportRxReady {
                role: 1,
                lane: 0,
                label_hint: 1,
            })
            .expect("queue first event");
        assert_eq!(
            resolver.push_irq(InterruptEvent::TransportRxReady {
                role: 1,
                lane: 0,
                label_hint: 2,
            }),
            Err(ResolverError::EventQueueFull)
        );
        let telemetry = resolver.rejection_telemetry();
        assert_eq!(telemetry.event_queue_full(), 1);
        assert_eq!(telemetry.total(), 1);
    }
}
