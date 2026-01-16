//! M5 Performance Benchmarks - JSON Primitive
//!
//! Run with: cargo bench --bench m5_performance
//!
//! These benchmarks measure M5 JSON performance and verify M4 non-regression:
//!
//! JSON Performance Targets:
//! - JSON create (1KB): < 1ms
//! - JSON get at path (1KB): < 100µs
//! - JSON set at path (1KB): < 1ms
//! - JSON delete at path: < 500µs
//!
//! Non-Regression Targets (M4):
//! - KV put InMemory: < 3µs
//! - KV put Buffered: < 30µs
//! - KV get fast path: < 5µs
//! - Event append: < 10µs
//! - State read: < 5µs
//! - Trace record: < 15µs

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use in_mem_core::json::{JsonPath, JsonValue};
use in_mem_core::types::{JsonDocId, RunId};
use in_mem_core::value::Value;
use in_mem_engine::Database;
use in_mem_primitives::{EventLog, JsonStore, KVStore, StateCell, TraceStore, TraceType};
use std::sync::Arc;
use std::time::Duration;

// ========== JSON Operation Benchmarks ==========

fn json_create_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_create");
    group.measurement_time(Duration::from_secs(5));

    for size in [100, 1_000, 10_000] {
        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::new("bytes", size), &size, |b, &size| {
            let db = Database::builder().in_memory().open_temp().unwrap();
            let json = JsonStore::new(Arc::new(db));
            let run_id = RunId::new();

            // Create document of specified size (approximately)
            let value = JsonValue::from("x".repeat(size));

            b.iter(|| {
                let doc_id = JsonDocId::new();
                json.create(&run_id, &doc_id, value.clone()).unwrap()
            });
        });
    }

    group.finish();
}

fn json_get_at_path_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_get_at_path");
    group.measurement_time(Duration::from_secs(5));

    for depth in [1, 5, 10] {
        group.bench_with_input(BenchmarkId::new("depth", depth), &depth, |b, &depth| {
            let db = Database::builder().in_memory().open_temp().unwrap();
            let json = JsonStore::new(Arc::new(db));
            let run_id = RunId::new();
            let doc_id = JsonDocId::new();

            // Create nested document using serde_json
            let mut value: serde_json::Value = serde_json::json!(42);
            for _ in 0..depth {
                let mut obj = serde_json::Map::new();
                obj.insert("nested".to_string(), value);
                value = serde_json::Value::Object(obj);
            }
            json.create(&run_id, &doc_id, JsonValue::from_value(value))
                .unwrap();

            // Build path
            let path_str = (0..depth).map(|_| "nested").collect::<Vec<_>>().join(".");
            let path: JsonPath = path_str.parse().unwrap();

            b.iter(|| json.get(&run_id, &doc_id, &path).unwrap());
        });
    }

    group.finish();
}

fn json_set_at_path_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_set_at_path");
    group.measurement_time(Duration::from_secs(5));

    for depth in [1, 5, 10] {
        group.bench_with_input(BenchmarkId::new("depth", depth), &depth, |b, &depth| {
            let db = Database::builder().in_memory().open_temp().unwrap();
            let json = JsonStore::new(Arc::new(db));
            let run_id = RunId::new();
            let doc_id = JsonDocId::new();

            // Create nested document using serde_json
            let mut value: serde_json::Value = serde_json::json!(42);
            for _ in 0..depth {
                let mut obj = serde_json::Map::new();
                obj.insert("nested".to_string(), value);
                value = serde_json::Value::Object(obj);
            }
            json.create(&run_id, &doc_id, JsonValue::from_value(value))
                .unwrap();

            let path_str = (0..depth).map(|_| "nested").collect::<Vec<_>>().join(".");
            let path: JsonPath = path_str.parse().unwrap();
            let mut counter = 0i64;

            b.iter(|| {
                counter += 1;
                json.set(&run_id, &doc_id, &path, JsonValue::from(counter))
                    .unwrap()
            });
        });
    }

    group.finish();
}

