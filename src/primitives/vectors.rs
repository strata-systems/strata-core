//! Vector similarity search primitive.
//!
//! The Vectors primitive provides vector embeddings storage with
//! similarity search and metadata filtering.

use crate::error::Result;
use crate::types::{run_id_to_api, DistanceMetric, RunId, Value, Version, Versioned};
use std::sync::Arc;

use strata_api::substrate::{SearchFilter, VectorData, VectorMatch, VectorStore};
use strata_api::substrate::vector::VectorCollectionInfo;

/// Vector similarity search operations.
///
/// Access via `db.vectors`.
pub struct Vectors {
    #[allow(dead_code)]
    db: Arc<strata_engine::Database>,
    substrate: strata_api::substrate::SubstrateImpl,
}

impl Vectors {
    pub(crate) fn new(db: Arc<strata_engine::Database>) -> Self {
        let substrate = strata_api::substrate::SubstrateImpl::new(db.clone());
        Self { db, substrate }
    }

    // =========================================================================
    // Collection management
    // =========================================================================

    /// Create a vector collection.
    ///
    /// # Arguments
    ///
    /// * `name` - Collection name
    /// * `dimension` - Vector dimension (must match all vectors in collection)
    /// * `metric` - Distance metric for similarity search
    ///
    /// # Example
    ///
    /// ```ignore
    /// db.vectors.create_collection(
    ///     &run,
    ///     "embeddings",
    ///     384,
    ///     DistanceMetric::Cosine,
    /// )?;
    /// ```
    pub fn create_collection(
        &self,
        run: &RunId,
        name: &str,
        dimension: usize,
        metric: DistanceMetric,
    ) -> Result<Version> {
        let api_run = run_id_to_api(run);
        // Convert strata_core::DistanceMetric to strata_api::substrate::DistanceMetric
        let api_metric = match metric {
            strata_core::DistanceMetric::Cosine => strata_api::substrate::DistanceMetric::Cosine,
            strata_core::DistanceMetric::Euclidean => strata_api::substrate::DistanceMetric::Euclidean,
            strata_core::DistanceMetric::DotProduct => strata_api::substrate::DistanceMetric::DotProduct,
        };
        Ok(self.substrate.vector_create_collection(&api_run, name, dimension, api_metric)?)
    }

    /// Delete a vector collection.
    pub fn delete_collection(&self, run: &RunId, collection: &str) -> Result<bool> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.vector_drop_collection(&api_run, collection)?)
    }

    /// List all collections in a run.
    pub fn list_collections(&self, run: &RunId) -> Result<Vec<VectorCollectionInfo>> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.vector_list_collections(&api_run)?)
    }

    // =========================================================================
    // Vector operations
    // =========================================================================

    /// Upsert a vector.
    ///
    /// Inserts or updates a vector with optional metadata.
    ///
    /// # Example
    ///
    /// ```ignore
    /// db.vectors.upsert(
    ///     &run,
    ///     "embeddings",
    ///     "doc-1",
    ///     &embedding,
    ///     Some(Value::from_json(json!({"title": "Hello World"}))),
    /// )?;
    /// ```
    pub fn upsert(
        &self,
        run: &RunId,
        collection: &str,
        key: &str,
        vector: &[f32],
        metadata: Option<Value>,
    ) -> Result<Version> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.vector_upsert(&api_run, collection, key, vector, metadata)?)
    }

    /// Get a vector by key.
    pub fn get(
        &self,
        run: &RunId,
        collection: &str,
        key: &str,
    ) -> Result<Option<Versioned<VectorData>>> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.vector_get(&api_run, collection, key)?)
    }

    /// Delete a vector.
    pub fn delete(&self, run: &RunId, collection: &str, key: &str) -> Result<bool> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.vector_delete(&api_run, collection, key)?)
    }

    // =========================================================================
    // Search
    // =========================================================================

    /// Search for similar vectors.
    ///
    /// # Arguments
    ///
    /// * `collection` - Collection to search
    /// * `query` - Query vector
    /// * `k` - Number of results to return
    /// * `filter` - Optional metadata filter
    ///
    /// # Example
    ///
    /// ```ignore
    /// let results = db.vectors.search(
    ///     &run,
    ///     "embeddings",
    ///     &query_embedding,
    ///     10,
    ///     None,
    /// )?;
    ///
    /// for result in results {
    ///     println!("{}: score={}", result.key, result.score);
    /// }
    /// ```
    pub fn search(
        &self,
        run: &RunId,
        collection: &str,
        query: &[f32],
        k: usize,
        filter: Option<SearchFilter>,
    ) -> Result<Vec<VectorMatch>> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.vector_search(&api_run, collection, query, k as u64, filter, None)?)
    }

    /// Search with score threshold.
    ///
    /// Returns only results with similarity score above the threshold.
    pub fn search_with_threshold(
        &self,
        run: &RunId,
        collection: &str,
        query: &[f32],
        k: usize,
        threshold: f32,
        filter: Option<SearchFilter>,
    ) -> Result<Vec<VectorMatch>> {
        let api_run = run_id_to_api(run);
        let results = self.substrate.vector_search(&api_run, collection, query, k as u64, filter, None)?;
        Ok(results.into_iter().filter(|m| m.score >= threshold).collect())
    }

    /// Count vectors in a collection.
    pub fn count(&self, run: &RunId, collection: &str) -> Result<u64> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.vector_count(&api_run, collection)?)
    }

    /// Get collection info.
    pub fn collection_info(&self, run: &RunId, collection: &str) -> Result<Option<VectorCollectionInfo>> {
        let api_run = run_id_to_api(run);
        Ok(self.substrate.vector_collection_info(&api_run, collection)?)
    }
}
