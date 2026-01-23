//! EventLog Recovery Invariants Tests
//!
//! Tests recovery invariants through the Substrate API layer:
//!
//! - **R1**: Deterministic recovery - same log produces same state
//! - **R2**: Idempotent recovery - multiple recoveries produce identical state
//! - **R3**: May drop uncommitted - in-flight events may be lost on crash
//! - **R4**: No drop committed - committed events must survive crash
//! - **R5**: No invent data - recovery cannot create events that were never appended
//! - **R6**: Prefix consistency - recovered state is a valid prefix of events
//!
//! Additional invariants specific to EventLog:
//! - **E1**: Sequence ordering preserved after recovery
//! - **E2**: Stream isolation preserved after recovery
//! - **E3**: Hash chain integrity (if exposed)

use crate::*;
use std::collections::{HashMap, HashSet};

// =============================================================================
// R1: DETERMINISTIC RECOVERY
// Same log produces same state every replay
// =============================================================================

#[test]
fn test_r1_deterministic_basic() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    // Write some events
    {
        let substrate = test_db.substrate();
        for i in 0..10 {
            let payload = Value::Object({
                let mut m = HashMap::new();
                m.insert("index".to_string(), Value::Int(i));
                m
            });
            substrate
                .event_append(&run, "stream1", payload)
                .expect("append should succeed");
        }
    }

    // First recovery
    test_db.reopen();
    let snapshot1: Vec<_> = {
        let substrate = test_db.substrate();
        substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed")
            .into_iter()
            .map(|e| (e.version.clone(), e.value.clone()))
            .collect()
    };

    // Second recovery
    test_db.reopen();
    let snapshot2: Vec<_> = {
        let substrate = test_db.substrate();
        substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed")
            .into_iter()
            .map(|e| (e.version.clone(), e.value.clone()))
            .collect()
    };

    assert_eq!(
        snapshot1, snapshot2,
        "R1: Two recoveries should produce identical state"
    );
}

#[test]
fn test_r1_deterministic_multiple_streams() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    // Write to multiple streams
    {
        let substrate = test_db.substrate();
        for i in 0..5 {
            let payload1 = Value::Object({
                let mut m = HashMap::new();
                m.insert("stream".to_string(), Value::String("s1".into()));
                m.insert("index".to_string(), Value::Int(i));
                m
            });
            substrate
                .event_append(&run, "stream1", payload1)
                .expect("append should succeed");

            let payload2 = Value::Object({
                let mut m = HashMap::new();
                m.insert("stream".to_string(), Value::String("s2".into()));
                m.insert("index".to_string(), Value::Int(i));
                m
            });
            substrate
                .event_append(&run, "stream2", payload2)
                .expect("append should succeed");
        }
    }

    // First recovery
    test_db.reopen();
    let (len1_a, len2_a) = {
        let substrate = test_db.substrate();
        (
            substrate.event_len(&run, "stream1").unwrap(),
            substrate.event_len(&run, "stream2").unwrap(),
        )
    };

    // Second recovery
    test_db.reopen();
    let (len1_b, len2_b) = {
        let substrate = test_db.substrate();
        (
            substrate.event_len(&run, "stream1").unwrap(),
            substrate.event_len(&run, "stream2").unwrap(),
        )
    };

    assert_eq!(len1_a, len1_b, "R1: stream1 count should be deterministic");
    assert_eq!(len2_a, len2_b, "R1: stream2 count should be deterministic");
}

// =============================================================================
// R2: IDEMPOTENT RECOVERY
// Multiple recoveries produce identical state
// =============================================================================

#[test]
fn test_r2_idempotent_recovery() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    // Write events
    {
        let substrate = test_db.substrate();
        for i in 0..20 {
            let payload = Value::Object({
                let mut m = HashMap::new();
                m.insert("value".to_string(), Value::Int(i));
                m
            });
            substrate
                .event_append(&run, "stream1", payload)
                .expect("append should succeed");
        }
    }

    // Multiple recoveries
    for cycle in 0..5 {
        test_db.reopen();
        let substrate = test_db.substrate();

        let len = substrate
            .event_len(&run, "stream1")
            .expect("len should succeed");

        assert_eq!(
            len, 20,
            "R2: Recovery cycle {} should have same event count",
            cycle
        );

        let latest = substrate
            .event_latest_sequence(&run, "stream1")
            .expect("latest should succeed");

        assert!(
            latest.is_some(),
            "R2: Recovery cycle {} should have latest sequence",
            cycle
        );
    }
}

// =============================================================================
// R4: NO DROP COMMITTED
// Committed events must survive crash
// =============================================================================

