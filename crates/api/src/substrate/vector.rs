//! VectorStore Substrate Operations
//!
//! The VectorStore provides dense vector storage and similarity search for embeddings.
//! It supports multiple collections with different dimensions and distance metrics.
//!
//! ## Collection Model
//!
//! - Vectors are organized into named collections
//! - Each collection has a fixed dimension (set on first insert)
//! - Vectors within a collection must all have the same dimension
//! - Metadata can be attached to vectors and used for filtering
//!
//! ## Distance Metrics
//!
//! - `Cosine`: Cosine similarity (normalized, range [0, 1] for similarity)
//! - `Euclidean`: L2 distance (smaller = more similar)
//! - `DotProduct`: Inner product (larger = more similar)
//!
//! ## Versioning
//!
//! Vectors use transaction-based versioning (`Version::Txn`).

use super::types::ApiRunId;
use strata_core::{StrataResult, Value, Version, Versioned};
use serde::{Deserialize, Serialize};

/// Vector data with metadata
///
/// Type alias for a vector and its associated metadata.
pub type VectorData = (Vec<f32>, Value);

/// Distance metric for vector similarity search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistanceMetric {
    /// Cosine similarity (1 - cosine distance)
    #[default]
    Cosine,
    /// Euclidean (L2) distance
    Euclidean,
    /// Dot product (inner product)
    DotProduct,
}

/// Vector search result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorMatch {
    /// Vector key
    pub key: String,
    /// Similarity/distance score
    pub score: f32,
    /// Vector data
    pub vector: Vec<f32>,
    /// Attached metadata
    pub metadata: Value,
    /// Version of the vector
    pub version: Version,
}

/// Search filter for metadata
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchFilter {
    /// Exact match: `metadata[field] == value`
    Equals {
        /// Metadata field name
        field: String,
        /// Value to match
        value: Value,
    },
    /// Prefix match: `metadata[field].starts_with(prefix)`
    Prefix {
        /// Metadata field name
        field: String,
        /// Prefix to match
        prefix: String,
    },
    /// Range match: `min <= metadata[field] <= max`
    Range {
        /// Metadata field name
        field: String,
        /// Minimum value (inclusive)
        min: Value,
        /// Maximum value (inclusive)
        max: Value,
    },
    /// AND of multiple filters
    And(Vec<SearchFilter>),
    /// OR of multiple filters
    Or(Vec<SearchFilter>),
    /// NOT of a filter
    Not(Box<SearchFilter>),
}

/// VectorStore substrate operations
///
/// This trait defines the canonical vector store operations.
/// All operations require explicit run_id and return versioned results.
///
/// ## Contract
///
/// - Collections have fixed dimension (set on first insert)
/// - All vectors in a collection must match the dimension
/// - Metadata is `Value::Object` or `Value::Null`
///
/// ## Error Handling
///
/// | Condition | Error |
/// |-----------|-------|
/// | Invalid collection name | `InvalidKey` |
/// | Invalid vector key | `InvalidKey` |
/// | Dimension mismatch | `ConstraintViolation` |
/// | Dimension too large | `ConstraintViolation` |
/// | Run not found | `NotFound` |
/// | Run is closed | `ConstraintViolation` |
pub trait VectorStore {
    /// Insert or update a vector
    ///
    /// Stores a vector with optional metadata.
    /// Returns the version of the stored vector.
    ///
    /// ## Semantics
    ///
    /// - Creates collection if it doesn't exist (dimension set from first vector)
    /// - Replaces vector if key exists (creates new version)
    /// - Validates dimension matches collection
    ///
    /// ## Parameters
    ///
    /// - `collection`: Collection name
    /// - `key`: Vector key (unique within collection)
    /// - `vector`: The vector data (f32 array)
    /// - `metadata`: Optional metadata (must be Object or Null)
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Collection or key name is invalid
    /// - `ConstraintViolation`: Dimension mismatch, too large, or run is closed
    /// - `NotFound`: Run does not exist
    fn vector_upsert(
        &self,
        run: &ApiRunId,
        collection: &str,
        key: &str,
        vector: &[f32],
        metadata: Option<Value>,
    ) -> StrataResult<Version>;

