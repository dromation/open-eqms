//! Open-EQMS Event Engine implementation for SPEC-002.
//!
//! This crate owns the Business Event Log only. It records immutable business
//! facts as append-only Events, validates them against inert Event Type
//! metadata, and makes them readable by identity or strictly increasing append
//! sequence through an abstract storage-provider boundary.
//!
//! It intentionally does not implement rules, processes, queries, transactions,
//! GUI, AI, synchronization, concrete databases, content packages, security
//! authorization, statistics, KPIs, package loading, or Object Runtime calls.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod errors;
pub mod storage;
pub mod types;

mod validation;

use crate::errors::{EventEngineError, EventEngineResult, MetadataError};
use crate::storage::{AppendSequenceGenerator, EventIdGenerator, StorageProvider};
use crate::types::{
    AppendSequence, CausationId, CorrelationId, EventId, EventRecord, EventSource, EventTimestamp,
    EventTypeDefinition, EventTypeRef, ObjectId, PropertyValue,
};
use crate::validation::{validate_append_request, validate_metadata};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

/// Event Engine facade implementing the SPEC-002 public API.
pub struct EventEngine<S, I, Q>
where
    S: StorageProvider,
    I: EventIdGenerator,
    Q: AppendSequenceGenerator,
{
    storage: Arc<Mutex<S>>,
    id_generator: Arc<Mutex<I>>,
    sequence_generator: Arc<Mutex<Q>>,
}

impl<S, I, Q> Clone for EventEngine<S, I, Q>
where
    S: StorageProvider,
    I: EventIdGenerator,
    Q: AppendSequenceGenerator,
{
    fn clone(&self) -> Self {
        Self {
            storage: Arc::clone(&self.storage),
            id_generator: Arc::clone(&self.id_generator),
            sequence_generator: Arc::clone(&self.sequence_generator),
        }
    }
}

