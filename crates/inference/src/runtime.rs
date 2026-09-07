//! Runtime facade for model execution.

#[cfg(any(
    feature = "local",
    feature = "anthropic",
    feature = "openai",
    feature = "google"
))]
use std::collections::HashMap;
#[cfg(feature = "local")]
use std::path::Path;
use std::path::PathBuf;
#[cfg(any(
    feature = "local",
    feature = "anthropic",
    feature = "openai",
    feature = "google"
))]
use std::sync::Mutex;

use crate::{
    generation_provider_feature_enabled, parse_model_spec, GenerateRequest, GenerateResponse,
    InferenceError, ModelInfo, ModelRegistry, ModelTask, ProviderKind,
};

#[cfg(any(feature = "openai", feature = "google"))]
use crate::embedding_provider_feature_enabled;
use crate::error::ProviderFailure;

#[cfg(any(feature = "openai", feature = "google"))]
use crate::InferenceEngine;

#[cfg(any(feature = "anthropic", feature = "openai", feature = "google"))]
use crate::api_key_env_var;

#[cfg(any(
    feature = "local",
    feature = "anthropic",
    feature = "openai",
    feature = "google"
))]
use crate::GenerationEngine;

#[cfg(any(feature = "openai", feature = "google"))]
use crate::CloudEmbeddingEngine;

#[cfg(feature = "local")]
use crate::{EmbeddingEngine, RankingEngine};

/// Runtime configuration for model execution.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wire-schemas", derive(schemars::JsonSchema))]
pub struct InferenceRuntimeConfig {
    /// Optional model directory override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models_dir: Option<PathBuf>,
    /// Whether provider network calls and model downloads are allowed.
    pub network_enabled: bool,
}

impl Default for InferenceRuntimeConfig {
    fn default() -> Self {
        Self {
            models_dir: None,
            network_enabled: true,
        }
    }
}

/// Model cache facts exposed for diagnostics.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wire-schemas", derive(schemars::JsonSchema))]
pub struct ModelCacheStatus {
    /// Cached generation model specs.
    pub generation_models: Vec<String>,
    /// Cached embedding model specs.
    pub embedding_models: Vec<String>,
    /// Cached ranking model specs.
    pub ranking_models: Vec<String>,
}

/// Whether one provider can be used right now, and if not, why not.
///
/// Never carries a key — only where one was found (#3124/D11).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wire-schemas", derive(schemars::JsonSchema))]
pub struct ProviderStatus {
    /// Which provider this row describes.
    pub provider: ProviderKind,
    /// Whether this binary was compiled with the provider.
    pub feature_enabled: bool,
    /// Whether this provider needs an API key at all.
    pub requires_api_key: bool,
    /// Whether a key was found. Never the key itself.
    pub key_present: bool,
    /// The environment variable this provider reads its key from, whether or
    /// not one is set — so a caller can say what to do about a missing key.
    /// `None` for providers that need no key.
    pub key_env_var: Option<String>,
    /// Where the key was found, when one was. An environment variable name,
    /// never a value. `None` when no key is present.
    pub key_source: Option<String>,
    /// Whether a call could be attempted right now.
    pub ready: bool,
    /// The model-spec prefix that selects this provider, e.g. `"openai:"`.
    ///
    /// A caller that finds a ready provider can use this directly: prefix any
    /// model name with it. That is the actionable form for a coding agent,
    /// which reads this over the human table.
    pub model_prefix: String,
}

/// What this binary can do before anything is attempted (D11).
///
/// #3124: every part of this was knowable up front and reported nowhere, so a
/// user learned their build's limits by watching an operation fail.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wire-schemas", derive(schemars::JsonSchema))]
pub struct InferenceStatus {
    /// Whether this binary can execute local models.
    pub local_execution: bool,
    /// Whether this binary can download model artifacts.
    pub model_download: bool,
    /// Every provider, in a stable order.
    pub providers: Vec<ProviderStatus>,
    /// The model directory, shared by every database on this machine.
    pub models_dir: PathBuf,
    /// Catalogued models whose artifact is already on disk.
    pub models_downloaded: usize,
    /// Catalogued models in total.
    pub models_catalogued: usize,
    /// What to do when local execution is needed and absent. `None` when this
    /// build already has it.
    pub local_remedy: Option<String>,
}

/// Provider/model capability facts.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wire-schemas", derive(schemars::JsonSchema))]
pub struct InferenceCapability {
    /// Provider kind.
    pub provider: ProviderKind,
    /// Model name or path after provider parsing.
    pub model: String,
    /// Whether **this binary** can generate with this model right now.
    ///
    /// False when the provider's feature is compiled out, even for a model
    /// that inherently generates — see `provider_feature_enabled` for why. The
    /// model's own task is in the catalog (`inference models list`).
    pub can_generate: bool,
    /// Whether **this binary** can tokenize with this model right now.
    pub can_tokenize: bool,
    /// Whether **this binary** can embed with this model right now.
    pub can_embed: bool,
    /// Whether **this binary** can rank with this model right now.
    pub can_rank: bool,
    /// Whether the operation requires network access.
    pub requires_network: bool,
    /// Whether the provider requires an API key.
    pub requires_api_key: bool,
    /// Whether this binary was compiled with the provider feature needed for execution.
    pub provider_feature_enabled: bool,
    /// Whether this runtime configuration currently allows network access.
    pub network_enabled: bool,
    /// Known embedding dimension, if available.
    pub embedding_dim: usize,
    /// Whether chat requests may offer `tools` (function calling).
    pub supports_tools: bool,
    /// Whether `response_format: json_object` is honored.
    pub supports_json_object: bool,
    /// Whether `response_format: json_schema` (structured output) is honored.
    pub supports_json_schema: bool,
    /// Whether `logprobs` are returned in the response.
    pub supports_logprobs: bool,
}