#[test]
fn test_r4_all_committed_survive() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    let mut committed_payloads = Vec::new();

    // Write events and track them
    {
        let substrate = test_db.substrate();
        for i in 0..25 {
            let payload = Value::Object({
                let mut m = HashMap::new();
                m.insert("committed".to_string(), Value::Int(i));
                m.insert("marker".to_string(), Value::String(format!("event_{}", i)));
                m
            });
            substrate
                .event_append(&run, "stream1", payload.clone())
                .expect("append should succeed");
            committed_payloads.push(payload);
        }
    }

    // Crash and recover
    test_db.reopen();

    // Verify all committed events survive
    {
        let substrate = test_db.substrate();
        let events = substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed");

        assert_eq!(
            events.len(),
            committed_payloads.len(),
            "R4: All {} committed events should survive",
            committed_payloads.len()
        );

        for (event, expected_payload) in events.iter().zip(committed_payloads.iter()) {
            assert_eq!(
                &event.value, expected_payload,
                "R4: Event payload should match"
            );
        }
    }
}

#[test]
fn test_r4_committed_survives_multiple_runs() {
    let mut test_db = TestDb::new_buffered();
    let run1 = ApiRunId::new();
    let run2 = ApiRunId::new();

    // Write to multiple runs
    {
        let substrate = test_db.substrate();

        let payload1 = Value::Object({
            let mut m = HashMap::new();
            m.insert("run".to_string(), Value::String("run1".into()));
            m
        });
        substrate
            .event_append(&run1, "stream1", payload1)
            .expect("append should succeed");

        let payload2 = Value::Object({
            let mut m = HashMap::new();
            m.insert("run".to_string(), Value::String("run2".into()));
            m
        });
        substrate
            .event_append(&run2, "stream1", payload2)
            .expect("append should succeed");
    }

    // Crash and recover
    test_db.reopen();

    // Both runs should survive
    {
        let substrate = test_db.substrate();

        let events1 = substrate
            .event_range(&run1, "stream1", None, None, None)
            .expect("range should succeed");
        let events2 = substrate
            .event_range(&run2, "stream1", None, None, None)
            .expect("range should succeed");

        assert_eq!(events1.len(), 1, "R4: run1 event should survive");
        assert_eq!(events2.len(), 1, "R4: run2 event should survive");

        if let Value::Object(ref m) = events1[0].value {
            assert_eq!(
                m.get("run"),
                Some(&Value::String("run1".into())),
                "R4: run1 event should have correct payload"
            );
        }
    }
}

// =============================================================================
// R5: NO INVENT DATA
// Recovery cannot create events that were never appended
// =============================================================================

#[test]
fn test_r5_no_invented_events() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    let mut appended_markers: HashSet<String> = HashSet::new();

    // Write events with unique markers
    {
        let substrate = test_db.substrate();
        for i in 0..15 {
            let marker = format!("unique_marker_{}", i);
            appended_markers.insert(marker.clone());

            let payload = Value::Object({
                let mut m = HashMap::new();
                m.insert("marker".to_string(), Value::String(marker));
                m
            });
            substrate
                .event_append(&run, "stream1", payload)
                .expect("append should succeed");
        }
    }

    // Crash and recover
    test_db.reopen();

    // Verify no invented events
    {
        let substrate = test_db.substrate();
        let events = substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed");

        for event in &events {
            if let Value::Object(ref m) = event.value {
                if let Some(Value::String(marker)) = m.get("marker") {
                    assert!(
                        appended_markers.contains(marker.as_str()),
                        "R5: Found invented event with marker '{}'",
                        marker
                    );
                }
            }
        }

        // Should not have more events than we appended
        assert!(
            events.len() <= appended_markers.len(),
            "R5: Should not have more events ({}) than appended ({})",
            events.len(),
            appended_markers.len()
        );
    }
}

#[test]
fn test_r5_no_invented_streams() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    let created_streams = vec!["stream_a", "stream_b", "stream_c"];

    // Write to known streams
    {
        let substrate = test_db.substrate();
        for stream in &created_streams {
            let payload = Value::Object({
                let mut m = HashMap::new();
                m.insert("stream".to_string(), Value::String(stream.to_string()));
                m
            });
            substrate
                .event_append(&run, stream, payload)
                .expect("append should succeed");
        }
    }

    // Crash and recover
    test_db.reopen();

    // Verify no invented streams (by checking non-existent stream)
    {
        let substrate = test_db.substrate();

        // Streams we never created should be empty
        let invented_len = substrate
            .event_len(&run, "invented_stream")
            .expect("len should succeed");
        assert_eq!(
            invented_len, 0,
            "R5: Invented stream should have 0 events"
        );

        // Created streams should have events
        for stream in &created_streams {
            let len = substrate.event_len(&run, stream).expect("len should succeed");
            assert_eq!(len, 1, "R5: Created stream '{}' should have 1 event", stream);
        }
    }
}

// =============================================================================
// R6: PREFIX CONSISTENCY
// Recovered state is a valid prefix of operations
// =============================================================================

