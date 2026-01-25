//! Vector Facade - Simplified vector operations
//!
//! This module provides Redis-like vector operations for similarity search.
//!
//! ## Desugaring
//!
//! | Facade | Substrate |
//! |--------|-----------|
//! | `vadd(coll, key, vec, meta)` | `vector_upsert(default_run, coll, key, vec, meta)` |
//! | `vsim(coll, vec, k)` | `vector_search(default_run, coll, vec, k, None, None)` |

use strata_core::{StrataResult, Value};

/// Vector search result
#[derive(Debug, Clone)]
pub struct VectorResult {
    /// Vector key
    pub key: String,
    /// Similarity score (higher = more similar for cosine/dot, lower = more similar for L2)
    pub score: f32,
    /// Vector data (if requested)
    pub vector: Option<Vec<f32>>,
    /// Metadata (if any)
    pub metadata: Value,
}

/// Vector collection summary
#[derive(Debug, Clone)]
pub struct VectorCollectionSummary {
    /// Collection name
    pub name: String,
    /// Vector dimension
    pub dimension: usize,
    /// Number of vectors in the collection
    pub count: u64,
}

/// Search options
#[derive(Debug, Clone, Default)]
pub struct VectorSearchOptions {
    /// Include vector data in results
    pub include_vectors: bool,
    /// Filter by metadata field equals value
    pub filter_eq: Option<(String, Value)>,
}

impl VectorSearchOptions {
    /// Create default options
    pub fn new() -> Self {
        Self::default()
    }

    /// Include vectors in results
    pub fn with_vectors(mut self) -> Self {
        self.include_vectors = true;
        self
    }

    /// Filter by metadata field
    pub fn filter(mut self, field: impl Into<String>, value: Value) -> Self {
        self.filter_eq = Some((field.into(), value));
        self
    }
}

/// Vector Facade - simplified similarity search
///
/// Provides vector storage and K-nearest-neighbor search.
///
/// ## Collection Model
///
/// Vectors are organized into collections. Each collection has a fixed
/// dimension (set from the first vector added).
pub trait VectorFacade {
    /// Add or update a vector
    ///
    /// If the key exists, replaces the vector. Creates collection if needed.
    ///
    /// ## Parameters
    /// - `collection`: Collection name
    /// - `key`: Unique key for this vector
    /// - `vector`: The vector data (f32 array)
    /// - `metadata`: Optional metadata object
    ///
    /// ## Example
    /// ```ignore
    /// // Add embedding with metadata
    /// facade.vadd(
    ///     "embeddings",
    ///     "doc:1",
    ///     &[0.1, 0.2, 0.3, 0.4],
    ///     Some(Value::Object(HashMap::from([
    ///         ("title".to_string(), Value::String("Hello World".to_string())),
    ///     ]))),
    /// )?;
    /// ```
    fn vadd(
        &self,
        collection: &str,
        key: &str,
        vector: &[f32],
        metadata: Option<Value>,
    ) -> StrataResult<()>;

    /// Get a vector by key
    ///
    /// Returns `None` if key doesn't exist.
    fn vget(&self, collection: &str, key: &str) -> StrataResult<Option<(Vec<f32>, Value)>>;

    /// Get a vector by key with version information
    ///
    /// Returns `None` if key doesn't exist.
    /// Returns (vector, metadata, version) if key exists.
    fn vgetv(&self, collection: &str, key: &str) -> StrataResult<Option<(Vec<f32>, Value, u64)>>;

    /// Delete a vector
    ///
    /// Returns `true` if the vector existed.
    fn vdel(&self, collection: &str, key: &str) -> StrataResult<bool>;