/// Pull-model command output.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wire-schemas", derive(schemars::JsonSchema))]
pub struct PullModelOutput {
    /// Requested model spec.
    pub model: String,
    /// Local path containing the resolved GGUF file.
    pub path: PathBuf,
}

/// Embedding request.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wire-schemas", derive(schemars::JsonSchema))]
pub struct EmbedRequest {
    /// Text to embed.
    pub text: String,
}

/// Batch embedding response.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wire-schemas", derive(schemars::JsonSchema))]
pub struct EmbedResponse {
    /// Ordered embedding item outcomes.
    pub items: Vec<EmbedRuntimeOutcome>,
    /// Embedding dimension when known.
    pub dimension: usize,
}

/// Embedding item outcome.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wire-schemas", derive(schemars::JsonSchema))]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum EmbedRuntimeOutcome {
    /// Successful embedding item.
    Ok {
        /// Embedding vector.
        vector: Vec<f32>,
    },
    /// Failed embedding item.
    Error {
        /// Stable error code.
        code: String,
        /// Redacted public error message.
        message: String,
    },
}

/// Ranking request.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wire-schemas", derive(schemars::JsonSchema))]
pub struct RankRequest {
    /// Query text.
    pub query: String,
    /// Candidate passages.
    pub passages: Vec<String>,
}

/// Ranking response.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wire-schemas", derive(schemars::JsonSchema))]
pub struct RankResponse {
    /// Ordered ranking item outcomes.
    pub items: Vec<RankRuntimeOutcome>,
}

/// Ranking item outcome.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "wire-schemas", derive(schemars::JsonSchema))]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RankRuntimeOutcome {
    /// Successful ranking score.
    Ok {
        /// Passage index.
        index: usize,
        /// Relevance score.
        score: f32,
    },
    /// Failed ranking item.
    Error {
        /// Passage index.
        index: usize,
        /// Stable error code.
        code: String,
        /// Redacted public error message.
        message: String,
    },
}

/// Public runtime facade.
#[derive(Debug)]
pub struct InferenceRuntime {
    config: InferenceRuntimeConfig,
    #[cfg(any(
        feature = "local",
        feature = "anthropic",
        feature = "openai",
        feature = "google"
    ))]
    generation: Mutex<HashMap<String, GenerationEngine>>,
    #[cfg(feature = "local")]
    embeddings: Mutex<HashMap<String, EmbeddingEngine>>,
    #[cfg(feature = "local")]
    rankers: Mutex<HashMap<String, RankingEngine>>,
}

impl Default for InferenceRuntime {
    fn default() -> Self {
        Self::new(InferenceRuntimeConfig::default())
    }
}