impl<S, I, Q> EventEngine<S, I, Q>
where
    S: StorageProvider,
    I: EventIdGenerator,
    Q: AppendSequenceGenerator,
{
    /// Creates an Event Engine over an abstract storage provider and deterministic sources.
    pub fn new(storage: S, id_generator: I, sequence_generator: Q) -> Self {
        Self {
            storage: Arc::new(Mutex::new(storage)),
            id_generator: Arc::new(Mutex::new(id_generator)),
            sequence_generator: Arc::new(Mutex::new(sequence_generator)),
        }
    }

    /// Registers a new immutable Event Type schema version.
    pub fn register_event_type(&self, definition: EventTypeDefinition) -> EventEngineResult<()> {
        validate_metadata(&definition)?;

        let mut storage = self.lock_storage()?;
        if storage.get_type(&definition.type_ref)?.is_some() {
            return Err(EventEngineError::MalformedMetadata {
                failure: MetadataError::EventTypeAlreadyRegistered {
                    event_type: definition.type_ref,
                },
            });
        }

        storage.insert_type(definition)
    }

    /// Retrieves a registered Event Type metadata definition by exact name/version.
    pub fn get_event_type(
        &self,
        event_type: &EventTypeRef,
    ) -> EventEngineResult<EventTypeDefinition> {
        let storage = self.lock_storage()?;
        storage
            .get_type(event_type)?
            .ok_or_else(|| EventEngineError::EventTypeNotFound {
                event_type: event_type.clone(),
            })
    }

    /// Appends a new immutable Event after validation.
    pub fn append_event(
        &self,
        request: AppendEventRequest,
    ) -> EventEngineResult<AppendEventResult> {
        let mut storage = self.lock_storage()?;
        let definition = storage.get_type(&request.event_type)?.ok_or_else(|| {
            EventEngineError::EventTypeNotFound {
                event_type: request.event_type.clone(),
            }
        })?;

        validate_append_request(&definition, &request)?;

        let event_id = self.next_id()?;
        if storage.event_exists(&event_id)? {
            return Err(EventEngineError::DuplicateIdentity {
                event_id: event_id.clone(),
            });
        }

        let append_sequence = self.next_sequence()?;
        let record = EventRecord::new(
            event_id.clone(),
            request.event_type,
            append_sequence,
            request.occurred_at,
            request.recorded_at,
            request.source,
            request.object_refs,
            request.payload,
            request.correlation_id,
            request.causation_id,
        );

        storage.append_event(record.clone())?;
        Ok(AppendEventResult {
            event_id,
            append_sequence,
            record,
        })
    }

    /// Reads one immutable Event by identity.
    pub fn read_event(&self, event_id: &EventId) -> EventEngineResult<EventRecord> {
        let storage = self.lock_storage()?;
        storage
            .get_event(event_id)?
            .ok_or_else(|| EventEngineError::EventNotFound {
                event_id: event_id.clone(),
            })
    }

    /// Reads a bounded sequence range in strictly ascending Append Sequence order.
    pub fn read_sequence_range(
        &self,
        start: AppendSequence,
        max: usize,
    ) -> EventEngineResult<EventRange> {
        if start.is_zero() || max == 0 {
            return Err(EventEngineError::InvalidAppendSequenceRange {
                start,
                max,
                reason: "start must be non-zero and max must be greater than zero".to_owned(),
            });
        }

        let storage = self.lock_storage()?;
        let events = storage.read_sequence_range(start, max)?;
        let next_start = events.last().map(|record| record.append_sequence.next());
        Ok(EventRange { events, next_start })
    }

    fn lock_storage(&self) -> EventEngineResult<std::sync::MutexGuard<'_, S>> {
        self.storage
            .lock()
            .map_err(|_| EventEngineError::StorageProviderFailure {
                message: "storage lock poisoned".to_owned(),
            })
    }

    fn next_id(&self) -> EventEngineResult<EventId> {
        let mut generator =
            self.id_generator
                .lock()
                .map_err(|_| EventEngineError::StorageProviderFailure {
                    message: "event id generator lock poisoned".to_owned(),
                })?;
        Ok(generator.next_id())
    }

    fn next_sequence(&self) -> EventEngineResult<AppendSequence> {
        let mut generator = self.sequence_generator.lock().map_err(|_| {
            EventEngineError::AppendSequenceAllocationFailure {
                message: "append sequence generator lock poisoned".to_owned(),
            }
        })?;
        Ok(generator.next_sequence())
    }
}

/// Request to append one immutable Event.
#[derive(Clone, Debug, PartialEq)]
pub struct AppendEventRequest {
    /// Event Type reference to validate against.
    pub event_type: EventTypeRef,
    /// Payload values keyed by language-neutral field name.
    pub payload: BTreeMap<String, PropertyValue>,
    /// Caller-supplied timestamp describing when the fact happened.
    pub occurred_at: EventTimestamp,
    /// Caller-supplied timestamp describing when the Runtime accepted the Event.
    pub recorded_at: EventTimestamp,
    /// Caller-supplied opaque source reference.
    pub source: EventSource,
    /// Opaque Object references the fact concerns, without existence checks.
    pub object_refs: BTreeSet<ObjectId>,
    /// Optional opaque correlation reference.
    pub correlation_id: Option<CorrelationId>,
    /// Optional opaque causation reference.
    pub causation_id: Option<CausationId>,
}

/// Result returned by a successful Event append operation.
#[derive(Clone, Debug, PartialEq)]
pub struct AppendEventResult {
    /// Newly assigned Event identifier.
    pub event_id: EventId,
    /// Newly assigned append sequence.
    pub append_sequence: AppendSequence,
    /// Full newly stored Event record.
    pub record: EventRecord,
}

/// Bounded sequential read result.
#[derive(Clone, Debug, PartialEq)]
pub struct EventRange {
    /// Events read in ascending Append Sequence order.
    pub events: Vec<EventRecord>,
    /// Continuation point for the next range read, when at least one Event was returned.
    pub next_start: Option<AppendSequence>,
}

#[cfg(test)]
mod tests;
