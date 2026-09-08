//! Cloud embedding engine: text → dense vector via OpenAI or Google APIs.
//!
//! [`CloudEmbeddingEngine`] provides the same `embed()` / `embed_batch()`
//! interface as the local [`EmbeddingEngine`](crate::EmbeddingEngine), but
//! sends requests to cloud provider APIs instead of running a local model.
//!
//! Anthropic does not offer an embedding API — attempting to construct a
//! `CloudEmbeddingEngine` with `ProviderKind::Anthropic` returns `NotSupported`.

use crate::{embedding_provider_feature_enabled, InferenceError, ProviderFailure, ProviderKind};

/// Embedding engine backed by a cloud provider (OpenAI or Google).
///
/// Thread-safe (`Send`) — no interior mutability needed since each embed
/// call is a stateless HTTP request.
pub struct CloudEmbeddingEngine {
    provider: ProviderKind,
    api_key: String,
    model: String,
}

impl std::fmt::Debug for CloudEmbeddingEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudEmbeddingEngine")
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

impl CloudEmbeddingEngine {
    /// Create a new cloud embedding engine.
    ///
    /// # Errors
    ///
    /// - `NotSupported` if provider is `Anthropic` or `Local`
    /// - `Provider` if api_key or model is empty
    pub fn new(
        provider: ProviderKind,
        api_key: String,
        model: String,
    ) -> Result<Self, InferenceError> {
        match provider {
            ProviderKind::Anthropic => {
                return Err(InferenceError::NotSupported(
                    "Anthropic does not offer an embedding API".to_string(),
                ));
            }
            ProviderKind::Local => {
                return Err(InferenceError::NotSupported(
                    "use EmbeddingEngine for local models, not CloudEmbeddingEngine".to_string(),
                ));
            }
            ProviderKind::OpenAI | ProviderKind::Google => {
                if !embedding_provider_feature_enabled(provider) {
                    return Err(InferenceError::NotSupported(format!(
                        "{provider} embedding provider not enabled"
                    )));
                }
            }
        }

        if api_key.trim().is_empty() {
            return Err(InferenceError::Provider(format!(
                "{} API key is empty",
                provider
            )));
        }
        if model.trim().is_empty() {
            return Err(InferenceError::Provider(format!(
                "{} model name is empty",
                provider
            )));
        }

        Ok(Self {
            provider,
            api_key,
            model,
        })
    }

    /// Which provider this engine uses.
    pub fn provider(&self) -> ProviderKind {
        self.provider
    }

    /// The model name.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Embed a single text via the cloud API.
    fn embed_single(&self, text: &str) -> Result<Vec<f32>, InferenceError> {
        match self.provider {
            #[cfg(feature = "openai")]
            ProviderKind::OpenAI => {
                let body = crate::provider::openai::build_embed_request_json(&self.model, &[text]);
                let agent = ureq::Agent::new_with_config(
                    ureq::config::Config::builder()
                        .timeout_global(Some(std::time::Duration::from_secs(30)))
                        .build(),
                );
                let mut response = agent
                    .post(crate::provider::openai::EMBED_API_URL)
                    .header("Authorization", &format!("Bearer {}", self.api_key))
                    .header("content-type", "application/json")
                    .send(&body)
                    .map_err(|e| map_http_error("OpenAI", e))?;

                let response_body = response.body_mut().read_to_string().map_err(|e| {
                    InferenceError::Provider(format!("OpenAI: failed to read embed response: {e}"))
                })?;

                let mut embeddings =
                    crate::provider::openai::parse_embed_response_json(&response_body)?;
                if embeddings.is_empty() {
                    return Err(InferenceError::Provider(
                        "OpenAI: no embeddings returned".to_string(),
                    ));
                }
                let embedding = embeddings.swap_remove(0);
                Ok(l2_normalize(embedding))
            }

            #[cfg(feature = "google")]
            ProviderKind::Google => {
                let url = crate::provider::google::build_embed_url(&self.model);
                let body = crate::provider::google::build_embed_request_json(text);
                let agent = ureq::Agent::new_with_config(
                    ureq::config::Config::builder()
                        .timeout_global(Some(std::time::Duration::from_secs(30)))
                        .build(),
                );
                let mut response = agent
                    .post(&url)
                    .header("x-goog-api-key", &self.api_key)
                    .header("content-type", "application/json")
                    .send(&body)
                    .map_err(|e| map_http_error("Google", e))?;

                let response_body = response.body_mut().read_to_string().map_err(|e| {
                    InferenceError::Provider(format!("Google: failed to read embed response: {e}"))
                })?;

                let embedding = crate::provider::google::parse_embed_response_json(&response_body)?;
                Ok(l2_normalize(embedding))
            }

            other => Err(InferenceError::NotSupported(format!(
                "cloud embedding not supported for provider: {other}"
            ))),
        }
    }

