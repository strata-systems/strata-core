//! Vector Index Backend trait
//!
//! Defines the interface for swappable vector index implementations.
//! M8: BruteForceBackend (O(n) search)
//! M9: HnswBackend (O(log n) search) - reserved

use crate::vector::{DistanceMetric, VectorConfig, VectorError, VectorId};

/// Trait for swappable vector index implementations
///
/// M8: BruteForceBackend (O(n) search)
/// M9: HnswBackend (O(log n) search)
///
/// IMPORTANT: This trait is designed to work for BOTH brute-force and HNSW.
/// Do NOT add methods that assume brute-force semantics (like get_all_vectors).
/// See Evolution Warning A in M8_ARCHITECTURE.md.
pub trait VectorIndexBackend: Send + Sync {
    /// Insert a vector (upsert semantics)
    ///
    /// If the VectorId already exists, updates the embedding.
    /// The VectorId is assigned externally and passed in.
    fn insert(&mut self, id: VectorId, embedding: &[f32]) -> Result<(), VectorError>;

    /// Delete a vector
    ///
    /// Returns true if the vector existed and was deleted.
    fn delete(&mut self, id: VectorId) -> Result<bool, VectorError>;

    /// Search for k nearest neighbors
    ///
    /// Returns (VectorId, score) pairs.
    /// Scores are normalized to "higher = more similar" (Invariant R2).
    /// Results are sorted by (score desc, VectorId asc) for determinism (Invariant R4).
    fn search(&self, query: &[f32], k: usize) -> Vec<(VectorId, f32)>;

    /// Get number of indexed vectors
    fn len(&self) -> usize;

    /// Check if empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get embedding dimension
    fn dimension(&self) -> usize;

    /// Get distance metric
    fn metric(&self) -> DistanceMetric;

    /// Get a vector by ID (for metadata lookups after search)
    fn get(&self, id: VectorId) -> Option<&[f32]>;

    /// Check if a vector exists
    fn contains(&self, id: VectorId) -> bool;
}

/// Factory for creating index backends
///
/// This abstraction allows switching between BruteForce (M8) and HNSW (M9)
/// without changing the VectorStore code.
#[derive(Clone, Default)]
pub enum IndexBackendFactory {
    /// Brute-force O(n) search
    #[default]
    BruteForce,
    // Hnsw(HnswConfig),  // Reserved for M9
}

impl IndexBackendFactory {
    /// Create a new backend instance
    pub fn create(&self, config: &VectorConfig) -> Box<dyn VectorIndexBackend> {
        match self {
            IndexBackendFactory::BruteForce => {
                Box::new(super::brute_force::BruteForceBackend::new(config))
            }
        }
    }
}
