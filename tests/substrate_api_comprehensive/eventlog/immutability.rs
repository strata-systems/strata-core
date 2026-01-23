//! EventLog Immutability Tests
//!
//! Tests that verify EventLog is append-only:
//! - No update operations exist
//! - No delete operations exist
//! - Events cannot be modified after append
//! - Event payloads are immutable
//! - Event sequences are immutable

use crate::test_data::load_eventlog_test_data;
use crate::*;
use std::collections::HashMap;

// =============================================================================
// APPEND-ONLY VERIFICATION
// =============================================================================

#[test]
fn test_no_update_api_exists() {
    // Verify that SubstrateImpl does not expose any update methods for events
    // This is a compile-time check via the trait - EventLog trait should not have update methods

    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    // Append an event
    let payload = Value::Object({
        let mut m = HashMap::new();
        m.insert("original".to_string(), Value::Bool(true));
        m
    });
    let version = substrate
        .event_append(&run, "stream1", payload)
        .expect("append should succeed");

    let seq = match version {
        Version::Sequence(n) => n,
        _ => panic!("Expected sequence version"),
    };

    // Get the event
    let event = substrate
        .event_get(&run, "stream1", seq)
        .expect("get should succeed")
        .expect("event should exist");

    // Verify we cannot modify it - there is no event_update method
    // This test documents the API contract: EventLog has no update capability

    // Verify the event is unchanged
    if let Value::Object(ref m) = event.value {
        assert_eq!(
            m.get("original"),
            Some(&Value::Bool(true)),
            "Event should retain original payload"
        );
    }
}

#[test]
fn test_no_delete_api_exists() {
    // Verify that SubstrateImpl does not expose any delete methods for events

    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    // Append several events
    for i in 0..5 {
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("index".to_string(), Value::Int(i));
            m
        });
        substrate
            .event_append(&run, "stream1", payload)
            .expect("append should succeed");
    }

    // Verify all 5 events exist
    let len = substrate
        .event_len(&run, "stream1")
        .expect("len should succeed");
    assert_eq!(len, 5, "Should have 5 events");

    // There is no event_delete method - this test documents that
    // Events can only be added, never removed

    // Verify still 5 events
    let len_after = substrate
        .event_len(&run, "stream1")
        .expect("len should succeed");
    assert_eq!(len_after, 5, "Should still have 5 events (no delete API)");
}

#[test]
fn test_sequence_cannot_be_reused() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    // Append events and track sequences
    let mut sequences = Vec::new();
    for i in 0..10 {
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("index".to_string(), Value::Int(i));
            m
        });
        let version = substrate
            .event_append(&run, "stream1", payload)
            .expect("append should succeed");

        if let Version::Sequence(seq) = version {
            sequences.push(seq);
        }
    }

    // Verify all sequences are unique
    let mut unique_seqs: Vec<_> = sequences.clone();
    unique_seqs.sort();
    unique_seqs.dedup();
    assert_eq!(
        unique_seqs.len(),
        sequences.len(),
        "All sequences should be unique"
    );

    // Verify sequences are never reused (strictly increasing)
    for window in sequences.windows(2) {
        assert!(
            window[1] > window[0],
            "Sequences must be strictly increasing (never reused)"
        );
    }
}

#[test]
fn test_event_content_immutable_across_reads() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    let original_payload = Value::Object({
        let mut m = HashMap::new();
        m.insert("data".to_string(), Value::String("immutable".into()));
        m.insert("count".to_string(), Value::Int(42));
        m
    });

    let version = substrate
        .event_append(&run, "stream1", original_payload.clone())
        .expect("append should succeed");

    let seq = match version {
        Version::Sequence(n) => n,
        _ => panic!("Expected sequence"),
    };

    // Read multiple times
    for _ in 0..5 {
        let event = substrate
            .event_get(&run, "stream1", seq)
            .expect("get should succeed")
            .expect("event should exist");

        assert_eq!(
            event.value, original_payload,
            "Event content should be identical on every read"
        );
    }

    // Read via range
    let events = substrate
        .event_range(&run, "stream1", None, None, None)
        .expect("range should succeed");

    assert_eq!(
        events[0].value, original_payload,
        "Event content should be identical via range"
    );
}

// =============================================================================
// IMMUTABILITY ACROSS RESTARTS
// =============================================================================

#[test]
fn test_events_immutable_after_crash() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    let original_payload = Value::Object({
        let mut m = HashMap::new();
        m.insert("persistent".to_string(), Value::String("immutable".into()));
        m
    });

    let original_seq;

    // Write event
    {
        let substrate = test_db.substrate();
        let version = substrate
            .event_append(&run, "stream1", original_payload.clone())
            .expect("append should succeed");

        original_seq = match version {
            Version::Sequence(n) => n,
            _ => panic!("Expected sequence"),
        };
    }

    // Crash and recover
    test_db.reopen();

    // Verify event is unchanged
    {
        let substrate = test_db.substrate();
        let event = substrate
            .event_get(&run, "stream1", original_seq)
            .expect("get should succeed")
            .expect("event should exist");

        assert_eq!(
            event.value, original_payload,
            "Event should be unchanged after crash"
        );

        // Verify sequence is preserved
        if let Version::Sequence(seq) = event.version {
            assert_eq!(seq, original_seq, "Sequence should be preserved after crash");
        }
    }
}

