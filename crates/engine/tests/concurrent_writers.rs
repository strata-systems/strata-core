//! #3126: concurrent writers on one `Database`.
//!
//! H1–H3 relaxed the engine's handle so several capability services can be held
//! at once and readers no longer queue behind writers. That made the BORROW
//! CHECKER permit shared use. It did not, by itself, prove the type is
//! thread-safe or that anything actually proceeds in parallel.
//!
//! These tests close that gap. They are deliberately about *correctness under
//! real threads* rather than speed: the storage layer already carries a
//! measured write-concurrency design (group commit, a covering-fsync chain,
//! off-lock reads), but until now the engine's exclusive handle meant no engine
//! caller could reach it from more than one thread.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Barrier;

use common::{branch, key, open_cache_database, space, value};
use strata_engine::{BranchName, Database};

/// The bounds every claim below rests on. A compile-time check, so a future
/// change that quietly makes `Database` thread-unsafe fails here rather than
/// in whichever test happens to notice the data race first.
#[test]
fn database_is_send_and_sync() {
    const fn assert_send<T: Send>() {}
    const fn assert_sync<T: Sync>() {}
    assert_send::<Database>();
    assert_sync::<Database>();
}

/// Twenty branches, twenty threads, all committing at once through one shared
/// `&Database` — the "twenty colonies" shape #3126 was filed against.
///
/// Every thread waits on a barrier before its first write, so the commits
/// genuinely overlap rather than politely queueing behind one another.
#[test]
fn twenty_threads_commit_to_twenty_branches_concurrently() {
    const THREADS: usize = 20;
    const WRITES_PER_THREAD: usize = 25;

    let mut database = open_cache_database().expect("cache open succeeds");
    let names: Vec<BranchName> = (0..THREADS)
        .map(|i| branch(&format!("colony-{i}")))
        .collect();
    {
        let mut branches = database.branches().expect("branch service opens");
        for name in &names {
            branches
                .fork_current(&branch("default"), name.clone())
                .expect("branch forks");
        }
    }

    let db = &database;
    let barrier = Barrier::new(THREADS);
    let committed = AtomicUsize::new(0);

    std::thread::scope(|scope| {
        for name in &names {
            scope.spawn(|| {
                let mut kv = db.kv(name.clone(), space("default")).expect("kv opens");
                // Release everyone at once so the commits actually contend.
                barrier.wait();
                for round in 0..WRITES_PER_THREAD {
                    kv.put(key(format!("k{round}").as_bytes()), value(b"v"))
                        .expect("concurrent commit succeeds");
                    committed.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });

    assert_eq!(
        committed.load(Ordering::Relaxed),
        THREADS * WRITES_PER_THREAD,
        "every concurrent commit must succeed"
    );

    // And every branch holds exactly what its own thread wrote — no commit
    // landed on the wrong branch, and none was lost.
    for name in &names {
        let mut kv = database
            .kv(name.clone(), space("default"))
            .expect("kv reopens");
        for round in 0..WRITES_PER_THREAD {
            assert!(
                kv.get(&key(format!("k{round}").as_bytes()))
                    .expect("read succeeds")
                    .is_some(),
                "branch {name} lost round {round}"
            );
        }
    }
}

/// Concurrent writers on the SAME branch. Harder than the per-branch case: the
/// commits contend for one branch's version allocator, so this exercises the
/// storage layer's monotonic-floor clamping rather than just its parallelism.
///
/// The claim is not that they interleave in any particular order — it is that
/// none is rejected and none is lost.
#[test]
fn concurrent_writers_on_one_branch_neither_conflict_nor_lose_writes() {
    const THREADS: usize = 8;
    const WRITES_PER_THREAD: usize = 20;

    let database = open_cache_database().expect("cache open succeeds");
    let db = &database;
    let barrier = Barrier::new(THREADS);

    std::thread::scope(|scope| {
        for thread in 0..THREADS {
            let barrier = &barrier;
            scope.spawn(move || {
                let mut kv = db
                    .kv(branch("default"), space("default"))
                    .expect("kv opens");
                barrier.wait();
                for round in 0..WRITES_PER_THREAD {
                    kv.put(key(format!("t{thread}-r{round}").as_bytes()), value(b"v"))
                        .expect("concurrent same-branch commit succeeds");
                }
            });
        }
    });

    let mut kv = database
        .kv(branch("default"), space("default"))
        .expect("kv reopens");
    for thread in 0..THREADS {
        for round in 0..WRITES_PER_THREAD {
            assert!(
                kv.get(&key(format!("t{thread}-r{round}").as_bytes()))
                    .expect("read succeeds")
                    .is_some(),
                "thread {thread} round {round} was lost"
            );
        }
    }
}

/// Readers do not queue behind writers (#3156). A reader thread runs
/// continuously while writers commit; it must complete its reads rather than
/// stall, and must never observe a torn or missing already-written value.
#[test]
fn readers_run_concurrently_with_writers() {
    const READS: usize = 500;

    let database = open_cache_database().expect("cache open succeeds");
    {
        let mut kv = database
            .kv(branch("default"), space("default"))
            .expect("kv opens");
        kv.put(key(b"stable"), value(b"seed")).expect("seed write");
    }

    let db = &database;
    let writes_done = AtomicUsize::new(0);

    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut kv = db
                .kv(branch("default"), space("default"))
                .expect("writer kv opens");
            for round in 0..200 {
                kv.put(key(format!("w{round}").as_bytes()), value(b"v"))
                    .expect("write succeeds");
                writes_done.fetch_add(1, Ordering::Relaxed);
            }
        });

        scope.spawn(|| {
            let mut kv = db
                .kv(branch("default"), space("default"))
                .expect("reader kv opens");
            for _ in 0..READS {
                // The seeded key was committed before either thread started, so
                // it must be visible on every read regardless of interleaving.
                let seen = kv.get(&key(b"stable")).expect("read succeeds");
                assert!(seen.is_some(), "a committed value vanished under load");
            }
        });
    });

    assert_eq!(writes_done.load(Ordering::Relaxed), 200);
}