impl InferenceRuntime {
    /// Creates a runtime facade.
    pub fn new(config: InferenceRuntimeConfig) -> Self {
        Self {
            config,
            #[cfg(any(
                feature = "local",
                feature = "anthropic",
                feature = "openai",
                feature = "google"
            ))]
            generation: Mutex::new(HashMap::new()),
            #[cfg(feature = "local")]
            embeddings: Mutex::new(HashMap::new()),
            #[cfg(feature = "local")]
            rankers: Mutex::new(HashMap::new()),
        }
    }

    /// Returns catalog model information.
    pub fn list_models(&self) -> Vec<ModelInfo> {
        self.registry().list_available()
    }

    /// Returns locally available catalog model information.
    pub fn list_local_models(&self) -> Vec<ModelInfo> {
        self.registry().list_local()
    }

    /// Resolves or downloads a model into the local model directory.
    pub fn pull_model(&self, model: &str) -> Result<PullModelOutput, InferenceError> {
        if !self.config.network_enabled {
            return Err(InferenceError::NotSupported(
                "model download requires network access".to_owned(),
            ));
        }

        #[cfg(feature = "download")]
        {
            let path = self.registry().resolve_or_pull(model)?;
            Ok(PullModelOutput {
                model: model.to_owned(),
                path,
            })
        }

        #[cfg(not(feature = "download"))]
        {
            let _ = model;
            Err(download_feature_unavailable())
        }
    }

    /// Reports what this binary can do, before anything is attempted.
    ///
    /// Answers in one place the questions that previously had to be inferred
    /// from a failure: which providers are compiled in, which have a key and
    /// where it came from, whether local execution exists in this build, and
    /// how many catalogued models are already on disk.
    pub fn status(&self) -> InferenceStatus {
        let providers = [
            ProviderKind::OpenAI,
            ProviderKind::Anthropic,
            ProviderKind::Google,
            ProviderKind::Local,
        ]
        .into_iter()
        .map(|provider| {
            let feature_enabled = generation_provider_feature_enabled(provider)
                || embedding_provider_feature_enabled_for_capability(provider);
            let requires_api_key = provider != ProviderKind::Local;
            let key_env_var = requires_api_key.then(|| api_key_env_var(provider).to_owned());
            let key_source = resolve_key_source(key_env_var.as_deref(), |name| {
                std::env::var_os(name).is_some_and(|value| !value.is_empty())
            });
            let key_present = key_source.is_some();
            ProviderStatus {
                provider,
                feature_enabled,
                requires_api_key,
                key_present,
                key_env_var,
                key_source,
                ready: feature_enabled && (key_present || !requires_api_key),
                model_prefix: format!("{provider}:"),
            }
        })
        .collect();

        let catalog = self.registry().list_available();
        InferenceStatus {
            local_execution: cfg!(feature = "local"),
            model_download: cfg!(feature = "download"),
            providers,
            models_dir: self.registry().models_dir().to_path_buf(),
            models_downloaded: catalog.iter().filter(|info| info.is_local).count(),
            models_catalogued: catalog.len(),
            local_remedy: (!cfg!(feature = "local")).then(|| LOCAL_UNAVAILABLE_REMEDY.to_owned()),
        }
    }

    /// Returns capability facts for a model spec.
    pub fn capability(&self, model_spec: &str) -> Result<InferenceCapability, InferenceError> {
        let (provider, model) = parse_model_spec(model_spec)?;
        let local_info = if provider == ProviderKind::Local {
            self.registry()
                .list_available()
                .into_iter()
                .find(|info| info.name.eq_ignore_ascii_case(&model))
        } else {
            None
        };
        let task = local_info.as_ref().map(|info| info.task);
        let abilities = ModelAbilities::of(provider, task);
        Ok(InferenceCapability {
            provider,
            model,
            // #3124: `can_*` answers "can THIS BINARY do this, now" — not
            // "does the model support it". The two diverge whenever a provider
            // feature is compiled out, and a released binary has `local` off:
            // reporting `can_embed: true` beside `provider_feature_enabled:
            // false` made every caller responsible for ANDing the two, and the
            // more prominent field was the wrong one. What the model inherently
            // is stays discoverable through the catalog's `task`.
            //
            // The two halves are separated deliberately: `ModelAbilities` is
            // what the model supports, decided by a pure function with its own
            // truth table, and the feature check is what this build can run.
            // Folded together, every branch of the first half was unobservable
            // in a build with the feature off (`x && false` is false whatever
            // `x` is), so the mutation gate could not distinguish them.
            can_generate: abilities.generate && generation_provider_feature_enabled(provider),
            can_tokenize: abilities.tokenize && cfg!(feature = "local"),
            can_embed: abilities.embed
                && embedding_provider_feature_enabled_for_capability(provider),
            can_rank: abilities.rank && cfg!(feature = "local"),
            requires_network: provider != ProviderKind::Local,
            requires_api_key: provider != ProviderKind::Local,
            provider_feature_enabled: generation_provider_feature_enabled(provider)
                || embedding_provider_feature_enabled_for_capability(provider),
            network_enabled: self.config.network_enabled,
            embedding_dim: local_info.map_or(0, |info| info.embedding_dim),
            // Chat feature support per provider. `json_object` is unsupported by
            // Anthropic (use json_schema); `logprobs` are unsupported by
            // Anthropic and local (deferred). Local structured outputs and tool
            // calling are grammar-based.
            supports_tools: true,
            supports_json_object: provider != ProviderKind::Anthropic,
            supports_json_schema: true,
            supports_logprobs: matches!(provider, ProviderKind::OpenAI | ProviderKind::Google),
        })
    }

    /// Generates text from a model spec.
    pub fn generate(
        &self,
        model_spec: &str,
        request: &GenerateRequest,
    ) -> Result<GenerateResponse, InferenceError> {
        #[cfg(any(
            feature = "local",
            feature = "anthropic",
            feature = "openai",
            feature = "google"
        ))]
        {
            let (provider, _model) = parse_model_spec(model_spec)?;
            if provider == ProviderKind::Local {
                let mut cache = self.lock_generation()?;
                let engine = self.cached_generation_engine(&mut cache, model_spec, None)?;
                return engine.generate(request);
            }

            if !self.config.network_enabled {
                return Err(InferenceError::NotSupported(
                    "cloud generation requires network access".to_owned(),
                ));
            }
            #[cfg(not(any(feature = "anthropic", feature = "openai", feature = "google")))]
            {
                Err(InferenceError::NotSupported(format!(
                    "{provider} provider not enabled"
                )))
            }
            #[cfg(any(feature = "anthropic", feature = "openai", feature = "google"))]
            {
                let mut engine = self.cloud_generation_engine(model_spec)?;
                engine.generate(request)
            }
        }

        #[cfg(not(any(
            feature = "local",
            feature = "anthropic",
            feature = "openai",
            feature = "google"
        )))]
        {
            let _ = (model_spec, request);
            Err(InferenceError::NotSupported(
                "generation requires a provider feature".to_owned(),
            ))
        }
    }

    /// Runs a chat/generation request (the OpenAI-shaped body).
    ///
    /// Local applies the model's chat template and the full sampler chain; cloud
    /// providers map messages natively (system prompt, multi-turn history,
    /// assistant prefill) and forward every knob the provider supports.
    /// `model_config` is threaded into local model loading (cache-keyed).
    pub fn chat(
        &self,
        model_spec: &str,
        request: &crate::wire::ChatRequest,
    ) -> Result<crate::wire::ChatResponse, InferenceError> {
        request.validate()?;

        #[cfg(any(
            feature = "local",
            feature = "anthropic",
            feature = "openai",
            feature = "google"
        ))]
        {
            let (provider, _model) = parse_model_spec(model_spec)?;
            if provider == ProviderKind::Local {
                let mut cache = self.lock_generation()?;
                let engine = self.cached_generation_engine(
                    &mut cache,
                    model_spec,
                    request.model_config.as_ref(),
                )?;
                let mut response = engine.generate_chat(request)?;
                response.model = model_spec.to_string();
                return Ok(response);
            }

            if !self.config.network_enabled {
                return Err(InferenceError::NotSupported(
                    "cloud generation requires network access".to_owned(),
                ));
            }
            #[cfg(not(any(feature = "anthropic", feature = "openai", feature = "google")))]
            {
                Err(InferenceError::NotSupported(format!(
                    "{provider} provider not enabled"
                )))
            }
            #[cfg(any(feature = "anthropic", feature = "openai", feature = "google"))]
            {
                let mut engine = self.cloud_generation_engine(model_spec)?;
                let mut response = engine.generate_chat(request)?;
                response.model = model_spec.to_string();
                Ok(response)
            }
        }

        #[cfg(not(any(
            feature = "local",
            feature = "anthropic",
            feature = "openai",
            feature = "google"
        )))]
        {
            let _ = (model_spec, request);
            Err(InferenceError::NotSupported(
                "chat requires a provider feature".to_owned(),
            ))
        }
    }

    /// Runs an embeddings request (single or batch, OpenAI-shaped).
    ///
    /// Phase A bridges to [`Self::embed_batch`]; `dimensions`/`normalize`/
    /// `input_type` are accepted but applied in a later phase.
    pub fn embeddings(
        &self,
        model_spec: &str,
        request: &crate::wire::EmbeddingsRequest,
    ) -> Result<crate::wire::EmbeddingsResponse, InferenceError> {
        let texts = request.input.to_vec();
        let batch = self.embed_batch(model_spec, &texts)?;
        let mut data = Vec::with_capacity(batch.items.len());
        for (index, item) in batch.items.into_iter().enumerate() {
            match item {
                EmbedRuntimeOutcome::Ok { vector } => data.push(crate::wire::EmbeddingItem {
                    index: index as u32,
                    embedding: vector,
                }),
                EmbedRuntimeOutcome::Error { code, message } => {
                    return Err(InferenceError::Provider(format!(
                        "embedding item {index} failed [{code}]: {message}"
                    )));
                }
            }
        }
        Ok(crate::wire::EmbeddingsResponse {
            model: model_spec.to_string(),
            dimension: batch.dimension,
            data,
            usage: crate::wire::Usage::default(),
        })
    }

    /// Tokenizes text with a local generation model.
    pub fn tokenize(
        &self,
        model_spec: &str,
        text: &str,
        add_special: bool,
    ) -> Result<Vec<u32>, InferenceError> {
        #[cfg(feature = "local")]
        {
            let mut cache = self.lock_generation()?;
            let engine = self.cached_generation_engine(&mut cache, model_spec, None)?;
            engine.encode(text, add_special)
        }

        #[cfg(not(feature = "local"))]
        {
            let _ = (model_spec, text, add_special);
            Err(local_feature_unavailable("tokenization"))
        }
    }

    /// Detokenizes local token ids.
    pub fn detokenize(&self, model_spec: &str, ids: &[u32]) -> Result<String, InferenceError> {
        #[cfg(feature = "local")]
        {
            let mut cache = self.lock_generation()?;
            let engine = self.cached_generation_engine(&mut cache, model_spec, None)?;
            engine.decode(ids)
        }

        #[cfg(not(feature = "local"))]
        {
            let _ = (model_spec, ids);
            Err(local_feature_unavailable("detokenization"))
        }
    }

    /// Embeds one text.
    pub fn embed(
        &self,
        model_spec: &str,
        request: &EmbedRequest,
    ) -> Result<Vec<f32>, InferenceError> {
        #[cfg(any(feature = "local", feature = "openai", feature = "google"))]
        {
            let (provider, _model) = parse_model_spec(model_spec)?;
            if provider == ProviderKind::Local {
                #[cfg(feature = "local")]
                {
                    let mut cache = self.lock_embeddings()?;
                    let engine = self.cached_embedding_engine(&mut cache, model_spec)?;
                    return engine.embed(&request.text);
                }
                #[cfg(not(feature = "local"))]
                {
                    return Err(local_feature_unavailable("local embedding"));
                }
            }

            if !self.config.network_enabled {
                return Err(InferenceError::NotSupported(
                    "cloud embedding requires network access".to_owned(),
                ));
            }
            #[cfg(any(feature = "openai", feature = "google"))]
            {
                let engine = self.cloud_embedding_engine(model_spec)?;
                engine.embed(&request.text)
            }
            #[cfg(not(any(feature = "openai", feature = "google")))]
            {
                Err(InferenceError::NotSupported(
                    "cloud embedding requires openai or google feature".to_owned(),
                ))
            }
        }

        #[cfg(not(any(feature = "local", feature = "openai", feature = "google")))]
        {
            let _ = (model_spec, request);
            Err(InferenceError::NotSupported(
                "embedding requires local, openai, or google feature".to_owned(),
            ))
        }
    }

    /// Embeds a batch of texts.
    pub fn embed_batch(
        &self,
        model_spec: &str,
        texts: &[String],
    ) -> Result<EmbedResponse, InferenceError> {
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();

        #[cfg(any(feature = "local", feature = "openai", feature = "google"))]
        {
            let (provider, _model) = parse_model_spec(model_spec)?;
            let embeddings = if provider == ProviderKind::Local {
                #[cfg(feature = "local")]
                {
                    let mut cache = self.lock_embeddings()?;
                    let engine = self.cached_embedding_engine(&mut cache, model_spec)?;
                    engine.embed_batch(&refs)?
                }
                #[cfg(not(feature = "local"))]
                {
                    return Err(local_feature_unavailable("local embedding"));
                }
            } else {
                if !self.config.network_enabled {
                    return Err(InferenceError::NotSupported(
                        "cloud embedding requires network access".to_owned(),
                    ));
                }
                #[cfg(any(feature = "openai", feature = "google"))]
                {
                    let engine = self.cloud_embedding_engine(model_spec)?;
                    engine.embed_batch(&refs)?
                }
                #[cfg(not(any(feature = "openai", feature = "google")))]
                {
                    return Err(InferenceError::NotSupported(
                        "cloud embedding requires openai or google feature".to_owned(),
                    ));
                }
            };
            let dimension = embeddings.first().map_or(0, Vec::len);
            Ok(EmbedResponse {
                dimension,
                items: embeddings
                    .into_iter()
                    .map(|vector| EmbedRuntimeOutcome::Ok { vector })
                    .collect(),
            })
        }

        #[cfg(not(any(feature = "local", feature = "openai", feature = "google")))]
        {
            let _ = (model_spec, refs);
            Err(InferenceError::NotSupported(
                "embedding requires local, openai, or google feature".to_owned(),
            ))
        }
    }

    /// Ranks passages against a query.
    pub fn rank(
        &self,
        model_spec: &str,
        request: &RankRequest,
    ) -> Result<RankResponse, InferenceError> {
        #[cfg(feature = "local")]
        {
            let refs: Vec<&str> = request.passages.iter().map(String::as_str).collect();
            let mut cache = self.lock_rankers()?;
            let engine = self.cached_ranking_engine(&mut cache, model_spec)?;
            let scores = engine.rank(&request.query, &refs)?;
            Ok(RankResponse {
                items: scores
                    .into_iter()
                    .enumerate()
                    .map(|(index, score)| RankRuntimeOutcome::Ok { index, score })
                    .collect(),
            })
        }

        #[cfg(not(feature = "local"))]
        {
            let _ = (model_spec, request);
            Err(local_feature_unavailable("ranking"))
        }
    }

    /// Unloads cached local models. When `model_spec` is `None`, all caches are cleared.
    pub fn unload(&self, _model_spec: Option<&str>) -> Result<bool, InferenceError> {
        #[cfg(any(
            feature = "local",
            feature = "anthropic",
            feature = "openai",
            feature = "google"
        ))]
        let mut unloaded = false;
        #[cfg(not(any(
            feature = "local",
            feature = "anthropic",
            feature = "openai",
            feature = "google"
        )))]
        let unloaded = false;
        #[cfg(any(
            feature = "local",
            feature = "anthropic",
            feature = "openai",
            feature = "google"
        ))]
        {
            let mut cache = self.lock_generation()?;
            unloaded |= remove_matching(&mut cache, _model_spec);
        }
        #[cfg(feature = "local")]
        {
            let mut cache = self.lock_embeddings()?;
            unloaded |= remove_matching(&mut cache, _model_spec);
            let mut cache = self.lock_rankers()?;
            unloaded |= remove_matching(&mut cache, _model_spec);
        }
        Ok(unloaded)
    }

    /// Returns model cache status.
    pub fn cache_status(&self) -> Result<ModelCacheStatus, InferenceError> {
        Ok(ModelCacheStatus {
            generation_models: {
                #[cfg(any(
                    feature = "local",
                    feature = "anthropic",
                    feature = "openai",
                    feature = "google"
                ))]
                {
                    let cache = self.lock_generation()?;
                    sorted_keys(&cache)
                }
                #[cfg(not(any(
                    feature = "local",
                    feature = "anthropic",
                    feature = "openai",
                    feature = "google"
                )))]
                {
                    Vec::new()
                }
            },
            embedding_models: {
                #[cfg(feature = "local")]
                {
                    let cache = self.lock_embeddings()?;
                    sorted_keys(&cache)
                }
                #[cfg(not(feature = "local"))]
                {
                    Vec::new()
                }
            },
            ranking_models: {
                #[cfg(feature = "local")]
                {
                    let cache = self.lock_rankers()?;
                    sorted_keys(&cache)
                }
                #[cfg(not(feature = "local"))]
                {
                    Vec::new()
                }
            },
        })
    }

    fn registry(&self) -> ModelRegistry {
        self.config
            .models_dir
            .clone()
            .map_or_else(ModelRegistry::new, ModelRegistry::with_dir)
    }

    #[cfg(any(
        feature = "local",
        feature = "anthropic",
        feature = "openai",
        feature = "google"
    ))]
    fn lock_generation(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, GenerationEngine>>, InferenceError> {
        self.generation
            .lock()
            .map_err(|err| InferenceError::Io(format!("generation cache mutex poisoned: {err}")))
    }

    #[cfg(feature = "local")]
    fn lock_embeddings(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, EmbeddingEngine>>, InferenceError> {
        self.embeddings
            .lock()
            .map_err(|err| InferenceError::Io(format!("embedding cache mutex poisoned: {err}")))
    }

    #[cfg(feature = "local")]
    fn lock_rankers(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, RankingEngine>>, InferenceError> {
        self.rankers
            .lock()
            .map_err(|err| InferenceError::Io(format!("ranking cache mutex poisoned: {err}")))
    }

    #[cfg(any(
        feature = "local",
        feature = "anthropic",
        feature = "openai",
        feature = "google"
    ))]
    fn cached_generation_engine<'a>(
        &self,
        cache: &'a mut HashMap<String, GenerationEngine>,
        model_spec: &str,
        config: Option<&crate::wire::ModelConfig>,
    ) -> Result<&'a mut GenerationEngine, InferenceError> {
        let key = engine_cache_key(model_spec, config);
        if !cache.contains_key(&key) {
            let engine = self.local_generation_engine(model_spec, config)?;
            cache.insert(key.clone(), engine);
        }
        Ok(cache
            .get_mut(&key)
            .expect("generation engine inserted before lookup"))
    }

    #[cfg(feature = "local")]
    fn cached_embedding_engine<'a>(
        &self,
        cache: &'a mut HashMap<String, EmbeddingEngine>,
        model_spec: &str,
    ) -> Result<&'a EmbeddingEngine, InferenceError> {
        if !cache.contains_key(model_spec) {
            let engine = self.local_embedding_engine(model_spec)?;
            cache.insert(model_spec.to_owned(), engine);
        }
        Ok(cache
            .get(model_spec)
            .expect("embedding engine inserted before lookup"))
    }

    #[cfg(feature = "local")]
    fn cached_ranking_engine<'a>(
        &self,
        cache: &'a mut HashMap<String, RankingEngine>,
        model_spec: &str,
    ) -> Result<&'a RankingEngine, InferenceError> {
        if !cache.contains_key(model_spec) {
            let engine = self.local_ranking_engine(model_spec)?;
            cache.insert(model_spec.to_owned(), engine);
        }
        Ok(cache
            .get(model_spec)
            .expect("ranking engine inserted before lookup"))
    }

    #[cfg(any(
        feature = "local",
        feature = "anthropic",
        feature = "openai",
        feature = "google"
    ))]
    fn local_generation_engine(
        &self,
        model_spec: &str,
        config: Option<&crate::wire::ModelConfig>,
    ) -> Result<GenerationEngine, InferenceError> {
        let (provider, model) = parse_model_spec(model_spec)?;
        if provider != ProviderKind::Local {
            return Err(InferenceError::NotSupported(
                "generation cache only stores local models".to_owned(),
            ));
        }
        #[cfg(feature = "local")]
        {
            if looks_like_path(&model) {
                return GenerationEngine::from_gguf_with_config(Path::new(&model), config);
            }
            GenerationEngine::from_registry_with_config(&model, config)
        }
        #[cfg(not(feature = "local"))]
        {
            let _ = (model, config);
            Err(local_feature_unavailable("local generation"))
        }
    }

    #[cfg(any(feature = "anthropic", feature = "openai", feature = "google"))]
    fn cloud_generation_engine(
        &self,
        model_spec: &str,
    ) -> Result<GenerationEngine, InferenceError> {
        let (provider, model) = parse_model_spec(model_spec)?;
        if !generation_provider_feature_enabled(provider) {
            return Err(InferenceError::NotSupported(format!(
                "{provider} provider not enabled"
            )));
        }
        let api_key = api_key(provider)?;
        GenerationEngine::cloud(provider, api_key, model)
    }

    #[cfg(feature = "local")]
    fn local_embedding_engine(&self, model_spec: &str) -> Result<EmbeddingEngine, InferenceError> {
        let (provider, model) = parse_model_spec(model_spec)?;
        if provider != ProviderKind::Local {
            return Err(InferenceError::NotSupported(
                "local embedding requires local provider".to_owned(),
            ));
        }
        if looks_like_path(&model) {
            return EmbeddingEngine::from_gguf(Path::new(&model));
        }
        EmbeddingEngine::from_registry(&model)
    }

    #[cfg(feature = "local")]
    fn local_ranking_engine(&self, model_spec: &str) -> Result<RankingEngine, InferenceError> {
        let (provider, model) = parse_model_spec(model_spec)?;
        if provider != ProviderKind::Local {
            return Err(InferenceError::NotSupported(
                "local ranking requires local provider".to_owned(),
            ));
        }
        if looks_like_path(&model) {
            return RankingEngine::from_gguf(Path::new(&model));
        }
        RankingEngine::from_registry(&model)
    }

    #[cfg(any(feature = "openai", feature = "google"))]
    fn cloud_embedding_engine(
        &self,
        model_spec: &str,
    ) -> Result<CloudEmbeddingEngine, InferenceError> {
        let (provider, model) = parse_model_spec(model_spec)?;
        if !embedding_provider_feature_enabled(provider) {
            return Err(InferenceError::NotSupported(format!(
                "{provider} embedding provider not enabled"
            )));
        }
        let api_key = api_key(provider)?;
        CloudEmbeddingEngine::new(provider, api_key, model)
    }
}

