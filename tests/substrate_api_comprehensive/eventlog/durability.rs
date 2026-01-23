//! EventLog Durability Tests
//!
//! Tests for durability guarantees:
//! - Crash recovery: events survive database close/reopen
//! - Persistence modes: in_memory vs buffered vs strict
//! - Sequence preservation after crash
//! - Multi-stream survival

use crate::*;
use std::collections::HashMap;

// =============================================================================
// BASIC CRASH RECOVERY
// =============================================================================

#[test]
fn test_in_memory_no_persistence() {
    let mut test_db = TestDb::new_in_memory();
    let run = ApiRunId::default();

    // Append an event
    {
        let substrate = test_db.substrate();
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("test".to_string(), Value::String("value".into()));
            m
        });
        substrate
            .event_append(&run, "stream1", payload)
            .expect("append should succeed");

        let len = substrate
            .event_len(&run, "stream1")
            .expect("len should succeed");
        assert_eq!(len, 1, "Should have 1 event before reopen");
    }

    // Reopen - in-memory loses data
    test_db.reopen();

    {
        let substrate = test_db.substrate();
        let len = substrate
            .event_len(&run, "stream1")
            .expect("len should succeed");
        // In-memory mode creates fresh database
        assert_eq!(len, 0, "In-memory should lose data after reopen");
    }
}

#[test]
fn test_buffered_crash_recovery() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    let expected_payload = Value::Object({
        let mut m = HashMap::new();
        m.insert("persistent".to_string(), Value::Bool(true));
        m.insert("count".to_string(), Value::Int(42));
        m
    });

    // Append an event
    {
        let substrate = test_db.substrate();
        substrate
            .event_append(&run, "stream1", expected_payload.clone())
            .expect("append should succeed");

        let len = substrate
            .event_len(&run, "stream1")
            .expect("len should succeed");
        assert_eq!(len, 1, "Should have 1 event before crash");
    }

    // Simulate crash and recover
    test_db.reopen();

    {
        let substrate = test_db.substrate();
        let events = substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed");

        assert_eq!(events.len(), 1, "Event should survive crash");
        assert_eq!(events[0].value, expected_payload, "Payload should match");
    }
}

#[test]
fn test_strict_crash_recovery() {
    let mut test_db = TestDb::new_strict();
    let run = ApiRunId::default();

    let expected_payload = Value::Object({
        let mut m = HashMap::new();
        m.insert("persistent".to_string(), Value::Bool(true));
        m.insert("mode".to_string(), Value::String("strict".into()));
        m
    });

    // Append an event
    {
        let substrate = test_db.substrate();
        substrate
            .event_append(&run, "stream1", expected_payload.clone())
            .expect("append should succeed");
    }

    // Simulate crash and recover
    test_db.reopen();

    {
        let substrate = test_db.substrate();
        let events = substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed");

        assert_eq!(events.len(), 1, "Event should survive crash in strict mode");
        assert_eq!(events[0].value, expected_payload, "Payload should match");
    }
}

// =============================================================================
// SEQUENCE PRESERVATION
// =============================================================================

#[test]
fn test_sequences_preserved_after_crash() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    let mut original_sequences = Vec::new();

    // Append several events, record their sequences
    {
        let substrate = test_db.substrate();
        for i in 0..5 {
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

    // Simulate crash and recover
    test_db.reopen();

    // Verify sequences match
    {
        let substrate = test_db.substrate();
        let events = substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed");

        assert_eq!(events.len(), 5, "All events should survive");

        for (event, expected_seq) in events.iter().zip(original_sequences.iter()) {
            if let Version::Sequence(seq) = event.version {
                assert_eq!(
                    seq, *expected_seq,
                    "Sequence should be preserved after crash"
                );
            }
        }
    }
}

#[test]
fn test_latest_sequence_correct_after_crash() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    let mut last_seq = 0;

    // Append events
    {
        let substrate = test_db.substrate();
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
                last_seq = seq;
            }
        }
    }

    // Simulate crash and recover
    test_db.reopen();

    // Verify latest sequence
    {
        let substrate = test_db.substrate();
        let latest = substrate
            .event_latest_sequence(&run, "stream1")
            .expect("latest_sequence should succeed")
            .expect("should have latest");

        assert_eq!(
            latest, last_seq,
            "Latest sequence should be correct after crash"
        );
    }
}

