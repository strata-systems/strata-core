//! MiniLM-L6-v2 encoder architecture and forward pass.

use crate::runtime::safetensors::SafeTensors;
use crate::runtime::tensor::Tensor;

use super::tokenizer::{TokenizedInput, WordPieceTokenizer};

const HIDDEN_SIZE: usize = 384;
const NUM_HEADS: usize = 12;
const HEAD_DIM: usize = HIDDEN_SIZE / NUM_HEADS; // 32
const NUM_LAYERS: usize = 6;
const VOCAB_SIZE: usize = 30522;
const LAYER_NORM_EPS: f32 = 1e-12;

/// A single transformer encoder layer.
struct TransformerLayer {
    q_weight: Tensor,
    q_bias: Vec<f32>,
    k_weight: Tensor,
    k_bias: Vec<f32>,
    v_weight: Tensor,
    v_bias: Vec<f32>,
    attn_output_weight: Tensor,
    attn_output_bias: Vec<f32>,
    attn_ln_weight: Vec<f32>,
    attn_ln_bias: Vec<f32>,
    intermediate_weight: Tensor,
    intermediate_bias: Vec<f32>,
    output_weight: Tensor,
    output_bias: Vec<f32>,
    output_ln_weight: Vec<f32>,
    output_ln_bias: Vec<f32>,
}

/// The MiniLM-L6-v2 embedding model.
pub struct EmbedModel {
    tokenizer: WordPieceTokenizer,
    word_embeddings: Tensor,
    position_embeddings: Tensor,
    token_type_embeddings: Tensor,
    embed_ln_weight: Vec<f32>,
    embed_ln_bias: Vec<f32>,
    layers: Vec<TransformerLayer>,
}

impl EmbedModel {
    /// Load model weights from SafeTensors bytes and vocabulary text.
    pub fn load(safetensors_bytes: &[u8], vocab_text: &str) -> Result<Self, String> {
        let st = SafeTensors::from_bytes(safetensors_bytes)?;
        let tokenizer = WordPieceTokenizer::from_vocab(vocab_text);

        let word_embeddings = st
            .tensor("bert.embeddings.word_embeddings.weight")
            .ok_or("Missing word_embeddings")?;

        if word_embeddings.rows != VOCAB_SIZE || word_embeddings.cols != HIDDEN_SIZE {
            return Err(format!(
                "word_embeddings shape mismatch: expected {}x{}, got {}x{}",
                VOCAB_SIZE, HIDDEN_SIZE, word_embeddings.rows, word_embeddings.cols
            ));
        }

        let position_embeddings = st
            .tensor("bert.embeddings.position_embeddings.weight")
            .ok_or("Missing position_embeddings")?;

        let token_type_embeddings = st
            .tensor("bert.embeddings.token_type_embeddings.weight")
            .ok_or("Missing token_type_embeddings")?;

        let embed_ln_weight = st
            .tensor_1d("bert.embeddings.LayerNorm.weight")
            .ok_or("Missing embeddings LayerNorm weight")?;

        let embed_ln_bias = st
            .tensor_1d("bert.embeddings.LayerNorm.bias")
            .ok_or("Missing embeddings LayerNorm bias")?;

        let mut layers = Vec::with_capacity(NUM_LAYERS);
        for i in 0..NUM_LAYERS {
            let prefix = format!("bert.encoder.layer.{}", i);
            let layer = TransformerLayer {
                q_weight: st
                    .tensor(&format!("{}.attention.self.query.weight", prefix))
                    .ok_or_else(|| format!("Missing {}.attention.self.query.weight", prefix))?,
                q_bias: st
                    .tensor_1d(&format!("{}.attention.self.query.bias", prefix))
                    .ok_or_else(|| format!("Missing {}.attention.self.query.bias", prefix))?,
                k_weight: st
                    .tensor(&format!("{}.attention.self.key.weight", prefix))
                    .ok_or_else(|| format!("Missing {}.attention.self.key.weight", prefix))?,
                k_bias: st
                    .tensor_1d(&format!("{}.attention.self.key.bias", prefix))
                    .ok_or_else(|| format!("Missing {}.attention.self.key.bias", prefix))?,
                v_weight: st
                    .tensor(&format!("{}.attention.self.value.weight", prefix))
                    .ok_or_else(|| format!("Missing {}.attention.self.value.weight", prefix))?,
                v_bias: st
                    .tensor_1d(&format!("{}.attention.self.value.bias", prefix))
                    .ok_or_else(|| format!("Missing {}.attention.self.value.bias", prefix))?,
                attn_output_weight: st
                    .tensor(&format!("{}.attention.output.dense.weight", prefix))
                    .ok_or_else(|| {
                        format!("Missing {}.attention.output.dense.weight", prefix)
                    })?,
                attn_output_bias: st
                    .tensor_1d(&format!("{}.attention.output.dense.bias", prefix))
                    .ok_or_else(|| {
                        format!("Missing {}.attention.output.dense.bias", prefix)
                    })?,
                attn_ln_weight: st
                    .tensor_1d(&format!("{}.attention.output.LayerNorm.weight", prefix))
                    .ok_or_else(|| {
                        format!("Missing {}.attention.output.LayerNorm.weight", prefix)
                    })?,
                attn_ln_bias: st
                    .tensor_1d(&format!("{}.attention.output.LayerNorm.bias", prefix))
                    .ok_or_else(|| {
                        format!("Missing {}.attention.output.LayerNorm.bias", prefix)
                    })?,
                intermediate_weight: st
                    .tensor(&format!("{}.intermediate.dense.weight", prefix))
                    .ok_or_else(|| {
                        format!("Missing {}.intermediate.dense.weight", prefix)
                    })?,
                intermediate_bias: st
                    .tensor_1d(&format!("{}.intermediate.dense.bias", prefix))
                    .ok_or_else(|| {
                        format!("Missing {}.intermediate.dense.bias", prefix)
                    })?,
                output_weight: st
                    .tensor(&format!("{}.output.dense.weight", prefix))
                    .ok_or_else(|| format!("Missing {}.output.dense.weight", prefix))?,
                output_bias: st
                    .tensor_1d(&format!("{}.output.dense.bias", prefix))
                    .ok_or_else(|| format!("Missing {}.output.dense.bias", prefix))?,
                output_ln_weight: st
                    .tensor_1d(&format!("{}.output.LayerNorm.weight", prefix))
                    .ok_or_else(|| format!("Missing {}.output.LayerNorm.weight", prefix))?,
                output_ln_bias: st
                    .tensor_1d(&format!("{}.output.LayerNorm.bias", prefix))
                    .ok_or_else(|| format!("Missing {}.output.LayerNorm.bias", prefix))?,
            };
            layers.push(layer);
        }

        Ok(Self {
            tokenizer,
            word_embeddings,
            position_embeddings,
            token_type_embeddings,
            embed_ln_weight,
            embed_ln_bias,
            layers,
        })
    }