fn json_delete_at_path_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_delete_at_path");
    group.measurement_time(Duration::from_secs(5));

    let db = Database::builder().in_memory().open_temp().unwrap();
    let json = JsonStore::new(Arc::new(db));
    let run_id = RunId::new();

    group.bench_function("object_key", |b| {
        b.iter_batched(
            || {
                let doc_id = JsonDocId::new();
                let mut obj = serde_json::Map::new();
                obj.insert("to_delete".to_string(), serde_json::json!(42));
                obj.insert("keep".to_string(), serde_json::json!(43));
                json.create(
                    &run_id,
                    &doc_id,
                    JsonValue::from_value(serde_json::Value::Object(obj)),
                )
                .unwrap();
                doc_id
            },
            |doc_id| {
                json.delete_at_path(&run_id, &doc_id, &"to_delete".parse().unwrap())
                    .unwrap()
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ========== Non-Regression Benchmarks (M4 Targets) ==========

fn kv_put_inmemory_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("m4_regression");
    group.measurement_time(Duration::from_secs(5));

    let db = Database::builder().in_memory().open_temp().unwrap();
    let kv = KVStore::new(Arc::new(db));
    let run_id = RunId::new();

    // Warmup
    for i in 0..100 {
        kv.put(&run_id, &format!("warmup{}", i), Value::I64(i as i64))
            .unwrap();
    }

    let mut counter = 0u64;

    group.bench_function("kv_put_inmemory", |b| {
        b.iter(|| {
            counter += 1;
            let key = format!("key_{}", counter);
            kv.put(&run_id, &key, Value::I64(counter as i64)).unwrap()
        });
    });

    group.finish();
}

fn kv_put_buffered_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("m4_regression");
    group.measurement_time(Duration::from_secs(5));

    let db = Database::builder().buffered().open_temp().unwrap();
    let kv = KVStore::new(Arc::new(db));
    let run_id = RunId::new();

    // Warmup
    for i in 0..100 {
        kv.put(&run_id, &format!("warmup{}", i), Value::I64(i as i64))
            .unwrap();
    }

    let mut counter = 0u64;

    group.bench_function("kv_put_buffered", |b| {
        b.iter(|| {
            counter += 1;
            let key = format!("key_{}", counter);
            kv.put(&run_id, &key, Value::I64(counter as i64)).unwrap()
        });
    });

    group.finish();
}

fn kv_get_fast_path_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("m4_regression");
    group.measurement_time(Duration::from_secs(5));

    let db = Database::builder().in_memory().open_temp().unwrap();
    let kv = KVStore::new(Arc::new(db));
    let run_id = RunId::new();

    // Pre-populate
    for i in 0..1000 {
        let key = format!("key_{}", i);
        kv.put(&run_id, &key, Value::I64(i as i64)).unwrap();
    }

    let mut counter = 0u64;

    group.bench_function("kv_get_fast_path", |b| {
        b.iter(|| {
            counter = (counter + 1) % 1000;
            let key = format!("key_{}", counter);
            kv.get(&run_id, &key).unwrap()
        });
    });

    group.finish();
}

fn event_append_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("m4_regression");
    group.measurement_time(Duration::from_secs(5));

    let db = Database::builder().in_memory().open_temp().unwrap();
    let events = EventLog::new(Arc::new(db));
    let run_id = RunId::new();

    let payload = Value::String("test data".to_string());

    group.bench_function("event_append", |b| {
        b.iter(|| {
            events
                .append(&run_id, "test_event", payload.clone())
                .unwrap()
        });
    });

    group.finish();
}

fn state_read_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("m4_regression");
    group.measurement_time(Duration::from_secs(5));

    let db = Database::builder().in_memory().open_temp().unwrap();
    let state = StateCell::new(Arc::new(db));
    let run_id = RunId::new();

    state.set(&run_id, "key", Value::I64(42)).unwrap();

    group.bench_function("state_read", |b| {
        b.iter(|| state.read(&run_id, "key").unwrap());
    });

    group.finish();
}

fn trace_record_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("m4_regression");
    group.measurement_time(Duration::from_secs(5));

    let db = Database::builder().in_memory().open_temp().unwrap();
    let trace = TraceStore::new(Arc::new(db));
    let run_id = RunId::new();

    let metadata = Value::String("test data".to_string());
    let trace_type = TraceType::Thought {
        content: "Benchmark thought".to_string(),
        confidence: Some(0.9),
    };

    group.bench_function("trace_record", |b| {
        b.iter(|| {
            trace
                .record(&run_id, trace_type.clone(), vec![], metadata.clone())
                .unwrap()
        });
    });

    group.finish();
}

// ========== Mixed Workload Benchmarks ==========

fn mixed_json_kv_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");
    group.measurement_time(Duration::from_secs(5));

    let db = Database::builder().in_memory().open_temp().unwrap();
    let db = Arc::new(db);
    let json = JsonStore::new(db.clone());
    let kv = KVStore::new(db);
    let run_id = RunId::new();

    // Setup
    let doc_id = JsonDocId::new();
    json.create(&run_id, &doc_id, JsonValue::object()).unwrap();

    let mut counter = 0u64;

    group.bench_function("json_and_kv", |b| {
        b.iter(|| {
            counter += 1;

            // JSON operation
            json.set(
                &run_id,
                &doc_id,
                &"counter".parse().unwrap(),
                JsonValue::from(counter as i64),
            )
            .unwrap();

            // KV operation
            let key = format!("key_{}", counter);
            kv.put(&run_id, &key, Value::I64(counter as i64)).unwrap();
        });
    });

    group.finish();
}

criterion_group!(
    json_benches,
    json_create_benchmarks,
    json_get_at_path_benchmarks,
    json_set_at_path_benchmarks,
    json_delete_at_path_benchmark,
);

criterion_group!(
    regression_benches,
    kv_put_inmemory_benchmark,
    kv_put_buffered_benchmark,
    kv_get_fast_path_benchmark,
    event_append_benchmark,
    state_read_benchmark,
    trace_record_benchmark,
);

criterion_group!(mixed_benches, mixed_json_kv_benchmark,);

criterion_main!(json_benches, regression_benches, mixed_benches);