    /// Search for similar vectors
    ///
    /// Returns the K most similar vectors to the query.
    ///
    /// ## Parameters
    /// - `collection`: Collection to search
    /// - `query`: Query vector (must match collection dimension)
    /// - `k`: Maximum number of results
    ///
    /// ## Example
    /// ```ignore
    /// let query = vec![0.1, 0.2, 0.3, 0.4];
    /// let results = facade.vsim("embeddings", &query, 10)?;
    ///
    /// for result in results {
    ///     println!("Key: {}, Score: {}", result.key, result.score);
    /// }
    /// ```
    fn vsim(&self, collection: &str, query: &[f32], k: u64) -> StrataResult<Vec<VectorResult>>;

    /// Search with options
    ///
    /// Like `vsim` but with filtering and include options.
    fn vsim_with_options(
        &self,
        collection: &str,
        query: &[f32],
        k: u64,
        options: VectorSearchOptions,
    ) -> StrataResult<Vec<VectorResult>>;

    /// Get collection info
    ///
    /// Returns (dimension, count) or None if collection doesn't exist.
    fn vcollection_info(&self, collection: &str) -> StrataResult<Option<(usize, u64)>>;

    /// Delete a collection
    ///
    /// Removes the collection and all its vectors.
    fn vcollection_drop(&self, collection: &str) -> StrataResult<bool>;

    /// List all collections
    ///
    /// Returns a list of collection names with their dimension and count.
    fn vcollection_list(&self) -> StrataResult<Vec<VectorCollectionSummary>>;

    /// Get the count of vectors in a collection
    ///
    /// Returns 0 if the collection doesn't exist.
    fn vcount(&self, collection: &str) -> StrataResult<u64>;

    // =========================================================================
    // Batch Operations
    // =========================================================================

    /// Add or update multiple vectors
    ///
    /// More efficient than calling `vadd` multiple times.
    ///
    /// ## Parameters
    /// - `collection`: Collection name
    /// - `vectors`: Vector of (key, vector, metadata) tuples
    ///
    /// ## Returns
    /// Number of vectors successfully added/updated.
    ///
    /// ## Example
    /// ```ignore
    /// let vectors = vec![
    ///     ("doc:1", vec![0.1, 0.2, 0.3].as_slice(), None),
    ///     ("doc:2", vec![0.4, 0.5, 0.6].as_slice(), Some(metadata)),
    /// ];
    /// let count = facade.vadd_batch("embeddings", vectors)?;
    /// ```
    fn vadd_batch(
        &self,
        collection: &str,
        vectors: Vec<(&str, &[f32], Option<Value>)>,
    ) -> StrataResult<usize>;

    /// Get multiple vectors by key
    ///
    /// More efficient than calling `vget` multiple times.
    ///
    /// ## Parameters
    /// - `collection`: Collection name
    /// - `keys`: Keys to retrieve
    ///
    /// ## Returns
    /// Vector of results in the same order as input keys.
    /// Missing keys return `None`.
    fn vget_batch(
        &self,
        collection: &str,
        keys: &[&str],
    ) -> StrataResult<Vec<Option<(Vec<f32>, Value)>>>;

    /// Delete multiple vectors by key
    ///
    /// More efficient than calling `vdel` multiple times.
    ///
    /// ## Parameters
    /// - `collection`: Collection name
    /// - `keys`: Keys to delete
    ///
    /// ## Returns
    /// Number of vectors that existed and were deleted.
    fn vdel_batch(&self, collection: &str, keys: &[&str]) -> StrataResult<usize>;

    /// Get version history for a vector
    ///
    /// Returns all historical versions of a vector, newest first.
    ///
    /// ## Parameters
    /// - `collection`: Collection name
    /// - `key`: Vector key
    /// - `limit`: Maximum number of versions to return (None = all)
    ///
    /// ## Returns
    /// Vector of (embedding, metadata, version) tuples in descending version order.
    /// Empty if the key doesn't exist.
    fn vhistory(
        &self,
        collection: &str,
        key: &str,
        limit: Option<usize>,
    ) -> StrataResult<Vec<(Vec<f32>, Value, u64)>>;

