use crate::clock::DeterministicClock;
use crate::crypto::DemoCryptographicProvider;
use crate::ids::{
    DeterministicAppendSequences, DeterministicEventIds, DeterministicObjectIds,
    DeterministicTransactionIds,
};
use open_eqms_event_engine::storage::{AppendSequenceGenerator, EventIdGenerator};
use open_eqms_object_runtime::storage::ObjectIdGenerator;
use open_eqms_transaction_engine::crypto::CryptographicProvider;
use open_eqms_transaction_engine::storage::TransactionIdGenerator;

#[test]
fn deterministic_generators_replay_identical_sequences() {
    fn scenario() -> Vec<String> {
        let mut object_ids = DeterministicObjectIds::new("asset");
        let mut event_ids = DeterministicEventIds::new("event");
        let mut transaction_ids = DeterministicTransactionIds::new("txn");
        let mut append_sequences = DeterministicAppendSequences::new();

        vec![
            object_ids.next_id().to_string(),
            object_ids.next_id().to_string(),
            event_ids.next_id().to_string(),
            event_ids.next_id().to_string(),
            transaction_ids.next_id().to_string(),
            transaction_ids.next_id().to_string(),
            append_sequences.next_sequence().value().to_string(),
            append_sequences.next_sequence().value().to_string(),
        ]
    }

    assert_eq!(scenario(), scenario());
}

#[test]
fn deterministic_clock_replays_fixed_timestamps() {
    fn scenario() -> Vec<String> {
        let mut clock = DeterministicClock::new(&["2026-07-15T10:00:00Z", "2026-07-15T10:00:01Z"]);

        vec![
            clock.next_timestamp(),
            clock.next_event_timestamp().as_str().to_owned(),
            clock.next_transaction_timestamp().as_str().to_owned(),
        ]
    }

    assert_eq!(scenario(), scenario());
    assert_eq!(
        scenario(),
        vec![
            "2026-07-15T10:00:00Z".to_owned(),
            "2026-07-15T10:00:01Z".to_owned(),
            "2026-07-15T10:00:00Z".to_owned()
        ]
    );
}

#[test]
fn demo_crypto_hash_is_deterministic_and_demo_only() {
    let mut first = DemoCryptographicProvider::default();
    let mut second = DemoCryptographicProvider::default();

    let first_hash = first.hash(b"same canonical content").unwrap();
    let second_hash = second.hash(b"same canonical content").unwrap();

    assert_eq!(first_hash, second_hash);
    assert!(first_hash.as_str().starts_with("demo-noncrypto-"));
}