// Delegates to the inherent methods above — Rust resolves `self.method()` to the
// inherent method (preferred over trait methods), so there is no recursion.
impl crate::InferenceService for InferenceRuntime {
    fn list_models(&self) -> Vec<crate::ModelInfo> {
        self.list_models()
    }

    fn list_local_models(&self) -> Vec<crate::ModelInfo> {
        self.list_local_models()
    }

    fn pull_model(&self, model: &str) -> Result<crate::PullModelOutput, InferenceError> {
        self.pull_model(model)
    }

    fn capability(&self, model_spec: &str) -> Result<crate::InferenceCapability, InferenceError> {
        self.capability(model_spec)
    }

    fn chat(
        &self,
        model_spec: &str,
        request: &crate::wire::ChatRequest,
    ) -> Result<crate::wire::ChatResponse, InferenceError> {
        self.chat(model_spec, request)
    }

    fn tokenize(
        &self,
        model_spec: &str,
        text: &str,
        add_special: bool,
    ) -> Result<Vec<u32>, InferenceError> {
        self.tokenize(model_spec, text, add_special)
    }

    fn detokenize(&self, model_spec: &str, ids: &[u32]) -> Result<String, InferenceError> {
        self.detokenize(model_spec, ids)
    }

    fn embeddings(
        &self,
        model_spec: &str,
        request: &crate::wire::EmbeddingsRequest,
    ) -> Result<crate::wire::EmbeddingsResponse, InferenceError> {
        self.embeddings(model_spec, request)
    }

