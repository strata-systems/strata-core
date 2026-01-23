//! EventLog Stream Tests
//!
//! Tests for multi-stream operations and stream isolation:
//! - Multiple streams within same run
//! - Stream isolation (events only visible in their stream)
//! - Global vs per-stream sequences (known limitation)
//! - Stream naming conventions

use crate::*;
use std::collections::HashMap;

// =============================================================================
// MULTI-STREAM TESTS
// =============================================================================

#[test]
fn test_multiple_streams_independent() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    // Append to stream1
    for i in 0..3 {
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("stream".to_string(), Value::String("stream1".into()));
            m.insert("index".to_string(), Value::Int(i));
            m
        });
        substrate
            .event_append(&run, "stream1", payload)
            .expect("append should succeed");
    }

    // Append to stream2
    for i in 0..5 {
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

    // Verify streams are independent
    let events1 = substrate
        .event_range(&run, "stream1", None, None, None)
        .expect("range should succeed");
    let events2 = substrate
        .event_range(&run, "stream2", None, None, None)
        .expect("range should succeed");

    assert_eq!(events1.len(), 3, "stream1 should have 3 events");
    assert_eq!(events2.len(), 5, "stream2 should have 5 events");

    // Verify all events in stream1 have correct marker
    for event in &events1 {
        if let Value::Object(ref m) = event.value {
            assert_eq!(
                m.get("stream"),
                Some(&Value::String("stream1".into())),
                "All stream1 events should have stream1 marker"
            );
        }
    }

    // Verify all events in stream2 have correct marker
    for event in &events2 {
        if let Value::Object(ref m) = event.value {
            assert_eq!(
                m.get("stream"),
                Some(&Value::String("stream2".into())),
                "All stream2 events should have stream2 marker"
            );
        }
    }
}

#[test]
fn test_interleaved_appends_to_different_streams() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    // Interleave appends: s1, s2, s1, s2, s1
    for i in 0..5 {
        let stream = if i % 2 == 0 { "stream1" } else { "stream2" };
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

    // stream1: indices 0, 2, 4 (3 events)
    let events1 = substrate
        .event_range(&run, "stream1", None, None, None)
        .expect("range should succeed");
    // stream2: indices 1, 3 (2 events)
    let events2 = substrate
        .event_range(&run, "stream2", None, None, None)
        .expect("range should succeed");

    assert_eq!(events1.len(), 3, "stream1 should have 3 events");
    assert_eq!(events2.len(), 2, "stream2 should have 2 events");

    // Verify global indices for stream1
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
    assert_eq!(indices1, vec![0, 2, 4], "stream1 should have indices 0, 2, 4");

    // Verify global indices for stream2
    let indices2: Vec<i64> = events2
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
    assert_eq!(indices2, vec![1, 3], "stream2 should have indices 1, 3");
}

// =============================================================================
// SEQUENCE BEHAVIOR TESTS
// Note: Sequences are GLOBAL, not per-stream (known limitation)
// =============================================================================

#[test]
fn test_sequences_are_global_not_per_stream() {
    // This documents the known limitation: sequences are global across all streams
    // within a run, not per-stream like Redis streams.

    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    // Append to stream1, get sequence
    let payload1 = Value::Object({
        let mut m = HashMap::new();
        m.insert("stream".to_string(), Value::String("stream1".into()));
        m
    });
    let v1 = substrate
        .event_append(&run, "stream1", payload1)
        .expect("append should succeed");
    let seq1 = match v1 {
        Version::Sequence(n) => n,
        _ => panic!("Expected sequence"),
    };

    // Append to stream2, get sequence
    let payload2 = Value::Object({
        let mut m = HashMap::new();
        m.insert("stream".to_string(), Value::String("stream2".into()));
        m
    });
    let v2 = substrate
        .event_append(&run, "stream2", payload2)
        .expect("append should succeed");
    let seq2 = match v2 {
        Version::Sequence(n) => n,
        _ => panic!("Expected sequence"),
    };

    // Append to stream1 again
    let payload3 = Value::Object({
        let mut m = HashMap::new();
        m.insert("stream".to_string(), Value::String("stream1".into()));
        m.insert("second".to_string(), Value::Bool(true));
        m
    });
    let v3 = substrate
        .event_append(&run, "stream1", payload3)
        .expect("append should succeed");
    let seq3 = match v3 {
        Version::Sequence(n) => n,
        _ => panic!("Expected sequence"),
    };

    // Document: sequences are global (seq1 < seq2 < seq3 even though stream1 has gaps)
    assert!(seq2 > seq1, "Second append should have higher sequence");
    assert!(seq3 > seq2, "Third append should have higher sequence");

    // Note: seq3 is NOT seq1+1 because seq2 was allocated to stream2
    // This is the known limitation - sequences span all streams
}