    /// Embed a batch of texts via the cloud API.
    /// Maximum texts per HTTP request to avoid API limits and timeouts.
    const CLOUD_BATCH_CHUNK_SIZE: usize = 64;

    fn embed_many(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, InferenceError> {
        // Chunk large batches to stay within API limits.
        if texts.len() > Self::CLOUD_BATCH_CHUNK_SIZE {
            let mut all_embeddings = Vec::with_capacity(texts.len());
            for chunk in texts.chunks(Self::CLOUD_BATCH_CHUNK_SIZE) {
                all_embeddings.extend(self.embed_many(chunk)?);
            }
            return Ok(all_embeddings);
        }

        match self.provider {
            #[cfg(feature = "openai")]
            ProviderKind::OpenAI => {
                let body = crate::provider::openai::build_embed_request_json(&self.model, texts);
                let agent = ureq::Agent::new_with_config(
                    ureq::config::Config::builder()
                        .timeout_global(Some(std::time::Duration::from_secs(60)))
                        .build(),
                );
                let mut response = agent
                    .post(crate::provider::openai::EMBED_API_URL)
                    .header("Authorization", &format!("Bearer {}", self.api_key))
                    .header("content-type", "application/json")
                    .send(&body)
                    .map_err(|e| map_http_error("OpenAI", e))?;

                let response_body = response.body_mut().read_to_string().map_err(|e| {
                    InferenceError::Provider(format!("OpenAI: failed to read embed response: {e}"))
                })?;

                let embeddings =
                    crate::provider::openai::parse_embed_response_json(&response_body)?;
                Ok(embeddings.into_iter().map(l2_normalize).collect())
            }

            #[cfg(feature = "google")]
            ProviderKind::Google => {
                let url = crate::provider::google::build_batch_embed_url(&self.model);
                let body =
                    crate::provider::google::build_batch_embed_request_json(&self.model, texts);
                let agent = ureq::Agent::new_with_config(
                    ureq::config::Config::builder()
                        .timeout_global(Some(std::time::Duration::from_secs(60)))
                        .build(),
                );
                let mut response = agent
                    .post(&url)
                    .header("x-goog-api-key", &self.api_key)
                    .header("content-type", "application/json")
                    .send(&body)
                    .map_err(|e| map_http_error("Google", e))?;

                let response_body = response.body_mut().read_to_string().map_err(|e| {
                    InferenceError::Provider(format!("Google: failed to read embed response: {e}"))
                })?;

                let embeddings =
                    crate::provider::google::parse_batch_embed_response_json(&response_body)?;
                Ok(embeddings.into_iter().map(l2_normalize).collect())
            }

            other => Err(InferenceError::NotSupported(format!(
                "cloud embedding not supported for provider: {other}"
            ))),
        }
    }
}

impl crate::InferenceEngine for CloudEmbeddingEngine {
    fn embed(&self, text: &str) -> Result<Vec<f32>, InferenceError> {
        self.embed_single(text)
    }

    fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, InferenceError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        if texts.len() == 1 {
            return self.embed_single(texts[0]).map(|e| vec![e]);
        }
        self.embed_many(texts)
    }

    fn supports_embed(&self) -> bool {
        true
    }

    fn embedding_dim(&self) -> usize {
        match self.model.as_str() {
            "text-embedding-3-small" => 1536,
            "text-embedding-3-large" => 3072,
            "text-embedding-ada-002" => 1536,
            "text-embedding-004" => 768, // Google
            "gemini-embedding-001" => 3072,
            "gemini-embedding-2" => 3072,
            _ => 0,
        }
    }

    fn is_healthy(&self) -> bool {
        true
    }
}

// Compile-time verify Send.
const _: () = {
    fn assert_send<T: Send>() {}
    fn assert_both() {
        assert_send::<CloudEmbeddingEngine>();
    }
    let _ = assert_both;
};

/// L2-normalize an embedding vector.
///
/// Returns the input unchanged if the norm is zero (all-zero vector).
fn l2_normalize(mut v: Vec<f32>) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