// =============================================================================
// MULTI-STREAM DURABILITY
// =============================================================================

#[test]
fn test_multiple_streams_survive_crash() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    // Append to multiple streams
    {
        let substrate = test_db.substrate();

        for i in 0..3 {
            let payload = Value::Object({
                let mut m = HashMap::new();
                m.insert("stream".to_string(), Value::String("s1".into()));
                m.insert("index".to_string(), Value::Int(i));
                m
            });
            substrate
                .event_append(&run, "stream1", payload)
                .expect("append should succeed");
        }

        for i in 0..5 {
            let payload = Value::Object({
                let mut m = HashMap::new();
                m.insert("stream".to_string(), Value::String("s2".into()));
                m.insert("index".to_string(), Value::Int(i));
                m
            });
            substrate
                .event_append(&run, "stream2", payload)
                .expect("append should succeed");
        }

        for i in 0..2 {
            let payload = Value::Object({
                let mut m = HashMap::new();
                m.insert("stream".to_string(), Value::String("s3".into()));
                m.insert("index".to_string(), Value::Int(i));
                m
            });
            substrate
                .event_append(&run, "stream3", payload)
                .expect("append should succeed");
        }
    }

    // Simulate crash and recover
    test_db.reopen();

    // Verify all streams
    {
        let substrate = test_db.substrate();

        let len1 = substrate
            .event_len(&run, "stream1")
            .expect("len should succeed");
        let len2 = substrate
            .event_len(&run, "stream2")
            .expect("len should succeed");
        let len3 = substrate
            .event_len(&run, "stream3")
            .expect("len should succeed");

        assert_eq!(len1, 3, "stream1 should have 3 events after crash");
        assert_eq!(len2, 5, "stream2 should have 5 events after crash");
        assert_eq!(len3, 2, "stream3 should have 2 events after crash");
    }
}

#[test]
fn test_interleaved_streams_survive_crash() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    // Interleave appends to different streams
    {
        let substrate = test_db.substrate();

        for i in 0..10 {
            let stream = match i % 3 {
                0 => "stream1",
                1 => "stream2",
                _ => "stream3",
            };
            let payload = Value::Object({
                let mut m = HashMap::new();
                m.insert("stream".to_string(), Value::String(stream.into()));
                m.insert("global_index".to_string(), Value::Int(i));
                m
            });
            substrate
                .event_append(&run, stream, payload)
                .expect("append should succeed");
        }
    }

    // Simulate crash and recover
    test_db.reopen();

    // Verify counts and content
    {
        let substrate = test_db.substrate();

        // stream1: indices 0, 3, 6, 9 (4 events)
        // stream2: indices 1, 4, 7 (3 events)
        // stream3: indices 2, 5, 8 (3 events)

        let events1 = substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed");
        let events2 = substrate
            .event_range(&run, "stream2", None, None, None)
            .expect("range should succeed");
        let events3 = substrate
            .event_range(&run, "stream3", None, None, None)
            .expect("range should succeed");

        assert_eq!(events1.len(), 4, "stream1 should have 4 events");
        assert_eq!(events2.len(), 3, "stream2 should have 3 events");
        assert_eq!(events3.len(), 3, "stream3 should have 3 events");

        // Verify indices for stream1
        let indices1: Vec<i64> = events1
            .iter()
            .filter_map(|e| {
                if let Value::Object(ref m) = e.value {
                    if let Some(Value::Int(i)) = m.get("global_index") {
                        return Some(*i);
                    }
                }
                None
            })
            .collect();
        assert_eq!(indices1, vec![0, 3, 6, 9], "stream1 indices should match");
    }
}

// =============================================================================
// RUN ISOLATION DURABILITY
// =============================================================================