#[test]
fn test_r6_prefix_consistency() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    // Write events with sequential markers
    {
        let substrate = test_db.substrate();
        for i in 0..20 {
            let payload = Value::Object({
                let mut m = HashMap::new();
                m.insert("sequence".to_string(), Value::Int(i));
                m
            });
            substrate
                .event_append(&run, "stream1", payload)
                .expect("append should succeed");
        }
    }

    // Crash and recover
    test_db.reopen();

    // Verify prefix consistency
    {
        let substrate = test_db.substrate();
        let events = substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed");

        // Events should form a prefix of 0..n
        let mut expected_seq = 0;
        for event in &events {
            if let Value::Object(ref m) = event.value {
                if let Some(Value::Int(seq)) = m.get("sequence") {
                    assert_eq!(
                        *seq, expected_seq,
                        "R6: Event sequence should be {}, got {}",
                        expected_seq, seq
                    );
                    expected_seq += 1;
                }
            }
        }
    }
}

// =============================================================================
// E1: SEQUENCE ORDERING PRESERVED
// EventLog-specific: sequence ordering is preserved after recovery
// =============================================================================

#[test]
fn test_e1_sequence_ordering_preserved() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    let mut original_sequences = Vec::new();

    // Write events and record sequences
    {
        let substrate = test_db.substrate();
        for i in 0..15 {
            let payload = Value::Object({
                let mut m = HashMap::new();
                m.insert("index".to_string(), Value::Int(i));
                m
            });
            let version = substrate
                .event_append(&run, "stream1", payload)
                .expect("append should succeed");

            if let Version::Sequence(seq) = version {
                original_sequences.push(seq);
            }
        }
    }

    // Crash and recover
    test_db.reopen();

    // Verify sequence ordering
    {
        let substrate = test_db.substrate();
        let events = substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed");

        assert_eq!(
            events.len(),
            original_sequences.len(),
            "E1: Should have same number of events"
        );

        let recovered_sequences: Vec<u64> = events
            .iter()
            .filter_map(|e| {
                if let Version::Sequence(seq) = e.version {
                    Some(seq)
                } else {
                    None
                }
            })
            .collect();

        assert_eq!(
            recovered_sequences, original_sequences,
            "E1: Sequences should match original"
        );

        // Verify strictly increasing
        for window in recovered_sequences.windows(2) {
            assert!(
                window[1] > window[0],
                "E1: Sequences should be strictly increasing: {} -> {}",
                window[0],
                window[1]
            );
        }
    }
}

// =============================================================================
// E2: STREAM ISOLATION PRESERVED
// EventLog-specific: stream isolation is preserved after recovery
// =============================================================================

#[test]
fn test_e2_stream_isolation_preserved() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    // Write to multiple streams with unique markers
    {
        let substrate = test_db.substrate();

        for i in 0..5 {
            let payload1 = Value::Object({
                let mut m = HashMap::new();
                m.insert("stream".to_string(), Value::String("s1".into()));
                m.insert("index".to_string(), Value::Int(i));
                m
            });
            substrate
                .event_append(&run, "stream1", payload1)
                .expect("append should succeed");
        }

        for i in 0..3 {
            let payload2 = Value::Object({
                let mut m = HashMap::new();
                m.insert("stream".to_string(), Value::String("s2".into()));
                m.insert("index".to_string(), Value::Int(i));
                m
            });
            substrate
                .event_append(&run, "stream2", payload2)
                .expect("append should succeed");
        }
    }

    // Crash and recover
    test_db.reopen();

    // Verify stream isolation
    {
        let substrate = test_db.substrate();

        let events1 = substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed");
        let events2 = substrate
            .event_range(&run, "stream2", None, None, None)
            .expect("range should succeed");

        assert_eq!(events1.len(), 5, "E2: stream1 should have 5 events");
        assert_eq!(events2.len(), 3, "E2: stream2 should have 3 events");

        // Verify no cross-contamination
        for event in &events1 {
            if let Value::Object(ref m) = event.value {
                assert_eq!(
                    m.get("stream"),
                    Some(&Value::String("s1".into())),
                    "E2: stream1 should only have s1 events"
                );
            }
        }

        for event in &events2 {
            if let Value::Object(ref m) = event.value {
                assert_eq!(
                    m.get("stream"),
                    Some(&Value::String("s2".into())),
                    "E2: stream2 should only have s2 events"
                );
            }
        }
    }
}

// =============================================================================
// CROSS-MODE EQUIVALENCE
// =============================================================================

#[test]
fn test_recovery_behavior_cross_mode() {
    test_across_modes("eventlog_recovery", |db| {
        let substrate = create_substrate(db);
        let run = ApiRunId::default();

        // Write events
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

        // Read back
        let events = substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed");

        events.len()
    });
}