/// Map ureq HTTP errors to InferenceError::Provider.
#[cfg(any(feature = "openai", feature = "google"))]
fn map_http_error(provider: &str, err: ureq::Error) -> InferenceError {
    match &err {
        ureq::Error::StatusCode(status) => {
            let code = *status;
            let description = match code {
                401 => "invalid API key",
                403 => "forbidden (check API key permissions)",
                429 => "rate limited (too many requests)",
                500 => "server error",
                502 => "bad gateway",
                503 => "service unavailable",
                _ => "HTTP error",
            };
            // D6: the status IS the classification. Describing it in prose and
            // matching the prose back is how "invalid API key" became
            // indistinguishable from an outage.
            InferenceError::ProviderFailed {
                kind: ProviderFailure::from_http_status(code),
                message: format!("{provider}: {description} (HTTP {code})"),
            }
        }
        // The transport already told us it timed out.
        ureq::Error::Timeout(_) => InferenceError::ProviderFailed {
            kind: ProviderFailure::Timeout,
            message: format!("{provider}: {err}"),
        },
        _ => InferenceError::ProviderFailed {
            kind: ProviderFailure::Unavailable,
            message: format!("{provider}: {err}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The transport's own verdict decides the kind (D6).
    ///
    /// `map_http_error` turns a real `ureq::Error` into a classified failure,
    /// and nothing tested it: deleting the `Timeout` arm let a timeout fall
    /// through to the catch-all and report as an outage, which is the exact
    /// collapse D6 exists to prevent — an outage invites a retry against a
    /// server that is fine, while a timeout points at the deadline.
    ///
    /// `from_http_status` had a truth table, but a truth table on a pure
    /// function proves nothing about the match that feeds it.
    #[test]
    fn a_transport_timeout_is_a_timeout_not_an_outage() {
        let timeout = map_http_error("p", ureq::Error::Timeout(ureq::Timeout::Global));
        assert_eq!(timeout.code(), "inference.provider_timeout");

        // The catch-all still reports an outage, so the assertion above is
        // about the arm and not about every error becoming a timeout.
        let other = map_http_error("p", ureq::Error::HostNotFound);
        assert_eq!(other.code(), "inference.provider_unavailable");
    }

    /// A status code reaches the classifier that reads it.
    #[test]
    fn an_http_status_is_classified_by_its_status() {
        for (status, expected) in [
            (401, "inference.provider_auth_failed"),
            (403, "inference.provider_auth_failed"),
            (429, "inference.provider_rate_limited"),
            (503, "inference.provider_unavailable"),
        ] {
            let error = map_http_error("p", ureq::Error::StatusCode(status));
            assert_eq!(error.code(), expected, "HTTP {status}");
            assert!(
                error.to_string().contains(&status.to_string()),
                "the message names the status: {error}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    #[cfg(feature = "openai")]
    #[test]
    fn new_openai_with_valid_args() {
        let engine = CloudEmbeddingEngine::new(
            ProviderKind::OpenAI,
            "sk-test-key".into(),
            "text-embedding-3-small".into(),
        );
        assert!(engine.is_ok());
        let engine = engine.unwrap();
        assert_eq!(engine.provider(), ProviderKind::OpenAI);
        assert_eq!(engine.model(), "text-embedding-3-small");
    }

    #[cfg(feature = "google")]
    #[test]
    fn new_google_with_valid_args() {
        let engine = CloudEmbeddingEngine::new(
            ProviderKind::Google,
            "AIza-test-key".into(),
            "text-embedding-004".into(),
        );
        assert!(engine.is_ok());
        let engine = engine.unwrap();
        assert_eq!(engine.provider(), ProviderKind::Google);
        assert_eq!(engine.model(), "text-embedding-004");
    }

    #[test]
    fn new_anthropic_returns_not_supported() {
        let result =
            CloudEmbeddingEngine::new(ProviderKind::Anthropic, "sk-ant-key".into(), "model".into());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, InferenceError::NotSupported(_)),
            "should be NotSupported, got: {err}"
        );
        assert!(err.to_string().contains("Anthropic"));
    }

    #[test]
    fn new_local_returns_not_supported() {
        let result = CloudEmbeddingEngine::new(ProviderKind::Local, "key".into(), "model".into());
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            InferenceError::NotSupported(_)
        ));
    }

    #[cfg(any(feature = "openai", feature = "google"))]
    #[test]
    fn new_empty_key_returns_provider_error() {
        let provider = if cfg!(feature = "openai") {
            ProviderKind::OpenAI
        } else {
            ProviderKind::Google
        };
        let result = CloudEmbeddingEngine::new(provider, "".into(), "model".into());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), InferenceError::Provider(_)));
    }

    #[cfg(any(feature = "openai", feature = "google"))]
    #[test]
    fn new_whitespace_key_returns_provider_error() {
        let provider = if cfg!(feature = "openai") {
            ProviderKind::OpenAI
        } else {
            ProviderKind::Google
        };
        let result = CloudEmbeddingEngine::new(provider, "  \t".into(), "model".into());
        assert!(result.is_err());
    }

    #[cfg(any(feature = "openai", feature = "google"))]
    #[test]
    fn new_empty_model_returns_provider_error() {
        let provider = if cfg!(feature = "openai") {
            ProviderKind::OpenAI
        } else {
            ProviderKind::Google
        };
        let result = CloudEmbeddingEngine::new(provider, "key".into(), "".into());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), InferenceError::Provider(_)));
    }

    // -----------------------------------------------------------------------
    // InferenceEngine trait
    // -----------------------------------------------------------------------

    #[cfg(feature = "openai")]
    #[test]
    fn supports_embed_returns_true() {
        let engine =
            CloudEmbeddingEngine::new(ProviderKind::OpenAI, "sk-key".into(), "model".into())
                .unwrap();
        assert!(crate::InferenceEngine::supports_embed(&engine));
    }

    #[cfg(feature = "openai")]
    #[test]
    fn supports_generate_returns_false() {
        let engine =
            CloudEmbeddingEngine::new(ProviderKind::OpenAI, "sk-key".into(), "model".into())
                .unwrap();
        assert!(!crate::InferenceEngine::supports_generate(&engine));
    }

    #[cfg(feature = "openai")]
    #[test]
    fn generate_returns_not_supported() {
        let mut engine =
            CloudEmbeddingEngine::new(ProviderKind::OpenAI, "sk-key".into(), "model".into())
                .unwrap();
        let req = crate::GenerateRequest::default();
        let err = crate::InferenceEngine::generate(&mut engine, &req).unwrap_err();
        assert!(matches!(err, InferenceError::NotSupported(_)));
    }

    #[cfg(feature = "openai")]
    #[test]
    fn embed_batch_empty_returns_empty() {
        let engine =
            CloudEmbeddingEngine::new(ProviderKind::OpenAI, "sk-key".into(), "model".into())
                .unwrap();
        let result = crate::InferenceEngine::embed_batch(&engine, &[]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // -----------------------------------------------------------------------
    // Debug output
    // -----------------------------------------------------------------------

    #[cfg(feature = "openai")]
    #[test]
    fn debug_redacts_api_key() {
        let engine = CloudEmbeddingEngine::new(
            ProviderKind::OpenAI,
            "sk-secret-key-do-not-leak".into(),
            "text-embedding-3-small".into(),
        )
        .unwrap();
        let dbg = format!("{:?}", engine);
        assert!(
            !dbg.contains("sk-secret-key-do-not-leak"),
            "API key leaked in Debug: {dbg}"
        );
        assert!(dbg.contains("[REDACTED]"), "should show [REDACTED]: {dbg}");
        assert!(
            dbg.contains("text-embedding-3-small"),
            "should show model: {dbg}"
        );
        assert!(dbg.contains("OpenAI"), "should show provider: {dbg}");
    }

    // -----------------------------------------------------------------------
    // L2 normalization
    // -----------------------------------------------------------------------

    #[test]
    fn l2_normalize_unit_vector() {
        let v = l2_normalize(vec![1.0, 0.0, 0.0]);
        assert!((v[0] - 1.0).abs() < 1e-6);
        assert!((v[1]).abs() < 1e-6);
        assert!((v[2]).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_non_unit_vector() {
        let v = l2_normalize(vec![3.0, 4.0]);
        // norm = 5.0, normalized = [0.6, 0.8]
        assert!((v[0] - 0.6).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_zero_vector_unchanged() {
        let v = l2_normalize(vec![0.0, 0.0, 0.0]);
        assert_eq!(v, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn l2_normalize_already_normalized() {
        let v = l2_normalize(vec![0.6, 0.8]);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn l2_normalize_empty_vector() {
        let v = l2_normalize(vec![]);
        assert!(v.is_empty());
    }

    #[test]
    fn l2_normalize_negative_values() {
        let v = l2_normalize(vec![-3.0, 4.0]);
        // norm = 5.0, normalized = [-0.6, 0.8]
        assert!((v[0] - (-0.6)).abs() < 1e-6);
        assert!((v[1] - 0.8).abs() < 1e-6);
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }
}