    /// Embed a text string into a 384-dimensional vector.
    pub fn embed(&self, text: &str) -> Vec<f32> {
        let input = self.tokenizer.tokenize(text);
        let seq_len = input.input_ids.len();

        // 1. Gather embeddings
        let hidden = self.gather_embeddings(&input, seq_len);

        // 2. Layer norm
        let mut hidden = hidden.layer_norm(&self.embed_ln_weight, &self.embed_ln_bias, LAYER_NORM_EPS);

        // 3. Transformer layers
        for layer in &self.layers {
            hidden = self.transformer_layer(layer, &hidden, &input.attention_mask);
        }

        // 4. Mean pooling (exclude padding)
        let pooled = self.mean_pool(&hidden, &input.attention_mask);

        // 5. L2 normalize
        l2_normalize(&pooled)
    }

    fn gather_embeddings(&self, input: &TokenizedInput, seq_len: usize) -> Tensor {
        let mut data = vec![0.0f32; seq_len * HIDDEN_SIZE];

        for (pos, (&token_id, &type_id)) in input
            .input_ids
            .iter()
            .zip(input.token_type_ids.iter())
            .enumerate()
        {
            let word_row = self.word_embeddings.row(token_id as usize);
            let pos_row = self.position_embeddings.row(pos);
            let type_row = self.token_type_embeddings.row(type_id as usize);

            let offset = pos * HIDDEN_SIZE;
            for i in 0..HIDDEN_SIZE {
                data[offset + i] = word_row[i] + pos_row[i] + type_row[i];
            }
        }

        Tensor::from_slice(&data, seq_len, HIDDEN_SIZE)
    }