    /// Get a vector by key
    ///
    /// Returns the vector data and metadata.
    ///
    /// ## Return Value
    ///
    /// - `Some((vector, metadata, version))`: Vector exists
    /// - `None`: Vector does not exist
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Collection or key name is invalid
    /// - `NotFound`: Run does not exist
    fn vector_get(
        &self,
        run: &ApiRunId,
        collection: &str,
        key: &str,
    ) -> StrataResult<Option<Versioned<VectorData>>>;

    /// Delete a vector
    ///
    /// Removes the vector from the collection.
    /// Returns `true` if the vector existed.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Collection or key name is invalid
    /// - `NotFound`: Run does not exist
    /// - `ConstraintViolation`: Run is closed
    fn vector_delete(&self, run: &ApiRunId, collection: &str, key: &str) -> StrataResult<bool>;

    /// Search for similar vectors
    ///
    /// Returns the K most similar vectors to the query.
    ///
    /// ## Parameters
    ///
    /// - `collection`: Collection to search
    /// - `query`: Query vector (must match collection dimension)
    /// - `k`: Maximum results to return
    /// - `filter`: Optional metadata filter
    /// - `metric`: Distance metric (defaults to collection default)
    ///
    /// ## Return Value
    ///
    /// Vector of matches sorted by similarity (most similar first).
    /// Empty if collection doesn't exist or no matches.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Collection name is invalid
    /// - `ConstraintViolation`: Query dimension mismatch
    /// - `NotFound`: Run does not exist
    fn vector_search(
        &self,
        run: &ApiRunId,
        collection: &str,
        query: &[f32],
        k: u64,
        filter: Option<SearchFilter>,
        metric: Option<DistanceMetric>,
    ) -> StrataResult<Vec<VectorMatch>>;

    /// Get collection info
    ///
    /// Returns information about a collection.
    ///
    /// ## Return Value
    ///
    /// - `Some((dimension, count, metric))`: Collection exists
    /// - `None`: Collection does not exist
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Collection name is invalid
    /// - `NotFound`: Run does not exist
    fn vector_collection_info(
        &self,
        run: &ApiRunId,
        collection: &str,
    ) -> StrataResult<Option<(usize, u64, DistanceMetric)>>;

    /// Create a collection with explicit configuration
    ///
    /// Pre-creates a collection with specific dimension and metric.
    /// Returns the version.
    ///
    /// ## Semantics
    ///
    /// - If collection exists, validates dimension matches
    /// - If collection doesn't exist, creates with config
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Collection name is invalid
    /// - `ConstraintViolation`: Dimension mismatch with existing, or run is closed
    /// - `NotFound`: Run does not exist
    fn vector_create_collection(
        &self,
        run: &ApiRunId,
        collection: &str,
        dimension: usize,
        metric: DistanceMetric,
    ) -> StrataResult<Version>;

    /// Delete a collection
    ///
    /// Removes the entire collection including all vectors.
    /// Returns `true` if the collection existed.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Collection name is invalid
    /// - `NotFound`: Run does not exist
    /// - `ConstraintViolation`: Run is closed
    fn vector_drop_collection(&self, run: &ApiRunId, collection: &str) -> StrataResult<bool>;

    /// List all collections in a run
    ///
    /// Returns information about all collections.
    ///
    /// ## Errors
    ///
    /// - `NotFound`: Run does not exist
    fn vector_list_collections(&self, run: &ApiRunId) -> StrataResult<Vec<VectorCollectionInfo>>;

    /// Check if a collection exists
    ///
    /// Returns `true` if the collection exists.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Collection name is invalid
    /// - `NotFound`: Run does not exist
    fn vector_collection_exists(&self, run: &ApiRunId, collection: &str) -> StrataResult<bool>;
}

/// Information about a vector collection
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorCollectionInfo {
    /// Collection name
    pub name: String,
    /// Vector dimension
    pub dimension: usize,
    /// Number of vectors in the collection
    pub count: u64,
}

// =============================================================================
// Implementation
// =============================================================================
//
// Note: The VectorStore primitive uses serde_json::Value for metadata,
// while the Substrate API uses strata_core::Value. This is a semantic
// boundary that needs proper bridging.

use strata_core::StrataError;
use super::impl_::{SubstrateImpl, convert_vector_error};