    fn rank(
        &self,
        model_spec: &str,
        request: &crate::RankRequest,
    ) -> Result<crate::RankResponse, InferenceError> {
        self.rank(model_spec, request)
    }

    fn unload(&self, model_spec: Option<&str>) -> Result<bool, InferenceError> {
        self.unload(model_spec)
    }

    fn cache_status(&self) -> Result<crate::ModelCacheStatus, InferenceError> {
        self.cache_status()
    }

    fn status(&self) -> InferenceStatus {
        self.status()
    }
}

#[cfg(any(feature = "anthropic", feature = "openai", feature = "google"))]
fn api_key(provider: ProviderKind) -> Result<String, InferenceError> {
    if provider == ProviderKind::Local {
        return Err(InferenceError::NotSupported(
            "local provider does not use API keys".to_owned(),
        ));
    }
    let env_var = api_key_env_var(provider);
    std::env::var(env_var).map_err(|_| {
        // D6: this used to carry a comment telling future editors to keep the
        // words "API key" and "not set" in the message, because that is what
        // made it classify as `inference.missing_api_key`. The constraint is
        // gone: the kind is carried explicitly, so this text can say whatever
        // serves the reader. That comment was the clearest evidence #3216 is
        // worth fixing — the classification was a property of the prose.
        let provider_name = provider.to_string();
        let message = match crate::provider_key_info(&provider_name) {
            Some(info) => format!(
                "{env_var} is not set: the {provider} API key is missing. Get a key at \
                 {url}, then set it with `strata config set {name}.api_key <KEY>` or by \
                 exporting {env_var}.",
                url = info.acquisition_url,
                name = info.provider,
            ),
            None => format!("{env_var} is not set: the {provider} API key is missing."),
        };
        InferenceError::ProviderFailed {
            kind: ProviderFailure::MissingApiKey,
            message,
        }
    })
}

