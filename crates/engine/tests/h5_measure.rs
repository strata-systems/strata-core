#![allow(clippy::cast_precision_loss)] // commit counts are small; this prints rates
//! Throughput probe (not a gate): does a shared handle buy PARALLELISM, or
//! only ergonomics? Run with --ignored --nocapture.
mod common;
use common::{branch, key, open_cache_database, open_durable_database, space, value};
use std::sync::Barrier;
use std::time::Instant;

fn run(label: &str, mut database: strata_engine::Database, threads: usize, writes: usize) {
    let names: Vec<_> = (0..threads).map(|i| branch(&format!("b{i}"))).collect();
    {
        let mut b = database.branches().expect("branches");
        for n in &names {
            b.fork_current(&branch("default"), n.clone()).expect("fork");
        }
    }

    // Serial: one thread does all the work.
    let start = Instant::now();
    for n in &names {
        let mut kv = database.kv(n.clone(), space("default")).expect("kv");
        for r in 0..writes {
            kv.put(key(format!("s{r}").as_bytes()), value(b"v"))
                .expect("put");
        }
    }
    let serial = start.elapsed();

    // Concurrent: one thread per branch, released together.
    let db = &database;
    let barrier = Barrier::new(threads);
    let start = Instant::now();
    std::thread::scope(|s| {
        for n in &names {
            let barrier = &barrier;
            s.spawn(move || {
                let mut kv = db.kv(n.clone(), space("default")).expect("kv");
                barrier.wait();
                for r in 0..writes {
                    kv.put(key(format!("c{r}").as_bytes()), value(b"v"))
                        .expect("put");
                }
            });
        }
    });
    let concurrent = start.elapsed();

    let total = threads * writes;
    println!(
        "{label}: {threads} threads x {writes} writes = {total} commits\n  \
         serial     {:>9.1?}  ({:>8.0} commits/s)\n  \
         concurrent {:>9.1?}  ({:>8.0} commits/s)\n  \
         speedup    {:.2}x",
        serial,
        total as f64 / serial.as_secs_f64(),
        concurrent,
        total as f64 / concurrent.as_secs_f64(),
        serial.as_secs_f64() / concurrent.as_secs_f64()
    );
}

#[test]
#[ignore = "measurement, not a gate"]
fn measure_cache() {
    run("cache  ", open_cache_database().expect("open"), 8, 2000);
}

#[test]
#[ignore = "measurement, not a gate"]
fn measure_durable() {
    let dir = tempfile::tempdir().expect("tmp");
    run(
        "durable",
        open_durable_database(dir.path()).expect("open"),
        8,
        500,
    );
}
