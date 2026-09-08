//! `--text` embeds with the collection's own model and stores it (D10).
//!
//! The point of D9 was to record which model wrote a collection. This is what
//! the record is *for*: a caller writes and searches with text, and never
//! names a model at all — so it cannot name the wrong one.
//!
//! Driven against the **real** local embedder — `miniLM`, the catalogued
//! `all-MiniLM-L6-v2` GGUF — because a fake proves the plumbing and not the
//! thing being claimed. The claim is that text written and text searched go
//! through the same model and land near each other, and only a real embedder
//! can show that: a deterministic fake would pass with any embedding function
//! at all, including one that ignores the text.
//!
//! Gated like `crates/inference/tests/local_integration.rs`: the `local`
//! feature plus `STRATA_RUN_LOCAL_INFERENCE_INTEGRATION`, since it needs both a
//! llama.cpp build and the model on disk.
//!
//! The orchestration under test lives in the **executor**, which is a
//! deliberate exception to the thin-executor rule — engine cannot import
//! inference (hard rules 2-3), and the intelligence layer that would mediate
//! it is deferred (#3171).

#![cfg(all(feature = "testkit", feature = "inference-local"))]

use strata_executor::{Command, Executor, Output};
use strata_inference::{InferenceRuntime, InferenceRuntimeConfig};

/// `all-MiniLM-L6-v2` embeds into 384 dimensions.
const MODEL: &str = "miniLM";
const DIMENSION: u64 = 384;

fn integration_enabled() -> bool {
    std::env::var_os("STRATA_RUN_LOCAL_INFERENCE_INTEGRATION").is_some()
}

fn executor() -> Executor {
    Executor::open_cache()
        .expect("cache executor opens")
        .with_inference_runtime(InferenceRuntime::new(InferenceRuntimeConfig::default()))
}

fn create(executor: &mut Executor, collection: &str, model: Option<&str>) {
    executor
        .execute(Command::VectorCreateCollection {
            branch: None,
            space: None,
            collection: collection.to_owned(),
            dimension: DIMENSION,
            metric: strata_executor::VectorDistanceMetric::Cosine,
            embedding_model: model.map(str::to_owned),
        })
        .expect("collection creates");
}

fn upsert_text(executor: &mut Executor, collection: &str, key: &str, text: &str) -> Output {
    executor
        .execute(Command::VectorUpsert {
            branch: None,
            space: None,
            collection: collection.to_owned(),
            key: key.to_owned(),
            vector: Vec::new(),
            text: Some(text.to_owned()),
            metadata: None,
        })
        .expect("text upsert succeeds")
}

/// The whole point: write with text, search with text, never name a model.
#[test]
fn text_round_trips_from_write_to_search_without_naming_a_model() {
    if !integration_enabled() {
        return;
    }
    let mut executor = executor();
    create(&mut executor, "docs", Some(MODEL));

    upsert_text(&mut executor, "docs", "branches", "how do branches work");
    upsert_text(
        &mut executor,
        "docs",
        "vectors",
        "similarity search over vectors",
    );

    let Output::VectorMatches(items) = executor
        .execute(Command::VectorQuery {
            branch: None,
            space: None,
            collection: "docs".to_owned(),
            query: Vec::new(),
            text: Some("how do branches work".to_owned()),
            k: 1,
            filter: None,
            as_of: None,
            as_of_time: None,
        })
        .expect("text query succeeds")
    else {
        panic!("expected vector matches");
    };

    // A real embedder makes this a claim about meaning, not plumbing: the
    // nearest neighbour of the branches question is the branches document, not
    // the unrelated one. An embedder that ignored the text would fail here,
    // where a deterministic fake would pass.
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].key(),
        "branches",
        "the query embedded with the collection's model finds the matching document"
    );
}

