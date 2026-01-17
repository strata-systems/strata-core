//! Vector primitive types and operations
//!
//! This module provides vector storage and similarity search capabilities.
//! It includes:
//!
//! - **VectorStore**: Main facade for vector storage and search
//! - **VectorConfig**: Collection configuration (dimension, metric, storage type)
//! - **DistanceMetric**: Similarity metrics (Cosine, Euclidean, DotProduct)
//! - **VectorEntry/Match**: Vector storage and search result types
//! - **VectorHeap**: Contiguous embedding storage with slot reuse
//! - **VectorIndexBackend**: Trait for swappable index implementations
//! - **BruteForceBackend**: O(n) brute-force search
//! - **MetadataFilter**: Equality-based metadata filtering
//! - **VectorError**: Error types for vector operations

pub mod backend;
pub mod brute_force;
pub mod collection;
pub mod error;
pub mod filter;
pub mod heap;
pub mod store;
pub mod types;

pub use backend::{IndexBackendFactory, VectorIndexBackend};
pub use brute_force::BruteForceBackend;
pub use collection::{validate_collection_name, validate_vector_key};
pub use error::{VectorError, VectorResult};
pub use filter::{JsonScalar, MetadataFilter};
pub use heap::VectorHeap;
pub use store::VectorStore;
pub use types::{
    CollectionId, CollectionInfo, CollectionRecord, DistanceMetric, StorageDtype, VectorConfig,
    VectorConfigSerde, VectorEntry, VectorId, VectorMatch, VectorRecord,
};
