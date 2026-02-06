//! SafeTensors binary format parser.
//!
//! Parses the SafeTensors file format: 8-byte header length (u64 LE),
//! JSON header describing tensor metadata, then raw tensor data.

use super::tensor::Tensor;
use std::collections::HashMap;

/// Parsed tensor metadata from the SafeTensors header.
#[derive(Debug, Clone)]
struct TensorInfo {
    dtype: String,
    shape: Vec<usize>,
    data_offsets: [usize; 2],
}

/// A parsed SafeTensors file.
pub struct SafeTensors {
    tensors: HashMap<String, TensorInfo>,
    data: Vec<u8>,
}

impl SafeTensors {
    /// Parse a SafeTensors file from raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 8 {
            return Err("SafeTensors file too short".into());
        }

        let header_len = u64::from_le_bytes(
            bytes[..8]
                .try_into()
                .map_err(|_| "Failed to read header length")?,
        ) as usize;

        if 8 + header_len > bytes.len() {
            return Err(format!(
                "Header length {} exceeds file size {}",
                header_len,
                bytes.len()
            ));
        }

        let header_json = std::str::from_utf8(&bytes[8..8 + header_len])
            .map_err(|e| format!("Invalid UTF-8 in header: {}", e))?;

        let header: HashMap<String, serde_json::Value> = serde_json::from_str(header_json)
            .map_err(|e| format!("Failed to parse header JSON: {}", e))?;

        let mut tensors = HashMap::new();

        for (name, meta) in &header {
            // Skip the __metadata__ key
            if name == "__metadata__" {
                continue;
            }

            let obj = meta
                .as_object()
                .ok_or_else(|| format!("Tensor '{}' metadata is not an object", name))?;

            let dtype = obj
                .get("dtype")
                .and_then(|v| v.as_str())
                .ok_or_else(|| format!("Tensor '{}' missing dtype", name))?
                .to_string();

            let shape: Vec<usize> = obj
                .get("shape")
                .and_then(|v| v.as_array())
                .ok_or_else(|| format!("Tensor '{}' missing shape", name))?
                .iter()
                .map(|v| v.as_u64().unwrap_or(0) as usize)
                .collect();

            let offsets = obj
                .get("data_offsets")
                .and_then(|v| v.as_array())
                .ok_or_else(|| format!("Tensor '{}' missing data_offsets", name))?;

            if offsets.len() != 2 {
                return Err(format!("Tensor '{}' has invalid data_offsets", name));
            }

            let start = offsets[0].as_u64().unwrap_or(0) as usize;
            let end = offsets[1].as_u64().unwrap_or(0) as usize;

            tensors.insert(
                name.clone(),
                TensorInfo {
                    dtype,
                    shape,
                    data_offsets: [start, end],
                },
            );
        }

        Ok(Self {
            tensors,
            data: bytes[8 + header_len..].to_vec(),
        })
    }

    /// Extract a named tensor as a 2D Tensor.
    pub fn tensor(&self, name: &str) -> Option<Tensor> {
        let info = self.tensors.get(name)?;

        if info.dtype != "F32" {
            return None;
        }

        let start = info.data_offsets[0];
        let end = info.data_offsets[1];

        if end > self.data.len() {
            return None;
        }

        let raw = &self.data[start..end];
        let floats: Vec<f32> = raw
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        let (rows, cols) = match info.shape.len() {
            1 => (1, info.shape[0]),
            2 => (info.shape[0], info.shape[1]),
            _ => return None,
        };

        Some(Tensor::from_slice(&floats, rows, cols))
    }

    /// Extract a named 1D tensor as a Vec<f32>.
    pub fn tensor_1d(&self, name: &str) -> Option<Vec<f32>> {
        let info = self.tensors.get(name)?;

        if info.dtype != "F32" {
            return None;
        }

        let start = info.data_offsets[0];
        let end = info.data_offsets[1];

        if end > self.data.len() {
            return None;
        }

        let raw = &self.data[start..end];
        let floats: Vec<f32> = raw
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();

        Some(floats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal synthetic SafeTensors file with one 2x3 F32 tensor.
    fn build_synthetic_safetensors() -> Vec<u8> {
        let header = r#"{"test_tensor":{"dtype":"F32","shape":[2,3],"data_offsets":[0,24]}}"#;
        let header_bytes = header.as_bytes();
        let header_len = header_bytes.len() as u64;

        let mut buf = Vec::new();
        buf.extend_from_slice(&header_len.to_le_bytes());
        buf.extend_from_slice(header_bytes);

        // 6 floats: 1.0, 2.0, 3.0, 4.0, 5.0, 6.0
        for &v in &[1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0] {
            buf.extend_from_slice(&v.to_le_bytes());
        }

        buf
    }

    #[test]
    fn test_parse_synthetic() {
        let data = build_synthetic_safetensors();
        let st = SafeTensors::from_bytes(&data).unwrap();
        let t = st.tensor("test_tensor").unwrap();
        assert_eq!(t.rows, 2);
        assert_eq!(t.cols, 3);
        assert_eq!(t.data, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_tensor_1d() {
        let header = r#"{"bias":{"dtype":"F32","shape":[3],"data_offsets":[0,12]}}"#;
        let header_bytes = header.as_bytes();
        let header_len = header_bytes.len() as u64;

        let mut buf = Vec::new();
        buf.extend_from_slice(&header_len.to_le_bytes());
        buf.extend_from_slice(header_bytes);
        for &v in &[0.1f32, 0.2, 0.3] {
            buf.extend_from_slice(&v.to_le_bytes());
        }

        let st = SafeTensors::from_bytes(&buf).unwrap();
        let bias = st.tensor_1d("bias").unwrap();
        assert_eq!(bias.len(), 3);
        assert!((bias[0] - 0.1).abs() < 1e-6);
    }

    #[test]
    fn test_missing_tensor() {
        let data = build_synthetic_safetensors();
        let st = SafeTensors::from_bytes(&data).unwrap();
        assert!(st.tensor("nonexistent").is_none());
    }

    #[test]
    fn test_too_short() {
        assert!(SafeTensors::from_bytes(&[0; 4]).is_err());
    }
}