/// Convert our SearchFilter to the primitive's MetadataFilter
///
/// Note: The primitive only supports equality filters with AND semantics.
/// Complex filters (Prefix, Range, Or, Not) return an error.
fn convert_search_filter(filter: &SearchFilter) -> StrataResult<strata_core::primitives::MetadataFilter> {
    match filter {
        SearchFilter::Equals { field, value } => {
            let mut mf = strata_core::primitives::MetadataFilter::new();
            let scalar = value_to_json_scalar(value)?;
            mf = mf.eq(field.clone(), scalar);
            Ok(mf)
        }
        SearchFilter::And(filters) => {
            let mut mf = strata_core::primitives::MetadataFilter::new();
            for f in filters {
                match f {
                    SearchFilter::Equals { field, value } => {
                        let scalar = value_to_json_scalar(value)?;
                        mf = mf.eq(field.clone(), scalar);
                    }
                    _ => {
                        return Err(StrataError::invalid_input(
                            "Vector search filter: only Equals filters supported inside And"
                        ));
                    }
                }
            }
            Ok(mf)
        }
        SearchFilter::Prefix { .. } => {
            Err(StrataError::invalid_input(
                "Vector search filter: Prefix filter not supported by backend"
            ))
        }
        SearchFilter::Range { .. } => {
            Err(StrataError::invalid_input(
                "Vector search filter: Range filter not supported by backend"
            ))
        }
        SearchFilter::Or(_) => {
            Err(StrataError::invalid_input(
                "Vector search filter: Or filter not supported by backend"
            ))
        }
        SearchFilter::Not(_) => {
            Err(StrataError::invalid_input(
                "Vector search filter: Not filter not supported by backend"
            ))
        }
    }
}

/// Convert Value to JsonScalar for metadata filtering
fn value_to_json_scalar(value: &Value) -> StrataResult<strata_core::primitives::JsonScalar> {
    match value {
        Value::Null => Ok(strata_core::primitives::JsonScalar::Null),
        Value::Bool(b) => Ok(strata_core::primitives::JsonScalar::Bool(*b)),
        Value::Int(i) => Ok(strata_core::primitives::JsonScalar::Number(*i as f64)),
        Value::Float(f) => Ok(strata_core::primitives::JsonScalar::Number(*f)),
        Value::String(s) => Ok(strata_core::primitives::JsonScalar::String(s.clone())),
        Value::Bytes(_) | Value::Array(_) | Value::Object(_) => {
            Err(StrataError::invalid_input(
                "Vector search filter: only scalar values (null, bool, int, float, string) supported"
            ))
        }
    }
}

impl VectorStore for SubstrateImpl {
    fn vector_upsert(
        &self,
        run: &ApiRunId,
        collection: &str,
        key: &str,
        vector: &[f32],
        metadata: Option<Value>,
    ) -> StrataResult<Version> {
        // Validate vector is not empty
        if vector.is_empty() {
            return Err(StrataError::invalid_input("Vector must not be empty"));
        }

        let run_id = run.to_run_id();

        // Auto-create collection if it doesn't exist (per API contract)
        let exists = self.vector().collection_exists(run_id, collection)
            .map_err(convert_vector_error)?;
        if !exists {
            // Create collection with dimension inferred from vector
            let config = strata_core::VectorConfig::new(
                vector.len(),
                strata_core::DistanceMetric::Cosine,
            )?;
            self.vector().create_collection(run_id, collection, config)
                .map_err(convert_vector_error)?;
        }

        // Convert strata_core::Value metadata to serde_json::Value
        let json_metadata = metadata.map(|v| {
            serde_json::to_value(&v).unwrap_or(serde_json::Value::Null)
        });
        let version = self.vector().insert(run_id, collection, key, vector, json_metadata)
            .map_err(convert_vector_error)?;
        Ok(version)
    }

    fn vector_get(
        &self,
        run: &ApiRunId,
        collection: &str,
        key: &str,
    ) -> StrataResult<Option<Versioned<VectorData>>> {
        let run_id = run.to_run_id();

        // Check if collection exists first - return None if not
        let exists = self.vector().collection_exists(run_id, collection)
            .map_err(convert_vector_error)?;
        if !exists {
            return Ok(None);
        }

        let entry = self.vector().get(run_id, collection, key)
            .map_err(convert_vector_error)?;
        Ok(entry.map(|e| {
            // Convert serde_json::Value back to strata_core::Value
            // VectorData is (Vec<f32>, Value) - metadata is NOT optional in API
            let api_metadata: Value = e.value.metadata.clone()
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or(Value::Null);
            Versioned {
                value: (e.value.embedding.clone(), api_metadata),
                version: Version::Txn(e.value.version),
                timestamp: e.timestamp,
            }
        }))
    }

