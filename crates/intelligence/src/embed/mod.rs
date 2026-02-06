//! Auto-embedding module: MiniLM-L6-v2 text embeddings.
//!
//! Provides a lazy-loading model lifecycle via [`EmbedModelState`] and
//! text extraction from Strata [`Value`] types.

pub mod extract;
pub mod model;
pub mod tokenizer;

use std::path::Path;
use std::sync::Arc;

use model::EmbedModel;

/// Lazy-loading model state stored as a Database extension.
///
/// On first use, loads the MiniLM-L6-v2 model from the model directory.
/// If model files are missing, stores the error and never retries.
pub struct EmbedModelState {
    model: once_cell::sync::OnceCell<Result<Arc<EmbedModel>, String>>,
}

impl Default for EmbedModelState {
    fn default() -> Self {
        Self {
            model: once_cell::sync::OnceCell::new(),
        }
    }
}

impl EmbedModelState {
    /// Get or load the embedding model.
    ///
    /// Loads from `model_dir/model.safetensors` and `model_dir/vocab.txt`.
    /// Caches the result (success or failure) so filesystem is probed at most once.
    pub fn get_or_load(&self, model_dir: &Path) -> Result<Arc<EmbedModel>, String> {
        self.model
            .get_or_init(|| {
                let safetensors_path = model_dir.join("model.safetensors");
                let vocab_path = model_dir.join("vocab.txt");

                let safetensors_bytes = std::fs::read(&safetensors_path).map_err(|e| {
                    format!(
                        "Failed to read model file '{}': {}",
                        safetensors_path.display(),
                        e
                    )
                })?;

                let vocab_text = std::fs::read_to_string(&vocab_path).map_err(|e| {
                    format!(
                        "Failed to read vocab file '{}': {}",
                        vocab_path.display(),
                        e
                    )
                })?;

                let model = EmbedModel::load(&safetensors_bytes, &vocab_text)?;
                Ok(Arc::new(model))
            })
            .clone()
    }
}
