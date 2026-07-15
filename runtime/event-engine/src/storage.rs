//! Storage-provider and identity-generation boundaries for the Event Engine.

use crate::errors::EventEngineResult;
use crate::types::{AppendSequence, EventId, EventRecord, EventTypeDefinition, EventTypeRef};

/// Abstract append-only persistence boundary used by the Event Engine.
///
/// Implementations may use any backing store as long as they preserve this
/// contract. The Event Engine must not depend on concrete database behavior.
pub trait StorageProvider {
    /// Retrieves an Event Type definition by exact name/version reference.
    fn get_type(&self, event_type: &EventTypeRef)
        -> EventEngineResult<Option<EventTypeDefinition>>;

    /// Stores a new immutable Event Type schema version.
    fn insert_type(&mut self, definition: EventTypeDefinition) -> EventEngineResult<()>;

    /// Retrieves an Event record by identity.
    fn get_event(&self, event_id: &EventId) -> EventEngineResult<Option<EventRecord>>;

    /// Reports whether an Event identity exists.
    fn event_exists(&self, event_id: &EventId) -> EventEngineResult<bool>;

    /// Appends a new Event record and rejects duplicate identities or append sequences.
    fn append_event(&mut self, record: EventRecord) -> EventEngineResult<()>;

    /// Reads Events in ascending Append Sequence order, starting at `start`.
    fn read_sequence_range(
        &self,
        start: AppendSequence,
        max: usize,
    ) -> EventEngineResult<Vec<EventRecord>>;
}

/// Event identity source used by append operations.
///
/// Production deployments may provide a UUIDv7/ULID-class generator. Tests use a
/// deterministic implementation so replayed API sequences produce identical logs.
pub trait EventIdGenerator {
    /// Returns the next opaque Event identifier.
    fn next_id(&mut self) -> EventId;
}

/// Append sequence source used by append operations.
///
/// Production deployments may derive this from a durable log cursor. Tests use a
/// deterministic implementation so append order can be replayed exactly.
pub trait AppendSequenceGenerator {
    /// Returns the next append sequence marker.
    fn next_sequence(&mut self) -> AppendSequence;
}
