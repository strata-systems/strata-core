//! Vector primitive types and operations
//!
//! This module provides vector storage and similarity search capabilities.
//! It includes:
//!
//! - **VectorConfig**: Collection configuration (dimension, metric, storage type)
//! - **DistanceMetric**: Similarity metrics (Cosine, Euclidean, DotProduct)
//! - **VectorEntry/Match**: Vector storage and search result types
//! - **MetadataFilter**: Equality-based metadata filtering
//! - **VectorError**: Error types for vector operations

pub mod error;
pub mod filter;
pub mod types;

pub use error::{VectorError, VectorResult};
pub use filter::{JsonScalar, MetadataFilter};
pub use types::{
    CollectionId, CollectionInfo, DistanceMetric, StorageDtype, VectorConfig, VectorEntry,
    VectorId, VectorMatch,
};