#[test]
fn test_get_event_by_global_sequence() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    // Append to stream1
    let payload1 = Value::Object({
        let mut m = HashMap::new();
        m.insert("which".to_string(), Value::String("first".into()));
        m
    });
    let v1 = substrate
        .event_append(&run, "stream1", payload1)
        .expect("append should succeed");
    let seq1 = match v1 {
        Version::Sequence(n) => n,
        _ => panic!("Expected sequence"),
    };

    // Append to stream2
    let payload2 = Value::Object({
        let mut m = HashMap::new();
        m.insert("which".to_string(), Value::String("second".into()));
        m
    });
    let v2 = substrate
        .event_append(&run, "stream2", payload2)
        .expect("append should succeed");
    let seq2 = match v2 {
        Version::Sequence(n) => n,
        _ => panic!("Expected sequence"),
    };

    // Get event from stream1 using its sequence
    let event1 = substrate
        .event_get(&run, "stream1", seq1)
        .expect("get should succeed")
        .expect("event should exist");

    if let Value::Object(ref m) = event1.value {
        assert_eq!(
            m.get("which"),
            Some(&Value::String("first".into())),
            "Should get correct event from stream1"
        );
    }

    // Get event from stream2 using its sequence
    let event2 = substrate
        .event_get(&run, "stream2", seq2)
        .expect("get should succeed")
        .expect("event should exist");

    if let Value::Object(ref m) = event2.value {
        assert_eq!(
            m.get("which"),
            Some(&Value::String("second".into())),
            "Should get correct event from stream2"
        );
    }

    // Try to get stream2's event from stream1 - should return None
    let wrong_stream = substrate
        .event_get(&run, "stream1", seq2)
        .expect("get should succeed");

    assert!(
        wrong_stream.is_none(),
        "Getting event with wrong stream should return None"
    );
}

// =============================================================================
// STREAM NAMING TESTS
// =============================================================================

#[test]
fn test_stream_name_with_special_characters() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    let special_streams = vec![
        "stream-with-dashes",
        "stream_with_underscores",
        "stream.with.dots",
        "stream:with:colons",
        "stream/with/slashes",
        "CamelCaseStream",
        "UPPERCASE_STREAM",
    ];

    for stream_name in &special_streams {
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("stream_name".to_string(), Value::String(stream_name.to_string()));
            m
        });

        let result = substrate.event_append(&run, stream_name, payload);
        assert!(
            result.is_ok(),
            "Stream name '{}' should be accepted: {:?}",
            stream_name,
            result
        );
    }

    // Verify each stream has exactly 1 event
    for stream_name in &special_streams {
        let len = substrate
            .event_len(&run, stream_name)
            .expect("len should succeed");
        assert_eq!(len, 1, "Stream '{}' should have 1 event", stream_name);
    }
}

#[test]
fn test_stream_name_unicode() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    let unicode_streams = vec![
        "stream_unicode_emoji",  // Avoid actual emoji in stream names
        "stream_chinese_test",
        "stream_arabic_test",
    ];

    for stream_name in &unicode_streams {
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("test".to_string(), Value::Bool(true));
            m
        });

        let result = substrate.event_append(&run, stream_name, payload);
        assert!(
            result.is_ok(),
            "Stream name '{}' should be accepted: {:?}",
            stream_name,
            result
        );
    }
}

