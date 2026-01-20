//! Vector types for the VectorStore primitive
//!
//! These types define the structure of vector embeddings and search results.
//! Implementation logic (distance calculations, indexing, ANN) remains in primitives.

use crate::error::StrataError;
use crate::types::RunId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Distance metric for similarity calculation
///
/// All metrics are normalized to "higher = more similar".
/// This normalization is part of the interface contract.
///
/// Note: The actual distance calculation logic is in the primitives crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DistanceMetric {
    /// Cosine similarity: dot(a,b) / (||a|| * ||b||)
    /// Range: [-1, 1], higher = more similar
    /// Best for: normalized embeddings, semantic similarity
    #[default]
    Cosine,

    /// Euclidean similarity: 1 / (1 + l2_distance)
    /// Range: (0, 1], higher = more similar
    /// Best for: absolute position comparisons
    Euclidean,

    /// Dot product (raw value)
    /// Range: unbounded, higher = more similar
    /// Best for: pre-normalized embeddings, retrieval
    /// WARNING: Assumes vectors are normalized. Non-normalized vectors
    /// will produce unbounded scores.
    DotProduct,
}

impl DistanceMetric {
    /// Human-readable name for display
    pub fn name(&self) -> &'static str {
        match self {
            DistanceMetric::Cosine => "cosine",
            DistanceMetric::Euclidean => "euclidean",
            DistanceMetric::DotProduct => "dot_product",
        }
    }

    /// Parse from string (case-insensitive)
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cosine" => Some(DistanceMetric::Cosine),
            "euclidean" | "l2" => Some(DistanceMetric::Euclidean),
            "dot_product" | "dot" | "inner_product" => Some(DistanceMetric::DotProduct),
            _ => None,
        }
    }

    /// Serialization value for WAL/snapshot
    pub fn to_byte(&self) -> u8 {
        match self {
            DistanceMetric::Cosine => 0,
            DistanceMetric::Euclidean => 1,
            DistanceMetric::DotProduct => 2,
        }
    }

    /// Deserialization from WAL/snapshot
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(DistanceMetric::Cosine),
            1 => Some(DistanceMetric::Euclidean),
            2 => Some(DistanceMetric::DotProduct),
            _ => None,
        }
    }
}

/// Storage data type for embeddings
///
/// Only F32 supported initially. F16 and Int8 are reserved for future quantization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StorageDtype {
    /// 32-bit floating point (default)
    #[default]
    F32,
    // F16,     // Reserved for half precision (value = 1)
    // Int8,    // Reserved for scalar quantization (value = 2)
}

impl StorageDtype {
    /// Serialization value for WAL/snapshot
    pub fn to_byte(&self) -> u8 {
        match self {
            StorageDtype::F32 => 0,
        }
    }

    /// Deserialization from WAL/snapshot
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0 => Some(StorageDtype::F32),
            _ => None,
        }
    }
}

/// Collection configuration - immutable after creation
///
/// IMPORTANT: This struct must NOT contain backend-specific fields.
/// HNSW parameters (ef_construction, M, etc.) belong in backend config, not here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorConfig {
    /// Embedding dimension (e.g., 384, 768, 1536)
    /// Must be > 0. Immutable after collection creation.
    pub dimension: usize,

    /// Distance metric for similarity calculation
    /// Immutable after collection creation.
    pub metric: DistanceMetric,

    /// Storage data type
    /// Only F32 supported initially. Reserved for F16/Int8 in future.
    pub storage_dtype: StorageDtype,
}

impl VectorConfig {
    /// Create a new VectorConfig with validation
    ///
    /// Returns an error if dimension is 0.
    pub fn new(dimension: usize, metric: DistanceMetric) -> Result<Self, StrataError> {
        if dimension == 0 {
            return Err(StrataError::InvalidInput {
                message: format!("Invalid dimension: {} (must be > 0)", dimension),
            });
        }
        Ok(VectorConfig {
            dimension,
            metric,
            storage_dtype: StorageDtype::F32,
        })
    }

    /// Config for OpenAI text-embedding-ada-002 (1536 dims)
    pub fn for_openai_ada() -> Self {
        VectorConfig {
            dimension: 1536,
            metric: DistanceMetric::Cosine,
            storage_dtype: StorageDtype::F32,
        }
    }

    /// Config for OpenAI text-embedding-3-large (3072 dims)
    pub fn for_openai_large() -> Self {
        VectorConfig {
            dimension: 3072,
            metric: DistanceMetric::Cosine,
            storage_dtype: StorageDtype::F32,
        }
    }

    /// Config for MiniLM (384 dims)
    pub fn for_minilm() -> Self {
        VectorConfig {
            dimension: 384,
            metric: DistanceMetric::Cosine,
            storage_dtype: StorageDtype::F32,
        }
    }

    /// Config for sentence-transformers/all-mpnet-base-v2 (768 dims)
    pub fn for_mpnet() -> Self {
        VectorConfig {
            dimension: 768,
            metric: DistanceMetric::Cosine,
            storage_dtype: StorageDtype::F32,
        }
    }
}

/// Internal vector identifier (stable within collection)
///
/// IMPORTANT: VectorIds are never reused.
/// Storage slots may be reused, but the ID value is monotonically increasing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VectorId(pub u64);

impl VectorId {
    /// Create a new VectorId
    pub fn new(id: u64) -> Self {
        VectorId(id)
    }

    /// Get the underlying u64 value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for VectorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VectorId({})", self.0)
    }
}

