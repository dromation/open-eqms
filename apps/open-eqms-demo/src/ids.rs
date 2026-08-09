use open_eqms_event_engine::storage::{AppendSequenceGenerator, EventIdGenerator};
use open_eqms_event_engine::types::{AppendSequence, EventId};
use open_eqms_object_runtime::storage::ObjectIdGenerator;
use open_eqms_runtime_contracts::ObjectId;
use open_eqms_transaction_engine::storage::TransactionIdGenerator;
use open_eqms_transaction_engine::types::TransactionId;

#[derive(Clone, Debug)]
pub struct DeterministicObjectIds {
    prefix: String,
    next: u64,
}

impl DeterministicObjectIds {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            next: 1,
        }
    }

    pub fn with_start(prefix: impl Into<String>, next: u64) -> Self {
        Self {
            prefix: prefix.into(),
            next,
        }
    }
}

impl ObjectIdGenerator for DeterministicObjectIds {
    fn next_id(&mut self) -> ObjectId {
        let id = ObjectId::new(format!("{}-{:04}", self.prefix, self.next));
        self.next += 1;
        id
    }
}

#[derive(Clone, Debug)]
pub struct DeterministicEventIds {
    prefix: String,
    next: u64,
}

impl DeterministicEventIds {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            next: 1,
        }
    }

    pub fn with_start(prefix: impl Into<String>, next: u64) -> Self {
        Self {
            prefix: prefix.into(),
            next,
        }
    }
}

impl EventIdGenerator for DeterministicEventIds {
    fn next_id(&mut self) -> EventId {
        let id = EventId::new(format!("{}-{:04}", self.prefix, self.next));
        self.next += 1;
        id
    }
}

#[derive(Clone, Debug)]
pub struct DeterministicTransactionIds {
    prefix: String,
    next: u64,
}

impl DeterministicTransactionIds {
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            next: 1,
        }
    }

    pub fn with_start(prefix: impl Into<String>, next: u64) -> Self {
        Self {
            prefix: prefix.into(),
            next,
        }
    }
}

impl TransactionIdGenerator for DeterministicTransactionIds {
    fn next_id(&mut self) -> TransactionId {
        let id = TransactionId::new(format!("{}-{:04}", self.prefix, self.next));
        self.next += 1;
        id
    }
}

#[derive(Clone, Debug)]
pub struct DeterministicAppendSequences {
    next: u64,
}

impl DeterministicAppendSequences {
    pub fn new() -> Self {
        Self { next: 1 }
    }

    pub fn with_start(next: u64) -> Self {
        Self { next }
    }
}

impl Default for DeterministicAppendSequences {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendSequenceGenerator for DeterministicAppendSequences {
    fn next_sequence(&mut self) -> AppendSequence {
        let sequence = AppendSequence::new(self.next);
        self.next += 1;
        sequence
    }
}