/// A collection with no recorded model refuses text, and says what to do.
///
/// This is D9's refusal reached through the path it was built for. Guessing a
/// model here would produce vectors that are not comparable with whatever is
/// already stored, and nothing downstream could detect it.
#[test]
fn a_collection_without_a_model_refuses_text() {
    if !integration_enabled() {
        return;
    }
    let mut executor = executor();
    create(&mut executor, "raw", None);

    let error = executor
        .execute(Command::VectorUpsert {
            branch: None,
            space: None,
            collection: "raw".to_owned(),
            key: "k".to_owned(),
            vector: Vec::new(),
            text: Some("hello".to_owned()),
            metadata: None,
        })
        .expect_err("text is refused without a recorded model");
    assert_eq!(
        error.code(),
        "failed_precondition.engine.embedding_model_missing"
    );
    assert!(
        error.to_string().contains("set-embedding-model"),
        "the refusal must name the command that declares a model: {error}"
    );

    // Raw vectors still work on that collection — the refusal is about text
    // needing a model, not about the collection being unusable.
    executor
        .execute(Command::VectorUpsert {
            branch: None,
            space: None,
            collection: "raw".to_owned(),
            key: "k".to_owned(),
            vector: vec![0.5; DIMENSION as usize],
            text: None,
            metadata: None,
        })
        .expect("an explicit vector is still accepted");
}

/// Exactly one of a vector or a text. Both is ambiguous, neither is empty.
#[test]
fn a_vector_and_a_text_together_are_refused() {
    if !integration_enabled() {
        return;
    }
    let mut executor = executor();
    create(&mut executor, "docs", Some(MODEL));

    let both = executor
        .execute(Command::VectorUpsert {
            branch: None,
            space: None,
            collection: "docs".to_owned(),
            key: "k".to_owned(),
            vector: vec![0.5; DIMENSION as usize],
            text: Some("hello".to_owned()),
            metadata: None,
        })
        .expect_err("both is refused");
    assert_eq!(both.code(), "invalid_argument.executor.vector_input");
    assert!(
        both.to_string().contains("not both"),
        "the refusal explains the choice: {both}"
    );

    let neither = executor
        .execute(Command::VectorUpsert {
            branch: None,
            space: None,
            collection: "docs".to_owned(),
            key: "k".to_owned(),
            vector: Vec::new(),
            text: None,
            metadata: None,
        })
        .expect_err("neither is refused");
    assert_eq!(neither.code(), "invalid_argument.executor.vector_input");
    assert!(neither.to_string().contains("vector"), "{neither}");
}

/// A text write that fails leaves nothing behind.
///
/// The embedding happens before the commit, so anything that goes wrong up to
/// and including the store leaves the collection exactly as it was — there is
/// no half-written row to clean up.
///
/// Provoked by a collection whose declared width disagrees with what the model
/// actually emits, which is the realistic version of this: someone records
/// `--embedding-model` and gets the dimension wrong. The embedding succeeds and
/// the store refuses.
#[test]
fn a_text_write_that_fails_leaves_nothing_behind() {
    if !integration_enabled() {
        return;
    }
    let mut executor = executor();
    executor
        .execute(Command::VectorCreateCollection {
            branch: None,
            space: None,
            collection: "docs".to_owned(),
            // miniLM emits DIMENSION; this collection claims twice that.
            dimension: DIMENSION * 2,
            metric: strata_executor::VectorDistanceMetric::Cosine,
            embedding_model: Some(MODEL.to_owned()),
        })
        .expect("collection creates");

    executor
        .execute(Command::VectorUpsert {
            branch: None,
            space: None,
            collection: "docs".to_owned(),
            key: "k".to_owned(),
            vector: Vec::new(),
            text: Some("hello".to_owned()),
            metadata: None,
        })
        .expect_err("a vector of the wrong width is refused");

    let Output::Uint(count) = executor
        .execute(Command::VectorCount {
            branch: None,
            space: None,
            collection: "docs".to_owned(),
            as_of: None,
            as_of_time: None,
        })
        .expect("count succeeds")
    else {
        panic!("expected a count");
    };
    assert_eq!(count, 0, "a failed embedding must leave nothing behind");
}