    fn transformer_layer(
        &self,
        layer: &TransformerLayer,
        hidden: &Tensor,
        attention_mask: &[u32],
    ) -> Tensor {
        let seq_len = hidden.rows;

        // Self-attention
        // Q, K, V projections: hidden × Wᵀ + b
        // BERT stores weights transposed: shape is (out, in), so we use matmul_transpose
        let mut q = hidden.matmul_transpose(&layer.q_weight);
        q.add_bias(&layer.q_bias);
        let mut k = hidden.matmul_transpose(&layer.k_weight);
        k.add_bias(&layer.k_bias);
        let mut v = hidden.matmul_transpose(&layer.v_weight);
        v.add_bias(&layer.v_bias);

        // Multi-head attention
        let scale = 1.0 / (HEAD_DIM as f32).sqrt();
        let mut attn_output_data = vec![0.0f32; seq_len * HIDDEN_SIZE];

        for head in 0..NUM_HEADS {
            let head_offset = head * HEAD_DIM;

            // Extract Q, K, V slices for this head
            let mut q_head = Tensor::zeros(seq_len, HEAD_DIM);
            let mut k_head = Tensor::zeros(seq_len, HEAD_DIM);
            let mut v_head = Tensor::zeros(seq_len, HEAD_DIM);

            for s in 0..seq_len {
                for d in 0..HEAD_DIM {
                    q_head.data[s * HEAD_DIM + d] = q.data[s * HIDDEN_SIZE + head_offset + d];
                    k_head.data[s * HEAD_DIM + d] = k.data[s * HIDDEN_SIZE + head_offset + d];
                    v_head.data[s * HEAD_DIM + d] = v.data[s * HIDDEN_SIZE + head_offset + d];
                }
            }

            // Attention scores: Q × Kᵀ / √d_k
            let mut scores = q_head.matmul_transpose(&k_head);
            scores.scale(scale);

            // Apply attention mask (set padding positions to -10000)
            for i in 0..seq_len {
                for j in 0..seq_len {
                    if attention_mask[j] == 0 {
                        scores.data[i * seq_len + j] = -10000.0;
                    }
                }
            }

            scores.softmax_rows();

            // Weighted sum: scores × V
            let context = scores.matmul(&v_head);

            // Copy back to full hidden dim
            for s in 0..seq_len {
                for d in 0..HEAD_DIM {
                    attn_output_data[s * HIDDEN_SIZE + head_offset + d] =
                        context.data[s * HEAD_DIM + d];
                }
            }
        }

        let attn_output = Tensor::from_slice(&attn_output_data, seq_len, HIDDEN_SIZE);

        // Output projection
        let mut projected = attn_output.matmul_transpose(&layer.attn_output_weight);
        projected.add_bias(&layer.attn_output_bias);

        // Residual + LayerNorm
        let post_attn = projected.add_tensor(hidden);
        let normed_attn =
            post_attn.layer_norm(&layer.attn_ln_weight, &layer.attn_ln_bias, LAYER_NORM_EPS);

        // FFN: intermediate
        let mut intermediate = normed_attn.matmul_transpose(&layer.intermediate_weight);
        intermediate.add_bias(&layer.intermediate_bias);
        let intermediate = intermediate.gelu();

        // FFN: output
        let mut output = intermediate.matmul_transpose(&layer.output_weight);
        output.add_bias(&layer.output_bias);

        // Residual + LayerNorm
        let post_ffn = output.add_tensor(&normed_attn);
        post_ffn.layer_norm(&layer.output_ln_weight, &layer.output_ln_bias, LAYER_NORM_EPS)
    }

    fn mean_pool(&self, hidden: &Tensor, attention_mask: &[u32]) -> Vec<f32> {
        let seq_len = hidden.rows;
        let mut sum = vec![0.0f32; HIDDEN_SIZE];
        let mut count = 0.0f32;

        for s in 0..seq_len {
            if attention_mask[s] == 1 {
                let row = hidden.row(s);
                for i in 0..HIDDEN_SIZE {
                    sum[i] += row[i];
                }
                count += 1.0;
            }
        }

        if count > 0.0 {
            for v in sum.iter_mut() {
                *v /= count;
            }
        }

        sum
    }
}

fn l2_normalize(v: &[f32]) -> Vec<f32> {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        v.iter().map(|x| x / norm).collect()
    } else {
        v.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l2_normalize() {
        let v = vec![3.0, 4.0];
        let n = l2_normalize(&v);
        assert!((n[0] - 0.6).abs() < 1e-6);
        assert!((n[1] - 0.8).abs() < 1e-6);
        let norm: f32 = n.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_l2_normalize_zero() {
        let v = vec![0.0, 0.0];
        let n = l2_normalize(&v);
        assert_eq!(n, vec![0.0, 0.0]);
    }
}