#[test]
fn test_run_isolation_survives_crash() {
    let mut test_db = TestDb::new_buffered();
    let run1 = ApiRunId::new();
    let run2 = ApiRunId::new();

    // Append to different runs
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

        for i in 0..3 {
            let payload2 = Value::Object({
                let mut m = HashMap::new();
                m.insert("run".to_string(), Value::String("run2".into()));
                m.insert("index".to_string(), Value::Int(i));
                m
            });
            substrate
                .event_append(&run2, "stream1", payload2)
                .expect("append should succeed");
        }
    }

    // Simulate crash and recover
    test_db.reopen();

    // Verify runs are still isolated
    {
        let substrate = test_db.substrate();

        let events1 = substrate
            .event_range(&run1, "stream1", None, None, None)
            .expect("range should succeed");
        let events2 = substrate
            .event_range(&run2, "stream1", None, None, None)
            .expect("range should succeed");

        assert_eq!(events1.len(), 1, "run1 should have 1 event after crash");
        assert_eq!(events2.len(), 3, "run2 should have 3 events after crash");

        // Verify correct run markers
        if let Value::Object(ref m) = events1[0].value {
            assert_eq!(
                m.get("run"),
                Some(&Value::String("run1".into())),
                "run1 event should have run1 marker"
            );
        }
    }
}

// =============================================================================
// LARGE DATA DURABILITY
// =============================================================================

#[test]
fn test_large_dataset_survives_crash() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    let event_count = 100;

    // Append many events
    {
        let substrate = test_db.substrate();

        for i in 0..event_count {
            let payload = Value::Object({
                let mut m = HashMap::new();
                m.insert("index".to_string(), Value::Int(i));
                m.insert(
                    "data".to_string(),
                    Value::String(format!("event_data_{}", i)),
                );
                m
            });
            substrate
                .event_append(&run, "stream1", payload)
                .expect("append should succeed");
        }
    }

    // Simulate crash and recover
    test_db.reopen();

    // Verify all events survived
    {
        let substrate = test_db.substrate();

        let len = substrate
            .event_len(&run, "stream1")
            .expect("len should succeed");
        assert_eq!(
            len, event_count as u64,
            "All {} events should survive crash",
            event_count
        );

        // Spot check some events
        let events = substrate
            .event_range(&run, "stream1", None, None, Some(10))
            .expect("range should succeed");

        for (i, event) in events.iter().enumerate() {
            if let Value::Object(ref m) = event.value {
                assert_eq!(
                    m.get("index"),
                    Some(&Value::Int(i as i64)),
                    "Event {} should have correct index",
                    i
                );
            }
        }
    }
}

// =============================================================================
// MULTIPLE CRASH CYCLES
// =============================================================================

#[test]
fn test_survives_multiple_crashes() {
    let mut test_db = TestDb::new_buffered();
    let run = ApiRunId::default();

    // First write cycle
    {
        let substrate = test_db.substrate();
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("cycle".to_string(), Value::Int(1));
            m
        });
        substrate
            .event_append(&run, "stream1", payload)
            .expect("append should succeed");
    }
    test_db.reopen();

    // Second write cycle
    {
        let substrate = test_db.substrate();
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("cycle".to_string(), Value::Int(2));
            m
        });
        substrate
            .event_append(&run, "stream1", payload)
            .expect("append should succeed");
    }
    test_db.reopen();

    // Third write cycle
    {
        let substrate = test_db.substrate();
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("cycle".to_string(), Value::Int(3));
            m
        });
        substrate
            .event_append(&run, "stream1", payload)
            .expect("append should succeed");
    }
    test_db.reopen();

    // Verify all cycles survived
    {
        let substrate = test_db.substrate();
        let events = substrate
            .event_range(&run, "stream1", None, None, None)
            .expect("range should succeed");

        assert_eq!(events.len(), 3, "All 3 cycles should survive");

        let cycles: Vec<i64> = events
            .iter()
            .filter_map(|e| {
                if let Value::Object(ref m) = e.value {
                    if let Some(Value::Int(i)) = m.get("cycle") {
                        return Some(*i);
                    }
                }
                None
            })
            .collect();

        assert_eq!(cycles, vec![1, 2, 3], "Cycles should be in order");
    }
}

// =============================================================================
// CROSS-MODE EQUIVALENCE (behavior, not persistence)
// =============================================================================

#[test]
fn test_append_behavior_across_modes() {
    test_across_modes("eventlog_durability_append", |db| {
        let substrate = create_substrate(db);
        let run = ApiRunId::default();

        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("test".to_string(), Value::Bool(true));
            m
        });

        substrate
            .event_append(&run, "stream1", payload)
            .expect("append should succeed");

        let len = substrate
            .event_len(&run, "stream1")
            .expect("len should succeed");

        len
    });
}
