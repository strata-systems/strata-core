//! Trace types for the TraceStore primitive
//!
//! These types define the structure of reasoning traces.

use crate::value::Value;
use serde::{Deserialize, Serialize};

/// Types of reasoning traces
///
/// Each variant captures different aspects of agent reasoning:
/// - ToolCall: External tool invocations
/// - Decision: Choice points with reasoning
/// - Query: Information lookups
/// - Thought: Internal reasoning
/// - Error: Error occurrences
/// - Custom: User-defined types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TraceType {
    /// External tool invocation
    ToolCall {
        /// Name of the tool invoked
        tool_name: String,
        /// Arguments passed to the tool
        arguments: Value,
        /// Result returned by the tool (if completed)
        result: Option<Value>,
        /// Duration in milliseconds (if measured)
        duration_ms: Option<u64>,
    },
    /// Decision point with options
    Decision {
        /// The question being decided
        question: String,
        /// Available options
        options: Vec<String>,
        /// The chosen option
        chosen: String,
        /// Reasoning for the choice (optional)
        reasoning: Option<String>,
    },
    /// Information query
    Query {
        /// Type of query (e.g., "database", "api", "search")
        query_type: String,
        /// The actual query
        query: String,
        /// Number of results returned (optional)
        results_count: Option<u32>,
    },
    /// Internal reasoning
    Thought {
        /// The thought content
        content: String,
        /// Confidence level 0.0-1.0 (optional)
        confidence: Option<f64>,
    },
    /// Error occurrence
    Error {
        /// Type of error
        error_type: String,
        /// Error message
        message: String,
        /// Whether the error is recoverable
        recoverable: bool,
    },
    /// User-defined trace type
    Custom {
        /// Custom type name
        name: String,
        /// Custom data
        data: Value,
    },
}

impl TraceType {
    /// Get the type name for indexing
    ///
    /// Returns a stable string identifier for the trace type.
    /// Custom types return their user-defined name.
    pub fn type_name(&self) -> &str {
        match self {
            TraceType::ToolCall { .. } => "ToolCall",
            TraceType::Decision { .. } => "Decision",
            TraceType::Query { .. } => "Query",
            TraceType::Thought { .. } => "Thought",
            TraceType::Error { .. } => "Error",
            TraceType::Custom { name, .. } => name,
        }
    }
}

/// A reasoning trace entry
///
/// Each trace represents a single unit of reasoning with:
/// - Unique identifier
/// - Optional parent for nesting
/// - Typed content
/// - Timestamp for ordering
/// - Tags for filtering
/// - Arbitrary metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Trace {
    /// Unique trace ID (format: "trace-{uuid}")
    pub id: String,
    /// Parent trace ID (if nested)
    pub parent_id: Option<String>,
    /// Type of trace with type-specific data
    pub trace_type: TraceType,
    /// Creation timestamp (milliseconds since epoch)
    pub timestamp: i64,
    /// User-defined tags for filtering
    pub tags: Vec<String>,
    /// Additional metadata
    pub metadata: Value,
}

impl Trace {
    /// Get current timestamp in milliseconds
    pub fn now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}

/// A trace with its children for tree visualization
///
/// Used by `get_tree()` to reconstruct the hierarchical
/// structure of traces from their parent-child relationships.
#[derive(Debug, Clone)]
pub struct TraceTree {
    /// The trace at this node
    pub trace: Trace,
    /// Child traces (recursively)
    pub children: Vec<TraceTree>,
}