/// Cache key for a loaded generation engine. A default or absent config shares
/// the plain-spec key (so tokenize/generate and a config-less chat reuse one
/// engine); a non-default config keys a distinct engine so its load params
/// (`n_ctx`/`n_gpu_layers`/`n_batch`/`n_threads`) take effect.
#[cfg(any(
    feature = "local",
    feature = "anthropic",
    feature = "openai",
    feature = "google"
))]
fn engine_cache_key(model_spec: &str, config: Option<&crate::wire::ModelConfig>) -> String {
    match config {
        Some(c) if *c != crate::wire::ModelConfig::default() => format!("{model_spec}\u{0}{c:?}"),
        _ => model_spec.to_owned(),
    }
}

#[cfg(feature = "local")]
fn looks_like_path(model: &str) -> bool {
    model.ends_with(".gguf")
        || model.contains('/')
        || model.contains('\\')
        || Path::new(model).exists()
}

#[cfg(any(
    feature = "local",
    feature = "anthropic",
    feature = "openai",
    feature = "google"
))]
fn sorted_keys<T>(map: &HashMap<String, T>) -> Vec<String> {
    let mut keys: Vec<String> = map.keys().cloned().collect();
    keys.sort();
    keys
}

#[cfg(any(
    feature = "local",
    feature = "anthropic",
    feature = "openai",
    feature = "google"
))]
fn remove_matching<T>(map: &mut HashMap<String, T>, model_spec: Option<&str>) -> bool {
    match model_spec {
        Some(model_spec) => map.remove(model_spec).is_some(),
        None => {
            let had_entries = !map.is_empty();
            map.clear();
            had_entries
        }
    }
}