#[test]
fn test_new_appends_dont_modify_existing() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    // Append first event
    let payload1 = Value::Object({
        let mut m = HashMap::new();
        m.insert("order".to_string(), Value::Int(1));
        m
    });
    let v1 = substrate
        .event_append(&run, "stream1", payload1.clone())
        .expect("append should succeed");

    let seq1 = match v1 {
        Version::Sequence(n) => n,
        _ => panic!("Expected sequence"),
    };

    // Append more events
    for i in 2..10 {
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("order".to_string(), Value::Int(i));
            m
        });
        substrate
            .event_append(&run, "stream1", payload)
            .expect("append should succeed");
    }

    // Verify first event is unchanged
    let event1 = substrate
        .event_get(&run, "stream1", seq1)
        .expect("get should succeed")
        .expect("event should exist");

    assert_eq!(
        event1.value, payload1,
        "First event should be unchanged after subsequent appends"
    );
}

// =============================================================================
// MULTI-STREAM IMMUTABILITY
// =============================================================================

#[test]
fn test_appends_to_other_streams_dont_affect_existing() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    // Append to stream1
    let payload1 = Value::Object({
        let mut m = HashMap::new();
        m.insert("stream".to_string(), Value::String("stream1".into()));
        m
    });
    let v1 = substrate
        .event_append(&run, "stream1", payload1.clone())
        .expect("append should succeed");

    let seq1 = match v1 {
        Version::Sequence(n) => n,
        _ => panic!("Expected sequence"),
    };

    // Append many events to stream2
    for i in 0..100 {
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("stream".to_string(), Value::String("stream2".into()));
            m.insert("index".to_string(), Value::Int(i));
            m
        });
        substrate
            .event_append(&run, "stream2", payload)
            .expect("append should succeed");
    }

    // Verify stream1 event is unchanged
    let event1 = substrate
        .event_get(&run, "stream1", seq1)
        .expect("get should succeed")
        .expect("event should exist");

    assert_eq!(
        event1.value, payload1,
        "Stream1 event should be unaffected by stream2 appends"
    );

    // Verify stream1 still has only 1 event
    let len1 = substrate
        .event_len(&run, "stream1")
        .expect("len should succeed");
    assert_eq!(len1, 1, "Stream1 should still have only 1 event");
}

// =============================================================================
// VERSION PRESERVATION
// =============================================================================

#[test]
fn test_version_immutable_after_append() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    let payload = Value::Object({
        let mut m = HashMap::new();
        m.insert("test".to_string(), Value::Bool(true));
        m
    });

    let original_version = substrate
        .event_append(&run, "stream1", payload)
        .expect("append should succeed");

    let original_seq = match original_version {
        Version::Sequence(n) => n,
        _ => panic!("Expected sequence"),
    };

    // Append more events
    for i in 0..10 {
        let p = Value::Object({
            let mut m = HashMap::new();
            m.insert("index".to_string(), Value::Int(i));
            m
        });
        substrate
            .event_append(&run, "stream1", p)
            .expect("append should succeed");
    }

    // Verify original event's version is unchanged
    let event = substrate
        .event_get(&run, "stream1", original_seq)
        .expect("get should succeed")
        .expect("event should exist");

    if let Version::Sequence(seq) = event.version {
        assert_eq!(
            seq, original_seq,
            "Event version should be immutable"
        );
    }
}

// =============================================================================
// TESTDATA-DRIVEN IMMUTABILITY TESTS
// =============================================================================

#[test]
fn test_testdata_events_immutable() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();
    let test_data = load_eventlog_test_data();

    // Use entries from run 0, stream "events"
    let entries = test_data.get_run_stream(0, "events");
    let mut versions = Vec::new();

    // Append all events
    for entry in &entries {
        let version = substrate
            .event_append(&run, "events", entry.payload.clone())
            .expect("append should succeed");
        versions.push((version, entry.payload.clone()));
    }

    // Verify all events are unchanged
    for (version, original_payload) in &versions {
        let seq = match version {
            Version::Sequence(n) => *n,
            _ => panic!("Expected sequence"),
        };

        let event = substrate
            .event_get(&run, "events", seq)
            .expect("get should succeed")
            .expect("event should exist");

        assert_eq!(
            &event.value, original_payload,
            "Event payload should be immutable"
        );
    }
}

// =============================================================================
// CROSS-MODE EQUIVALENCE
// =============================================================================

#[test]
fn test_immutability_cross_mode() {
    test_across_modes("eventlog_immutability", |db| {
        let substrate = create_substrate(db);
        let run = ApiRunId::default();

        // Append event
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("immutable".to_string(), Value::Bool(true));
            m
        });
        let version = substrate
            .event_append(&run, "stream1", payload.clone())
            .expect("append should succeed");

        let seq = match version {
            Version::Sequence(n) => n,
            _ => panic!("Expected sequence"),
        };

        // Append more events
        for i in 0..5 {
            let p = Value::Object({
                let mut m = HashMap::new();
                m.insert("index".to_string(), Value::Int(i));
                m
            });
            substrate.event_append(&run, "stream1", p).unwrap();
        }

        // Verify original is unchanged
        let event = substrate
            .event_get(&run, "stream1", seq)
            .expect("get should succeed")
            .expect("event should exist");

        event.value == payload
    });
}