    fn vector_delete(&self, run: &ApiRunId, collection: &str, key: &str) -> StrataResult<bool> {
        let run_id = run.to_run_id();
        self.vector().delete(run_id, collection, key)
            .map_err(convert_vector_error)
    }

    fn vector_search(
        &self,
        run: &ApiRunId,
        collection: &str,
        query: &[f32],
        k: u64,
        filter: Option<SearchFilter>,
        _metric: Option<DistanceMetric>,
    ) -> StrataResult<Vec<VectorMatch>> {
        let run_id = run.to_run_id();

        // Convert filter if provided
        let metadata_filter = match filter {
            Some(ref f) => Some(convert_search_filter(f)?),
            None => None,
        };

        let results = self.vector().search(run_id, collection, query, k as usize, metadata_filter)
            .map_err(convert_vector_error)?;

        // Fetch vector data for each result
        let mut matches = Vec::with_capacity(results.len());
        for r in results {
            // Convert serde_json::Value metadata to strata_core::Value
            let api_metadata: Value = r.metadata
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or(Value::Null);

            // Fetch the actual vector data
            let vector_data = match self.vector().get(run_id, collection, &r.key) {
                Ok(Some(entry)) => entry.value.embedding.clone(),
                _ => vec![], // Fall back to empty if fetch fails
            };

            matches.push(VectorMatch {
                key: r.key,
                score: r.score,
                vector: vector_data,
                metadata: api_metadata,
                version: Version::Txn(0),
            });
        }

        Ok(matches)
    }

    fn vector_collection_info(
        &self,
        run: &ApiRunId,
        collection: &str,
    ) -> StrataResult<Option<(usize, u64, DistanceMetric)>> {
        let run_id = run.to_run_id();
        let info = self.vector().get_collection(run_id, collection)
            .map_err(convert_vector_error)?;
        Ok(info.map(|i| (i.value.config.dimension, i.value.count as u64, DistanceMetric::Cosine)))
    }

    fn vector_create_collection(
        &self,
        run: &ApiRunId,
        collection: &str,
        dimension: usize,
        _metric: DistanceMetric,
    ) -> StrataResult<Version> {
        let run_id = run.to_run_id();
        let config = strata_core::VectorConfig::new(
            dimension,
            strata_core::DistanceMetric::Cosine,
        )?;
        let versioned = self.vector().create_collection(run_id, collection, config)
            .map_err(convert_vector_error)?;
        Ok(versioned.version)
    }

    fn vector_drop_collection(&self, run: &ApiRunId, collection: &str) -> StrataResult<bool> {
        let run_id = run.to_run_id();
        // Primitive returns () - we check if collection existed first
        let existed = self.vector().collection_exists(run_id, collection)
            .map_err(convert_vector_error)?;
        if existed {
            self.vector().delete_collection(run_id, collection)
                .map_err(convert_vector_error)?;
        }
        Ok(existed)
    }

    fn vector_list_collections(&self, run: &ApiRunId) -> StrataResult<Vec<VectorCollectionInfo>> {
        let run_id = run.to_run_id();
        let collections = self.vector().list_collections(run_id)
            .map_err(convert_vector_error)?;

        Ok(collections.into_iter().map(|info| VectorCollectionInfo {
            name: info.name,
            dimension: info.config.dimension,
            count: info.count as u64,
        }).collect())
    }

    fn vector_collection_exists(&self, run: &ApiRunId, collection: &str) -> StrataResult<bool> {
        let run_id = run.to_run_id();
        self.vector().collection_exists(run_id, collection)
            .map_err(convert_vector_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn VectorStore) {}
    }

    #[test]
    fn test_distance_metric_default() {
        assert_eq!(DistanceMetric::default(), DistanceMetric::Cosine);
    }

    #[test]
    fn test_distance_metric_serialization() {
        let metric = DistanceMetric::Euclidean;
        let json = serde_json::to_string(&metric).unwrap();
        assert_eq!(json, "\"euclidean\"");

        let restored: DistanceMetric = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, metric);
    }
}