/// Vector entry stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    /// User-provided key (unique within collection)
    pub key: String,

    /// Embedding vector
    pub embedding: Vec<f32>,

    /// Optional JSON metadata
    pub metadata: Option<serde_json::Value>,

    /// Internal ID (for index backend)
    pub vector_id: VectorId,

    /// Version for optimistic concurrency
    pub version: u64,
}

impl VectorEntry {
    /// Create a new VectorEntry
    pub fn new(
        key: String,
        embedding: Vec<f32>,
        metadata: Option<serde_json::Value>,
        vector_id: VectorId,
    ) -> Self {
        VectorEntry {
            key,
            embedding,
            metadata,
            vector_id,
            version: 1,
        }
    }

    /// Get the embedding dimension
    pub fn dimension(&self) -> usize {
        self.embedding.len()
    }

    /// Get the version number
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Get the vector ID
    pub fn vector_id(&self) -> VectorId {
        self.vector_id
    }
}

/// Search result entry
///
/// Returned by search operations. Score is always "higher = more similar"
/// regardless of the underlying distance metric.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorMatch {
    /// User-provided key
    pub key: String,

    /// Similarity score (higher = more similar)
    /// This is normalized per the interface contract.
    pub score: f32,

    /// Optional metadata (if requested and present)
    pub metadata: Option<serde_json::Value>,
}

impl VectorMatch {
    /// Create a new VectorMatch
    pub fn new(key: String, score: f32, metadata: Option<serde_json::Value>) -> Self {
        VectorMatch {
            key,
            score,
            metadata,
        }
    }
}

/// Collection metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    /// Collection name
    pub name: String,

    /// Immutable configuration
    pub config: VectorConfig,

    /// Current vector count
    pub count: usize,

    /// Creation timestamp (microseconds since epoch)
    pub created_at: u64,
}

/// Unique identifier for a collection within a run
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CollectionId {
    /// Run ID this collection belongs to
    pub run_id: RunId,
    /// Collection name
    pub name: String,
}

impl CollectionId {
    /// Create a new CollectionId
    pub fn new(run_id: RunId, name: impl Into<String>) -> Self {
        CollectionId {
            run_id,
            name: name.into(),
        }
    }
}

impl Ord for CollectionId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.run_id
            .as_bytes()
            .cmp(other.run_id.as_bytes())
            .then(self.name.cmp(&other.name))
    }
}

impl PartialOrd for CollectionId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// JSON scalar value for filtering
///
/// Only scalar values can be used in equality filters.
/// Complex types (arrays, objects) are not supported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JsonScalar {
    /// Null value
    Null,
    /// Boolean value
    Bool(bool),
    /// Numeric value (stored as f64)
    Number(f64),
    /// String value
    String(String),
}

impl JsonScalar {
    /// Check if this scalar matches a JSON value
    pub fn matches_json(&self, value: &serde_json::Value) -> bool {
        match (self, value) {
            (JsonScalar::Null, serde_json::Value::Null) => true,
            (JsonScalar::Bool(a), serde_json::Value::Bool(b)) => a == b,
            (JsonScalar::Number(a), serde_json::Value::Number(b)) => {
                b.as_f64().is_some_and(|n| (a - n).abs() < f64::EPSILON)
            }
            (JsonScalar::String(a), serde_json::Value::String(b)) => a == b,
            _ => false,
        }
    }
}

impl From<bool> for JsonScalar {
    fn from(v: bool) -> Self {
        JsonScalar::Bool(v)
    }
}

impl From<i32> for JsonScalar {
    fn from(v: i32) -> Self {
        JsonScalar::Number(v as f64)
    }
}

impl From<i64> for JsonScalar {
    fn from(v: i64) -> Self {
        JsonScalar::Number(v as f64)
    }
}

impl From<f32> for JsonScalar {
    fn from(v: f32) -> Self {
        JsonScalar::Number(v as f64)
    }
}

impl From<f64> for JsonScalar {
    fn from(v: f64) -> Self {
        JsonScalar::Number(v)
    }
}

impl From<String> for JsonScalar {
    fn from(v: String) -> Self {
        JsonScalar::String(v)
    }
}

impl From<&str> for JsonScalar {
    fn from(v: &str) -> Self {
        JsonScalar::String(v.to_string())
    }
}

/// Metadata filter for search (equality only)
///
/// Supports only top-level field equality filtering.
/// Complex filters (ranges, nested paths, arrays) are deferred to future versions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataFilter {
    /// Top-level field equality (scalar values only)
    /// All conditions must match (AND semantics)
    pub equals: HashMap<String, JsonScalar>,
}

impl MetadataFilter {
    /// Create an empty filter (matches all)
    pub fn new() -> Self {
        MetadataFilter {
            equals: HashMap::new(),
        }
    }

    /// Add an equality condition
    pub fn eq(mut self, field: impl Into<String>, value: impl Into<JsonScalar>) -> Self {
        self.equals.insert(field.into(), value.into());
        self
    }

    /// Check if metadata matches this filter
    ///
    /// Returns true if all conditions match.
    /// Returns false if metadata is None and filter is non-empty.
    pub fn matches(&self, metadata: &Option<serde_json::Value>) -> bool {
        if self.equals.is_empty() {
            return true;
        }

        let Some(meta) = metadata else {
            return false;
        };

        let Some(obj) = meta.as_object() else {
            return false;
        };

        for (key, expected) in &self.equals {
            let Some(actual) = obj.get(key) else {
                return false;
            };
            if !expected.matches_json(actual) {
                return false;
            }
        }

        true
    }

    /// Check if filter is empty (matches all)
    pub fn is_empty(&self) -> bool {
        self.equals.is_empty()
    }

    /// Get the number of conditions in the filter
    pub fn len(&self) -> usize {
        self.equals.len()
    }
}