/// What to tell a caller who needs local model execution and does not have it.
///
/// **One authoring site on purpose.** Seven refusal messages, the status
/// renderer, and the model listing all said this, and #3124 was partly about
/// them disagreeing. It is also the string that changes when D2 lands
/// `strata inference install-local` — at which point this becomes a command,
/// and only here.
///
/// It names `strata inference install-local` and never `cargo install`: the
/// callers of this surface are mostly coding agents, which have no Rust
/// toolchain and cannot answer a build prompt. A remediation an agent cannot
/// execute is the same dead end as no remediation at all.
///
/// It also states that **a bare model name means a local model**, because
/// without that the advice to "use a cloud model" is unactionable: the reader
/// has just named a model and is being told to name one, with no way to see
/// what should change. `miniLM` and `local:miniLM` are the same spec; the
/// prefix is the whole difference.
pub const LOCAL_UNAVAILABLE_REMEDY: &str =
    "this build runs cloud models only, and a bare model name means a local \
     model. Either name a cloud model instead — `openai:<model>`, \
     `google:<model>` or `anthropic:<model>` — or run `strata inference \
     install-local` to add local execution.";

/// Where a provider's key came from, given which variable it reads and whether
/// that variable holds anything.
///
/// A pure function on purpose: the alternative is a test that mutates the
/// process environment, which races every other test in the binary. It also
/// makes the one property that matters directly checkable — the result is the
/// variable's NAME, never its value, so `status` cannot leak a key.
fn resolve_key_source(env_var: Option<&str>, is_set: impl Fn(&str) -> bool) -> Option<String> {
    env_var.filter(|name| is_set(name)).map(str::to_owned)
}

/// What a model and provider inherently support, before any question of what
/// this binary was compiled with.
///
/// Split out of `capability` (#3124) so the decision has a truth table that can
/// be tested directly. Inside the `can_*` expressions each branch was ANDed
/// with a feature check, so in a build with that feature off the whole
/// expression was false regardless — every mutation of this logic was
/// equivalent, and the mutation gate rightly could not tell them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ModelAbilities {
    pub(crate) generate: bool,
    pub(crate) tokenize: bool,
    pub(crate) embed: bool,
    pub(crate) rank: bool,
}

impl ModelAbilities {
    /// `task` is the catalogued task for a local model, and `None` for a cloud
    /// spec (where the provider alone decides).
    pub(crate) fn of(provider: ProviderKind, task: Option<ModelTask>) -> Self {
        let local = provider == ProviderKind::Local;
        Self {
            // Every cloud provider generates. A local model generates unless it
            // is an embedding model.
            generate: !local || task != Some(ModelTask::Embed),
            // Tokenization is a property of a local GGUF; cloud providers do
            // not expose it.
            tokenize: local,
            // OpenAI and Google serve embedding endpoints; Anthropic does not.
            // A local model embeds when that is its catalogued task.
            embed: matches!(provider, ProviderKind::OpenAI | ProviderKind::Google)
                || task == Some(ModelTask::Embed),
            // Reranking is local-only, and only for a rank model.
            rank: local && task == Some(ModelTask::Rank),
        }
    }
}

/// One phrasing for "this binary was not built with local model execution".
///
/// #3124: seven sites each said it differently ("tokenization requires the
/// local feature", "local embedding requires the local feature", …) and none
/// said what to do about it. A refusal a user cannot act on is a dead end, and
/// this is the most common refusal a released binary produces.
///
/// **The wording is load-bearing, and has caught this change out twice.**
/// `InferenceError::code()` classifies `NotSupported` by substring-matching the
/// message: the word "provider" would silently make it
/// `inference.unsupported_provider`, and "download"
/// `inference.download_disabled`, instead of the
/// `inference.unsupported_operation` these paths have always returned. That is
/// why the text below names the prefixes directly rather than calling them
/// providers. `inference_refusals_keep_their_codes` pins it; #3216 is the fix.
pub(crate) fn local_feature_unavailable(operation: &str) -> InferenceError {
    InferenceError::NotSupported(format!(
        "{operation} needs local model execution: {LOCAL_UNAVAILABLE_REMEDY} \
         `strata inference status` shows which of those are ready."
    ))
}

