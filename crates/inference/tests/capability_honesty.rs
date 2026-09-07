//! The surface may not claim an ability this binary does not have (#3124).
//!
//! A released binary is built `native + cloud` — no `local` feature — and so
//! cannot run any of the eleven catalogued local models. It nonetheless
//! reported `can_embed: true` beside `provider_feature_enabled: false` in the
//! same object, listed every local model as present, and then refused the
//! operation with one of seven differently-worded messages, none of which said
//! what to do.
//!
//! These tests hold the surface to one law: **a capability flag is a promise
//! this binary can keep right now.**
//!
//! They are written to be correct under either feature set — with `local` on,
//! the claims become true and the assertions follow — so the same file guards
//! the source build and the released one.

use strata_inference::{InferenceRuntime, InferenceRuntimeConfig};

const LOCAL_BUILT_IN: bool = cfg!(feature = "local");

fn runtime() -> InferenceRuntime {
    InferenceRuntime::new(InferenceRuntimeConfig::default())
}

/// No `can_*` may be true while the feature that would execute it is absent.
/// This is the contradiction #3124 reported, stated as a law.
#[test]
fn capability_never_claims_more_than_the_build_can_do() {
    let runtime = runtime();
    for spec in ["miniLM", "gpt2", "tinyllama", "nomic-embed"] {
        let Ok(capability) = runtime.capability(spec) else {
            continue; // not in this build's catalog; nothing claimed, nothing to check
        };
        if capability.provider_feature_enabled {
            continue;
        }
        assert!(
            !capability.can_generate
                && !capability.can_embed
                && !capability.can_tokenize
                && !capability.can_rank,
            "{spec} claims an ability while its provider feature is off: {capability:?}"
        );
    }
}

/// The specific case from the issue: `capability miniLM` on a released binary.
#[test]
fn a_local_embedding_model_reports_what_this_build_can_do() {
    let capability = runtime()
        .capability("miniLM")
        .expect("miniLM is catalogued");

    assert_eq!(
        capability.can_embed, LOCAL_BUILT_IN,
        "can_embed must track whether this binary can actually embed"
    );
    assert_eq!(capability.can_tokenize, LOCAL_BUILT_IN);
    assert_eq!(capability.provider_feature_enabled, LOCAL_BUILT_IN);

    // The model's inherent shape stays reportable either way: it is still an
    // embedding model with a known dimension, which is what the catalog is for.
    assert_eq!(capability.embedding_dim, 384);
}

/// A cloud model in the default build is genuinely usable, so its flags stay
/// true — the fix must not make every flag false.
#[test]
fn cloud_capability_is_unaffected_when_its_feature_is_on() {
    let capability = runtime()
        .capability("openai:gpt-4o-mini")
        .expect("cloud spec parses");
    assert_eq!(capability.can_generate, cfg!(feature = "openai"));
    assert_eq!(capability.can_embed, cfg!(feature = "openai"));
    assert!(capability.requires_api_key);
    assert!(capability.requires_network);
}

/// The catalog says whether this binary can run what it lists.
#[test]
fn every_listed_model_declares_whether_it_can_run_here() {
    for model in runtime().list_models() {
        assert_eq!(
            model.runnable, LOCAL_BUILT_IN,
            "{} must declare runnability honestly; is_local ({}) is about the \
             file on disk, not about execution",
            model.name, model.is_local
        );
    }
}

/// Every local refusal uses the one phrasing, and that phrasing tells the user
/// what to do. Seven different sentences for one fact was the reported defect.
#[test]
#[cfg(not(feature = "local"))]
fn local_refusals_share_one_actionable_phrasing() {
    use strata_inference::EmbedRequest;

    let runtime = runtime();
    let refusals = [
        runtime
            .embed(
                "miniLM",
                &EmbedRequest {
                    text: "hello".to_owned(),
                },
            )
            .expect_err("embedding refuses without the local feature")
            .to_string(),
        runtime
            .tokenize("miniLM", "hello", false)
            .expect_err("tokenizing refuses without the local feature")
            .to_string(),
    ];

    for message in &refusals {
        assert!(
            message.contains("--features inference-local"),
            "a refusal must name the build that fixes it: {message}"
        );
        assert!(
            message.contains("cloud model") || message.contains("openai:"),
            "a refusal must name the alternative that needs no rebuild: {message}"
        );
    }
}

/// The refusal codes are unchanged by the rewording.
///
/// `InferenceError::code()` classifies `NotSupported` by substring-matching the
/// human message: "provider" yields `unsupported_provider`, "download" yields
/// `download_disabled`, everything else `unsupported_operation`. Improving a
/// message can therefore silently reclassify an error, which is a trap this
/// change walked into once already. Until that design is fixed, this pins it.
#[test]
#[cfg(not(feature = "local"))]
fn inference_refusals_keep_their_codes() {
    use strata_inference::EmbedRequest;

    let runtime = runtime();
    assert_eq!(
        runtime
            .embed(
                "miniLM",
                &EmbedRequest {
                    text: "hello".to_owned()
                }
            )
            .expect_err("embedding refuses")
            .code(),
        "inference.unsupported_operation"
    );
    assert_eq!(
        runtime
            .tokenize("miniLM", "hello", false)
            .expect_err("tokenizing refuses")
            .code(),
        "inference.unsupported_operation"
    );
}
