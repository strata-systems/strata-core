//! A collection remembers which model produced its vectors (D9, rule 24).
//!
//! CLAUDE.md hard rule 24 has promised
//! `failed_precondition.embedding_model_mismatch` since V1 was designed. It
//! existed in neither the code nor `errors.yaml`, so the guarantee was a
//! sentence. This is the behaviour behind it.
//!
//! The failure it prevents is the quiet one. Dimension checks catch a 768-wide
//! vector going into a 384-wide collection. They cannot catch `nomic-embed`
//! vectors going into a `miniLM` collection at the same width — that returns
//! neighbours which are ranked, confident, and meaningless, and nothing
//! downstream can tell.

mod common;

use strata_engine::{
    EmbeddingModelId, EngineErrorClass, VectorCollectionName, VectorConfig, VectorDistanceMetric,
};

use common::{branch, open_cache_database, open_durable_database, space};

fn collection(name: &str) -> VectorCollectionName {
    VectorCollectionName::new(name).expect("collection name")
}

fn model(name: &str) -> EmbeddingModelId {
    EmbeddingModelId::new(name).expect("model id")
}

fn config(dimension: usize) -> VectorConfig {
    VectorConfig::new(dimension, VectorDistanceMetric::Cosine).expect("config")
}

/// A recorded model survives the round trip through storage.
#[test]
fn a_collection_remembers_the_model_it_was_created_with() {
    let mut database = open_cache_database().expect("cache open succeeds");
    let mut vectors = database
        .vector(branch("default"), space("default"))
        .expect("vector service opens");

    let created = vectors
        .create_collection(
            collection("docs"),
            config(384).with_embedding_model(model("miniLM")),
        )
        .expect("collection is created");
    assert_eq!(
        created
            .config()
            .embedding_model()
            .map(EmbeddingModelId::as_str),
        Some("miniLM")
    );

    // Read back through a fresh lookup, not the creation outcome.
    let info = vectors
        .collection_info(&collection("docs"))
        .expect("info read succeeds")
        .expect("collection exists");
    assert_eq!(
        info.config()
            .embedding_model()
            .map(EmbeddingModelId::as_str),
        Some("miniLM"),
        "the model is stored, not just returned from create"
    );
}

/// The provenance survives a reopen, which is what makes it provenance rather
/// than a runtime hint.
#[test]
fn the_model_survives_a_durable_reopen() {
    let directory = tempfile::tempdir().expect("tempdir");

    {
        let mut database = open_durable_database(directory.path()).expect("durable open");
        database
            .vector(branch("default"), space("default"))
            .expect("vector service opens")
            .create_collection(
                collection("docs"),
                config(384).with_embedding_model(model("openai:text-embedding-3-small")),
            )
            .expect("collection is created");
    }

    let mut database = open_durable_database(directory.path()).expect("durable reopen");
    let info = database
        .vector(branch("default"), space("default"))
        .expect("vector service opens")
        .collection_info(&collection("docs"))
        .expect("info read succeeds")
        .expect("collection survived the reopen");
    assert_eq!(
        info.config()
            .embedding_model()
            .map(EmbeddingModelId::as_str),
        Some("openai:text-embedding-3-small")
    );
}

/// The case dimension cannot catch: two models, same width.
#[test]
fn a_second_model_at_the_same_width_is_refused() {
    let mut database = open_cache_database().expect("cache open succeeds");
    let mut vectors = database
        .vector(branch("default"), space("default"))
        .expect("vector service opens");

    vectors
        .create_collection(
            collection("docs"),
            config(384).with_embedding_model(model("miniLM")),
        )
        .expect("collection is created");

    // Same dimension, same metric — nothing but the recorded model can tell
    // these apart, which is the whole reason it is recorded.
    let error = vectors
        .require_embedding_model(&collection("docs"), &model("nomic-embed"))
        .expect_err("a different model is refused");
    assert_eq!(error.class(), EngineErrorClass::Conflict);
    assert_eq!(
        error.code(),
        "failed_precondition.engine.embedding_model_mismatch"
    );

    // The matching model is accepted, so the refusal is about disagreement and
    // not about the check being on.
    vectors
        .require_embedding_model(&collection("docs"), &model("miniLM"))
        .expect("the recorded model is accepted");
}

/// Collections created before provenance existed keep working.
///
/// Every collection in every database today is in this state: the field
/// defaults to absent, so the check has nothing to disagree with. Refusing
/// them here would break existing data to enforce a rule they predate.
#[test]
fn a_collection_without_a_recorded_model_accepts_any_model() {
    let mut database = open_cache_database().expect("cache open succeeds");
    let mut vectors = database
        .vector(branch("default"), space("default"))
        .expect("vector service opens");

    vectors
        .create_collection(collection("legacy"), config(384))
        .expect("collection is created");

    let info = vectors
        .collection_info(&collection("legacy"))
        .expect("info read succeeds")
        .expect("collection exists");
    assert!(
        info.config().embedding_model().is_none(),
        "no model is recorded, which is a legal state"
    );

    for any in ["miniLM", "nomic-embed", "openai:text-embedding-3-small"] {
        vectors
            .require_embedding_model(&collection("legacy"), &model(any))
            .expect("a collection with no recorded model accepts any");
    }
}

/// A model identifier is validated, but its shape is not the engine's business:
/// engine never speaks to a provider, so it cannot know which names are real.
#[test]
fn model_identifiers_are_validated_without_knowing_any_provider() {
    // Accepted: bare names and provider-prefixed specs alike.
    for accepted in ["miniLM", "openai:text-embedding-3-small", "some/vendor:v2"] {
        EmbeddingModelId::new(accepted).expect(accepted);
    }

    // Refused: the shapes that would corrupt a stored record.
    let empty = EmbeddingModelId::new("").expect_err("empty is refused");
    assert_eq!(empty.class(), EngineErrorClass::InvalidInput);
    assert_eq!(empty.code(), "invalid_argument.engine.embedding_model");

    EmbeddingModelId::new("has\0nul").expect_err("a NUL byte is refused");
    EmbeddingModelId::new("has\nnewline").expect_err("a newline is refused");
    EmbeddingModelId::new("x".repeat(257)).expect_err("an over-long id is refused");
}

/// Provenance is per collection, so one space can hold collections from
/// different models without either contaminating the other.
#[test]
fn two_collections_can_record_different_models() {
    let mut database = open_cache_database().expect("cache open succeeds");
    let mut vectors = database
        .vector(branch("default"), space("default"))
        .expect("vector service opens");

    vectors
        .create_collection(
            collection("small"),
            config(384).with_embedding_model(model("miniLM")),
        )
        .expect("first collection");
    vectors
        .create_collection(
            collection("large"),
            config(768).with_embedding_model(model("nomic-embed")),
        )
        .expect("second collection");

    vectors
        .require_embedding_model(&collection("small"), &model("miniLM"))
        .expect("first collection keeps its own model");
    vectors
        .require_embedding_model(&collection("large"), &model("nomic-embed"))
        .expect("second collection keeps its own model");
    vectors
        .require_embedding_model(&collection("small"), &model("nomic-embed"))
        .expect_err("the other collection's model is still refused here");
}