/// One phrasing for "this binary was not built with model downloading".
///
/// Must contain "download" and must not contain "provider" — see
/// [`local_feature_unavailable`] on why the wording is load-bearing.
pub(crate) fn download_feature_unavailable() -> InferenceError {
    InferenceError::NotSupported(format!(
        "model download is not built into this binary: {LOCAL_UNAVAILABLE_REMEDY} \
         To use a local model anyway, fetch its GGUF file into the models \
         directory yourself — `strata inference status` shows the directory and \
         `strata inference models list` the expected repository and file name."
    ))
}

fn embedding_provider_feature_enabled_for_capability(provider: ProviderKind) -> bool {
    match provider {
        ProviderKind::Local => cfg!(feature = "local"),
        ProviderKind::OpenAI => cfg!(feature = "openai"),
        ProviderKind::Google => cfg!(feature = "google"),
        ProviderKind::Anthropic => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_allows_network() {
        let config = InferenceRuntimeConfig::default();
        assert!(config.network_enabled);
        assert!(config.models_dir.is_none());
    }

    /// The key source is the variable's name, never its value (D11).
    #[test]
    fn key_source_reports_the_variable_name_never_the_value() {
        const SECRET: &str = "sk-do-not-leak-this-value";

        // Set: the source is the NAME. If this ever returned the value, a key
        // would ride out on every `inference status`.
        let found = resolve_key_source(Some("OPENAI_API_KEY"), |_| true);
        assert_eq!(found.as_deref(), Some("OPENAI_API_KEY"));
        assert_ne!(found.as_deref(), Some(SECRET));

        // Unset: no source at all.
        assert_eq!(resolve_key_source(Some("OPENAI_API_KEY"), |_| false), None);

        // A provider that needs no key never reports a source, however the
        // lookup behaves.
        assert_eq!(resolve_key_source(None, |_| true), None);
        assert_eq!(resolve_key_source(None, |_| false), None);
    }

    /// The truth table for what a model inherently supports (#3124).
    ///
    /// Observable regardless of which provider features are compiled in, which
    /// is the point: folded into the `can_*` expressions this logic was
    /// unreachable in a build with the feature off.
    #[test]
    fn model_abilities_truth_table() {
        use ProviderKind::{Anthropic, Google, Local, OpenAI};

        // Local embedding model: embeds and tokenizes, does not generate.
        let embed = ModelAbilities::of(Local, Some(ModelTask::Embed));
        assert_eq!(
            embed,
            ModelAbilities {
                generate: false,
                tokenize: true,
                embed: true,
                rank: false
            }
        );

        // Local generation model: generates and tokenizes, does not embed.
        let generate = ModelAbilities::of(Local, Some(ModelTask::Generate));
        assert_eq!(
            generate,
            ModelAbilities {
                generate: true,
                tokenize: true,
                embed: false,
                rank: false
            }
        );

        // Local rank model: ranks, and generates (it is not an embed model).
        let rank = ModelAbilities::of(Local, Some(ModelTask::Rank));
        assert_eq!(
            rank,
            ModelAbilities {
                generate: true,
                tokenize: true,
                embed: false,
                rank: true
            }
        );

        // An uncatalogued local spec has no task: nothing local-specific is
        // claimed beyond tokenization.
        let unknown = ModelAbilities::of(Local, None);
        assert_eq!(
            unknown,
            ModelAbilities {
                generate: true,
                tokenize: true,
                embed: false,
                rank: false
            }
        );

        // Cloud providers never tokenize or rank here. OpenAI and Google embed;
        // Anthropic does not.
        for provider in [OpenAI, Google] {
            assert_eq!(
                ModelAbilities::of(provider, None),
                ModelAbilities {
                    generate: true,
                    tokenize: false,
                    embed: true,
                    rank: false
                },
                "{provider:?}"
            );
        }
        assert_eq!(
            ModelAbilities::of(Anthropic, None),
            ModelAbilities {
                generate: true,
                tokenize: false,
                embed: false,
                rank: false
            }
        );
    }

    #[test]
    fn capability_for_local_embed_model_reports_embedding() {
        let runtime = InferenceRuntime::default();
        let capability = runtime.capability("local:miniLM").expect("capability");
        assert_eq!(capability.provider, ProviderKind::Local);
        // #3124: `can_*` reports what THIS BINARY can do, so these follow the
        // feature rather than the model's declared task. The model's own shape
        // stays visible through `embedding_dim`.
        assert_eq!(capability.can_embed, cfg!(feature = "local"));
        assert_eq!(capability.can_tokenize, cfg!(feature = "local"));
        assert_eq!(capability.provider_feature_enabled, cfg!(feature = "local"));
        assert!(!capability.requires_api_key);
        assert_eq!(capability.embedding_dim, 384);
    }

    #[test]
    fn capability_for_openai_reports_network_and_api_key() {
        let runtime = InferenceRuntime::default();
        let capability = runtime
            .capability("openai:text-embedding-3-small")
            .expect("capability");
        assert_eq!(capability.provider, ProviderKind::OpenAI);
        assert!(capability.can_generate);
        assert!(capability.can_embed);
        assert!(capability.requires_network);
        assert!(capability.requires_api_key);
    }

    #[test]
    fn cache_status_is_empty_by_default() {
        let runtime = InferenceRuntime::default();
        assert_eq!(
            runtime.cache_status().expect("cache status"),
            ModelCacheStatus::default()
        );
    }
}