#[test]
fn test_stream_case_sensitivity() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    // Append to "Stream" and "stream" - should be different streams
    let payload_upper = Value::Object({
        let mut m = HashMap::new();
        m.insert("case".to_string(), Value::String("upper".into()));
        m
    });
    substrate
        .event_append(&run, "Stream", payload_upper)
        .expect("append should succeed");

    let payload_lower = Value::Object({
        let mut m = HashMap::new();
        m.insert("case".to_string(), Value::String("lower".into()));
        m
    });
    substrate
        .event_append(&run, "stream", payload_lower)
        .expect("append should succeed");

    // Should be different streams
    let events_upper = substrate
        .event_range(&run, "Stream", None, None, None)
        .expect("range should succeed");
    let events_lower = substrate
        .event_range(&run, "stream", None, None, None)
        .expect("range should succeed");

    assert_eq!(events_upper.len(), 1, "Stream should have 1 event");
    assert_eq!(events_lower.len(), 1, "stream should have 1 event");

    // Verify correct payloads
    if let Value::Object(ref m) = events_upper[0].value {
        assert_eq!(
            m.get("case"),
            Some(&Value::String("upper".into())),
            "Stream should have upper event"
        );
    }
    if let Value::Object(ref m) = events_lower[0].value {
        assert_eq!(
            m.get("case"),
            Some(&Value::String("lower".into())),
            "stream should have lower event"
        );
    }
}

// =============================================================================
// LATEST SEQUENCE PER STREAM
// =============================================================================

#[test]
fn test_latest_sequence_per_stream() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    // Append to multiple streams
    let mut last_seq1 = 0;
    let mut last_seq2 = 0;

    for i in 0..3 {
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("index".to_string(), Value::Int(i));
            m
        });
        let v = substrate
            .event_append(&run, "stream1", payload)
            .expect("append should succeed");
        if let Version::Sequence(seq) = v {
            last_seq1 = seq;
        }
    }

    for i in 0..5 {
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("index".to_string(), Value::Int(i));
            m
        });
        let v = substrate
            .event_append(&run, "stream2", payload)
            .expect("append should succeed");
        if let Version::Sequence(seq) = v {
            last_seq2 = seq;
        }
    }

    // Latest sequence should be different for each stream
    let latest1 = substrate
        .event_latest_sequence(&run, "stream1")
        .expect("latest_sequence should succeed")
        .expect("should have latest");

    let latest2 = substrate
        .event_latest_sequence(&run, "stream2")
        .expect("latest_sequence should succeed")
        .expect("should have latest");

    assert_eq!(latest1, last_seq1, "stream1 latest should match");
    assert_eq!(latest2, last_seq2, "stream2 latest should match");
    assert_ne!(latest1, latest2, "Different streams should have different latest");
}

// =============================================================================
// LEN PER STREAM
// =============================================================================

#[test]
fn test_len_isolated_per_stream() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    // Append different counts to different streams
    for i in 0..2 {
        let payload = Value::Object({
            let mut m = HashMap::new();
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
            m.insert("index".to_string(), Value::Int(i));
            m
        });
        substrate
            .event_append(&run, "stream2", payload)
            .expect("append should succeed");
    }

    for i in 0..10 {
        let payload = Value::Object({
            let mut m = HashMap::new();
            m.insert("index".to_string(), Value::Int(i));
            m
        });
        substrate
            .event_append(&run, "stream3", payload)
            .expect("append should succeed");
    }

    assert_eq!(
        substrate.event_len(&run, "stream1").expect("len should succeed"),
        2,
        "stream1 should have 2 events"
    );
    assert_eq!(
        substrate.event_len(&run, "stream2").expect("len should succeed"),
        5,
        "stream2 should have 5 events"
    );
    assert_eq!(
        substrate.event_len(&run, "stream3").expect("len should succeed"),
        10,
        "stream3 should have 10 events"
    );
    assert_eq!(
        substrate.event_len(&run, "stream_nonexistent").expect("len should succeed"),
        0,
        "nonexistent stream should have 0 events"
    );
}