    /// List all vector keys in a collection
    ///
    /// Returns keys in lexicographical order with optional pagination.
    ///
    /// ## Parameters
    /// - `collection`: Collection name
    /// - `limit`: Maximum number of keys to return (None = all)
    /// - `cursor`: Start from key greater than this (for pagination)
    ///
    /// ## Returns
    /// Vector of keys in lexicographical order.
    fn vlist(
        &self,
        collection: &str,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> StrataResult<Vec<String>>;

    /// Scan vectors in a collection
    ///
    /// Returns vectors in lexicographical key order with optional pagination.
    ///
    /// ## Parameters
    /// - `collection`: Collection name
    /// - `limit`: Maximum number of vectors to return (None = all)
    /// - `cursor`: Start from key greater than this (for pagination)
    ///
    /// ## Returns
    /// Vector of (key, embedding, metadata) tuples in key order.
    fn vscan(
        &self,
        collection: &str,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> StrataResult<Vec<(String, Vec<f32>, Value)>>;
}

// =============================================================================
// Implementation
// =============================================================================

use super::impl_::FacadeImpl;
use crate::substrate::{VectorStore as SubstrateVectorStore, DistanceMetric, SearchFilter};

impl VectorFacade for FacadeImpl {
    fn vadd(
        &self,
        collection: &str,
        key: &str,
        vector: &[f32],
        metadata: Option<Value>,
    ) -> StrataResult<()> {
        let info = self.substrate().vector_collection_info(self.default_run(), collection)?;
        if info.is_none() {
            let _version = self.substrate().vector_create_collection(
                self.default_run(),
                collection,
                vector.len(),
                DistanceMetric::Cosine,
            )?;
        }

        let _version = self.substrate().vector_upsert(
            self.default_run(),
            collection,
            key,
            vector,
            metadata,
        )?;
        Ok(())
    }

    fn vget(&self, collection: &str, key: &str) -> StrataResult<Option<(Vec<f32>, Value)>> {
        let result = self.substrate().vector_get(self.default_run(), collection, key)?;
        Ok(result.map(|v| v.value))
    }

    fn vgetv(&self, collection: &str, key: &str) -> StrataResult<Option<(Vec<f32>, Value, u64)>> {
        let result = self.substrate().vector_get(self.default_run(), collection, key)?;
        Ok(result.map(|v| {
            let version = match v.version {
                strata_core::Version::Txn(txn) => txn,
                strata_core::Version::Counter(c) => c,
                strata_core::Version::Sequence(s) => s,
            };
            (v.value.0, v.value.1, version)
        }))
    }

    fn vdel(&self, collection: &str, key: &str) -> StrataResult<bool> {
        self.substrate().vector_delete(self.default_run(), collection, key)
    }

    fn vsim(&self, collection: &str, query: &[f32], k: u64) -> StrataResult<Vec<VectorResult>> {
        let results = self.substrate().vector_search(
            self.default_run(),
            collection,
            query,
            k,
            None,
            None,
        )?;
        Ok(results.into_iter().map(|m| VectorResult {
            key: m.key,
            score: m.score,
            vector: None,
            metadata: m.metadata,
        }).collect())
    }

    fn vsim_with_options(
        &self,
        collection: &str,
        query: &[f32],
        k: u64,
        options: VectorSearchOptions,
    ) -> StrataResult<Vec<VectorResult>> {
        let filter = options.filter_eq.map(|(field, value)| SearchFilter::Equals { field, value });
        let results = self.substrate().vector_search(
            self.default_run(),
            collection,
            query,
            k,
            filter,
            None,
        )?;
        Ok(results.into_iter().map(|m| {
            let vector = if options.include_vectors {
                Some(m.vector.clone())
            } else {
                None
            };
            VectorResult {
                key: m.key,
                score: m.score,
                vector,
                metadata: m.metadata,
            }
        }).collect())
    }

    fn vcollection_info(&self, collection: &str) -> StrataResult<Option<(usize, u64)>> {
        let info = self.substrate().vector_collection_info(self.default_run(), collection)?;
        Ok(info.map(|i| (i.dimension, i.count)))
    }

    fn vcollection_drop(&self, collection: &str) -> StrataResult<bool> {
        self.substrate().vector_drop_collection(self.default_run(), collection)
    }

    fn vcollection_list(&self) -> StrataResult<Vec<VectorCollectionSummary>> {
        let collections = self.substrate().vector_list_collections(self.default_run())?;
        Ok(collections.into_iter().map(|c| VectorCollectionSummary {
            name: c.name,
            dimension: c.dimension,
            count: c.count,
        }).collect())
    }

    fn vcount(&self, collection: &str) -> StrataResult<u64> {
        self.substrate().vector_count(self.default_run(), collection)
    }

    fn vadd_batch(
        &self,
        collection: &str,
        vectors: Vec<(&str, &[f32], Option<Value>)>,
    ) -> StrataResult<usize> {
        if vectors.is_empty() {
            return Ok(0);
        }

        // Ensure collection exists (using first vector's dimension)
        let info = self.substrate().vector_collection_info(self.default_run(), collection)?;
        if info.is_none() {
            if let Some((_, first_vec, _)) = vectors.iter().find(|(_, v, _)| !v.is_empty()) {
                let _version = self.substrate().vector_create_collection(
                    self.default_run(),
                    collection,
                    first_vec.len(),
                    DistanceMetric::Cosine,
                )?;
            }
        }

        let results = self.substrate().vector_upsert_batch(
            self.default_run(),
            collection,
            vectors,
        )?;

        // Count successful inserts
        Ok(results.into_iter().filter(|r| r.is_ok()).count())
    }

    fn vget_batch(
        &self,
        collection: &str,
        keys: &[&str],
    ) -> StrataResult<Vec<Option<(Vec<f32>, Value)>>> {
        let results = self.substrate().vector_get_batch(
            self.default_run(),
            collection,
            keys,
        )?;

        Ok(results
            .into_iter()
            .map(|opt| opt.map(|v| v.value))
            .collect())
    }

    fn vdel_batch(&self, collection: &str, keys: &[&str]) -> StrataResult<usize> {
        let results = self.substrate().vector_delete_batch(
            self.default_run(),
            collection,
            keys,
        )?;

        // Count successful deletes (true = existed and was deleted)
        Ok(results.into_iter().filter(|&deleted| deleted).count())
    }

    fn vhistory(
        &self,
        collection: &str,
        key: &str,
        limit: Option<usize>,
    ) -> StrataResult<Vec<(Vec<f32>, Value, u64)>> {
        let history = self.substrate().vector_history(
            self.default_run(),
            collection,
            key,
            limit,
            None, // No before_version pagination in facade
        )?;

        Ok(history.into_iter().map(|v| {
            let version = match v.version {
                strata_core::Version::Txn(txn) => txn,
                strata_core::Version::Counter(c) => c,
                strata_core::Version::Sequence(s) => s,
            };
            (v.value.0, v.value.1, version)
        }).collect())
    }

    fn vlist(
        &self,
        collection: &str,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> StrataResult<Vec<String>> {
        self.substrate().vector_list_keys(
            self.default_run(),
            collection,
            limit,
            cursor,
        )
    }

    fn vscan(
        &self,
        collection: &str,
        limit: Option<usize>,
        cursor: Option<&str>,
    ) -> StrataResult<Vec<(String, Vec<f32>, Value)>> {
        let results = self.substrate().vector_scan(
            self.default_run(),
            collection,
            limit,
            cursor,
        )?;

        Ok(results
            .into_iter()
            .map(|(key, (embedding, metadata))| (key, embedding, metadata))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn VectorFacade) {}
    }

    #[test]
    fn test_search_options() {
        let opts = VectorSearchOptions::new()
            .with_vectors()
            .filter("type", Value::String("article".to_string()));

        assert!(opts.include_vectors);
        assert!(opts.filter_eq.is_some());
    }
}
