use crate::choreography::protocol::{TimerSleepDone, TimerSleepUntil};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerError {
    TableFull,
    InvalidSleepId,
    UnknownSleepId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerSleepId(u8);

impl TimerSleepId {
    pub const fn raw(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TimerSleepEntry {
    id: TimerSleepId,
    tick: u64,
}

pub struct TimerSleepTable<const N: usize> {
    slots: [Option<TimerSleepEntry>; N],
}

impl<const N: usize> TimerSleepTable<N> {
    pub const fn new() -> Self {
        Self { slots: [None; N] }
    }

    pub fn request_sleep(&mut self, sleep: TimerSleepUntil) -> Result<TimerSleepId, TimerError> {
        let index = self
            .slots
            .iter()
            .position(Option::is_none)
            .ok_or(TimerError::TableFull)?;
        let id = self.allocate_id()?;
        self.slots[index] = Some(TimerSleepEntry {
            id,
            tick: sleep.tick(),
        });
        Ok(id)
    }

    pub fn cancel(&mut self, id: TimerSleepId) -> Result<(), TimerError> {
        if id.raw() == 0 {
            return Err(TimerError::InvalidSleepId);
        }
        for slot in &mut self.slots {
            if slot.is_some_and(|entry| entry.id == id) {
                *slot = None;
                return Ok(());
            }
        }
        Err(TimerError::UnknownSleepId)
    }

    pub fn poll(&mut self, now_tick: u64) -> Option<TimerSleepDone> {
        let ready = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let entry = (*entry)?;
                (entry.tick <= now_tick).then_some((index, entry.tick))
            })
            .min_by_key(|(_, tick)| *tick);
        let (index, tick) = ready?;
        self.slots[index] = None;
        Some(TimerSleepDone::new(tick))
    }

    fn allocate_id(&self) -> Result<TimerSleepId, TimerError> {
        for candidate in 1..=u8::MAX {
            if self
                .slots
                .iter()
                .flatten()
                .all(|entry| entry.id.raw() != candidate)
            {
                return Ok(TimerSleepId(candidate));
            }
        }
        Err(TimerError::TableFull)
    }
}

impl<const N: usize> Default for TimerSleepTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{TimerError, TimerSleepTable};
    use crate::choreography::protocol::TimerSleepUntil;

    #[test]
    fn sleep_request_polls_when_tick_is_reached() {
        let mut timers: TimerSleepTable<2> = TimerSleepTable::new();
        let id = timers
            .request_sleep(TimerSleepUntil::new(10))
            .expect("schedule sleep");
        assert_eq!(id.raw(), 1);
        assert_eq!(timers.poll(9), None);
        assert_eq!(
            timers.poll(10).expect("sleep ready").tick(),
            TimerSleepUntil::new(10).tick()
        );
        assert_eq!(timers.poll(11), None);
    }

    #[test]
    fn sleep_table_rejects_when_full() {
        let mut timers: TimerSleepTable<1> = TimerSleepTable::new();
        timers
            .request_sleep(TimerSleepUntil::new(10))
            .expect("schedule first sleep");
        assert_eq!(
            timers.request_sleep(TimerSleepUntil::new(11)),
            Err(TimerError::TableFull)
        );
    }
}
