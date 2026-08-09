use open_eqms_event_engine::types::EventTimestamp;
use open_eqms_transaction_engine::types::TransactionTimestamp;
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct DeterministicClock {
    scripted: VecDeque<String>,
    fallback_offset_seconds: u64,
}

impl DeterministicClock {
    pub fn new(scripted: &[&str]) -> Self {
        Self {
            scripted: scripted
                .iter()
                .map(|timestamp| (*timestamp).to_owned())
                .collect(),
            fallback_offset_seconds: 0,
        }
    }

    pub fn demo() -> Self {
        Self::new(&[
            "2026-07-15T10:00:00Z",
            "2026-07-15T10:00:01Z",
            "2026-07-15T10:00:02Z",
            "2026-07-15T10:00:03Z",
            "2026-07-15T10:00:04Z",
            "2026-07-15T10:00:05Z",
            "2026-07-15T10:00:06Z",
            "2026-07-15T10:00:07Z",
            "2026-07-15T10:00:08Z",
            "2026-07-15T10:00:09Z",
        ])
    }

    pub fn next_timestamp(&mut self) -> String {
        if let Some(timestamp) = self.scripted.pop_front() {
            return timestamp;
        }

        let minute = self.fallback_offset_seconds / 60;
        let second = self.fallback_offset_seconds % 60;
        self.fallback_offset_seconds += 1;
        format!("2026-07-15T10:{minute:02}:{second:02}Z")
    }

    pub fn next_event_timestamp(&mut self) -> EventTimestamp {
        EventTimestamp::new(self.next_timestamp())
    }

    pub fn next_transaction_timestamp(&mut self) -> TransactionTimestamp {
        TransactionTimestamp::new(self.next_timestamp())
    }
}

impl Default for DeterministicClock {
    fn default() -> Self {
        Self::demo()
    }
}
