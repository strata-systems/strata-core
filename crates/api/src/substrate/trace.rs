//! TraceStore Substrate Operations
//!
//! The TraceStore provides structured logging for reasoning traces.
//! It supports hierarchical traces with parent-child relationships.
//!
//! ## Trace Model
//!
//! - Traces form a forest (multiple roots) or a tree (single root)
//! - Each trace has a unique ID
//! - Traces can have a parent (forming a hierarchy)
//! - Traces can have type and tags for categorization
//!
//! ## Trace Types
//!
//! - `Thought`: Internal reasoning
//! - `Action`: Executed action
//! - `Observation`: External observation
//! - `Tool`: Tool invocation
//! - `Message`: User/assistant message
//!
//! ## Versioning
//!
//! Traces use transaction-based versioning (`Version::Txn`).

use super::types::ApiRunId;
use strata_core::{StrataResult, Value, Version, Versioned};
use serde::{Deserialize, Serialize};

/// Trace type for categorization
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceType {
    /// Internal reasoning
    #[default]
    Thought,
    /// Executed action
    Action,
    /// External observation
    Observation,
    /// Tool invocation
    Tool,
    /// User or assistant message
    Message,
    /// Custom type
    Custom(String),
}

/// A trace entry
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceEntry {
    /// Unique trace ID
    pub id: String,
    /// Trace type
    pub trace_type: TraceType,
    /// Parent trace ID (if any)
    pub parent_id: Option<String>,
    /// Trace content/payload
    pub content: Value,
    /// Tags for filtering
    pub tags: Vec<String>,
    /// Creation timestamp (microseconds since epoch)
    pub created_at: u64,
}

/// TraceStore substrate operations
///
/// This trait defines the canonical trace store operations.
/// All operations require explicit run_id and return versioned results.
///
/// ## Contract
///
/// - Trace IDs are unique within a run
/// - Parent references must point to existing traces
/// - Content must be `Value::Object`
///
/// ## Error Handling
///
/// | Condition | Error |
/// |-----------|-------|
/// | Invalid trace ID | `InvalidKey` |
/// | Invalid parent reference | `NotFound` |
/// | Content not Object | `ConstraintViolation` |
/// | Run not found | `NotFound` |
/// | Run is closed | `ConstraintViolation` |
pub trait TraceStore {
    /// Create a new trace
    ///
    /// Adds a new trace entry and returns its version.
    ///
    /// ## Parameters
    ///
    /// - `trace_type`: Type of trace for categorization
    /// - `parent_id`: Optional parent trace ID
    /// - `content`: Trace content (must be Object)
    /// - `tags`: Optional tags for filtering
    ///
    /// ## Return Value
    ///
    /// Returns `(trace_id, version)` where `trace_id` is a generated UUID.
    ///
    /// ## Errors
    ///
    /// - `ConstraintViolation`: Content is not Object, or run is closed
    /// - `NotFound`: Run or parent trace does not exist
    fn trace_create(
        &self,
        run: &ApiRunId,
        trace_type: TraceType,
        parent_id: Option<&str>,
        content: Value,
        tags: Vec<String>,
    ) -> StrataResult<(String, Version)>;

    /// Create a trace with explicit ID
    ///
    /// Like `trace_create`, but with a caller-provided ID.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Trace ID is invalid
    /// - `ConstraintViolation`: ID already exists, content not Object, or run closed
    /// - `NotFound`: Run or parent trace does not exist
    fn trace_create_with_id(
        &self,
        run: &ApiRunId,
        id: &str,
        trace_type: TraceType,
        parent_id: Option<&str>,
        content: Value,
        tags: Vec<String>,
    ) -> StrataResult<Version>;

    /// Get a trace by ID
    ///
    /// Returns the trace entry.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Trace ID is invalid
    /// - `NotFound`: Run or trace does not exist
    fn trace_get(&self, run: &ApiRunId, id: &str) -> StrataResult<Option<Versioned<TraceEntry>>>;

    /// List traces with optional filters
    ///
    /// Returns traces matching the filters, newest first.
    ///
    /// ## Parameters
    ///
    /// - `trace_type`: Filter by type
    /// - `parent_id`: Filter by parent (`Some(None)` = roots only, `None` = no filter)
    /// - `tag`: Filter by tag (trace must have this tag)
    /// - `limit`: Maximum traces to return
    /// - `before`: Return traces older than this (exclusive)
    ///
    /// ## Errors
    ///
    /// - `NotFound`: Run does not exist
    fn trace_list(
        &self,
        run: &ApiRunId,
        trace_type: Option<TraceType>,
        parent_id: Option<Option<&str>>,
        tag: Option<&str>,
        limit: Option<u64>,
        before: Option<Version>,
    ) -> StrataResult<Vec<Versioned<TraceEntry>>>;

    /// Get child traces
    ///
    /// Returns all traces with the given parent ID.
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Parent ID is invalid
    /// - `NotFound`: Run or parent trace does not exist
    fn trace_children(
        &self,
        run: &ApiRunId,
        parent_id: &str,
    ) -> StrataResult<Vec<Versioned<TraceEntry>>>;

    /// Get the trace tree rooted at the given trace
    ///
    /// Returns the trace and all its descendants.
    /// Order is pre-order (parent before children).
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Trace ID is invalid
    /// - `NotFound`: Run or trace does not exist
    fn trace_tree(&self, run: &ApiRunId, root_id: &str) -> StrataResult<Vec<Versioned<TraceEntry>>>;

    /// Update trace tags
    ///
    /// Adds or removes tags from a trace.
    /// Returns the new version.
    ///
    /// ## Parameters
    ///
    /// - `add_tags`: Tags to add
    /// - `remove_tags`: Tags to remove (if present)
    ///
    /// ## Errors
    ///
    /// - `InvalidKey`: Trace ID is invalid
    /// - `NotFound`: Run or trace does not exist
    /// - `ConstraintViolation`: Run is closed
    fn trace_update_tags(
        &self,
        run: &ApiRunId,
        id: &str,
        add_tags: Vec<String>,
        remove_tags: Vec<String>,
    ) -> StrataResult<Version>;

    /// Query traces by tag
    ///
    /// Returns all traces that have the specified tag.
    ///
    /// ## Errors
    ///
    /// - `NotFound`: Run does not exist
    fn trace_query_by_tag(&self, run: &ApiRunId, tag: &str) -> StrataResult<Vec<Versioned<TraceEntry>>>;

    /// Query traces by time range
    ///
    /// Returns all traces created within the specified time range.
    ///
    /// ## Parameters
    ///
    /// - `start_ms`: Start time (inclusive), milliseconds since epoch
    /// - `end_ms`: End time (inclusive), milliseconds since epoch
    ///
    /// ## Errors
    ///
    /// - `NotFound`: Run does not exist
    fn trace_query_by_time(
        &self,
        run: &ApiRunId,
        start_ms: i64,
        end_ms: i64,
    ) -> StrataResult<Vec<Versioned<TraceEntry>>>;

    /// Count traces in a run
    ///
    /// Returns the total number of traces.
    ///
    /// ## Errors
    ///
    /// - `NotFound`: Run does not exist
    fn trace_count(&self, run: &ApiRunId) -> StrataResult<u64>;

    /// Search traces
    ///
    /// Performs full-text search across trace content.
    ///
    /// ## Parameters
    ///
    /// - `query`: Search query string
    /// - `k`: Maximum results to return
    ///
    /// ## Return Value
    ///
    /// Search results with trace IDs and relevance scores.
    ///
    /// ## Errors
    ///
    /// - `NotFound`: Run does not exist
    fn trace_search(&self, run: &ApiRunId, query: &str, k: u64) -> StrataResult<Vec<TraceSearchHit>>;
}

/// A search hit in trace search
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TraceSearchHit {
    /// Trace ID
    pub id: String,
    /// Relevance score (higher = more relevant)
    pub score: f32,
}

// =============================================================================
// Implementation
// =============================================================================
//
// Note: Substrate TraceType is simple (Thought, Action, etc.)
// Primitive TraceType is rich (Thought{content, confidence}, ToolCall{...}, etc.)
// We map substrate types to primitive's Custom type with the content as data.

use strata_core::StrataError;
use super::impl_::{SubstrateImpl, convert_error};

impl TraceStore for SubstrateImpl {
    fn trace_create(
        &self,
        run: &ApiRunId,
        trace_type: TraceType,
        parent_id: Option<&str>,
        content: Value,
        tags: Vec<String>,
    ) -> StrataResult<(String, Version)> {
        let run_id = run.to_run_id();
        let primitive_type = convert_trace_type_to_primitive(&trace_type, &content);

        let versioned = match parent_id {
            Some(pid) => self.trace().record_child(&run_id, pid, primitive_type, tags, content)
                .map_err(convert_error)?,
            None => self.trace().record(&run_id, primitive_type, tags, content)
                .map_err(convert_error)?,
        };

        Ok((versioned.value, versioned.version))
    }

    fn trace_create_with_id(
        &self,
        _run: &ApiRunId,
        _id: &str,
        _trace_type: TraceType,
        _parent_id: Option<&str>,
        _content: Value,
        _tags: Vec<String>,
    ) -> StrataResult<Version> {
        // STUB: Primitive doesn't support explicit IDs (always generates UUID)
        // This would require database-level implementation
        Err(StrataError::invalid_operation(
            strata_core::EntityRef::run(_run.to_run_id()),
            "trace_create_with_id not supported - primitive always generates IDs",
        ))
    }

    fn trace_get(&self, run: &ApiRunId, id: &str) -> StrataResult<Option<Versioned<TraceEntry>>> {
        let run_id = run.to_run_id();
        let result = self.trace().get(&run_id, id).map_err(convert_error)?;

        Ok(result.map(|versioned| {
            let trace = versioned.value;
            Versioned {
                value: convert_primitive_trace_to_entry(trace),
                version: versioned.version,
                timestamp: versioned.timestamp,
            }
        }))
    }

    fn trace_list(
        &self,
        run: &ApiRunId,
        trace_type: Option<TraceType>,
        parent_id: Option<Option<&str>>,
        tag: Option<&str>,
        limit: Option<u64>,
        _before: Option<Version>,
    ) -> StrataResult<Vec<Versioned<TraceEntry>>> {
        let run_id = run.to_run_id();

        // Start with most selective filter
        let mut traces: Vec<strata_core::Trace> = if let Some(ref t) = trace_type {
            let type_name = substrate_trace_type_name(t);
            self.trace().query_by_type(&run_id, &type_name).map_err(convert_error)?
        } else if let Some(t) = tag {
            self.trace().query_by_tag(&run_id, t).map_err(convert_error)?
        } else if let Some(maybe_parent) = parent_id {
            match maybe_parent {
                Some(pid) => self.trace().get_children(&run_id, pid).map_err(convert_error)?,
                None => self.trace().get_roots(&run_id).map_err(convert_error)?,
            }
        } else {
            self.trace().list(&run_id).map_err(convert_error)?
        };

        // Sort newest first
        traces.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Apply limit
        if let Some(n) = limit {
            traces.truncate(n as usize);
        }

        // Convert to Versioned<TraceEntry> - note version info is lost from query
        Ok(traces
            .into_iter()
            .map(|t| Versioned {
                value: convert_primitive_trace_to_entry(t),
                version: Version::Txn(0), // Version lost in primitive query methods
                timestamp: strata_core::Timestamp::now(),
            })
            .collect())
    }

    fn trace_children(
        &self,
        run: &ApiRunId,
        parent_id: &str,
    ) -> StrataResult<Vec<Versioned<TraceEntry>>> {
        let run_id = run.to_run_id();
        let traces = self.trace().get_children(&run_id, parent_id).map_err(convert_error)?;

        // Version info lost - primitive returns Vec<Trace> not Vec<Versioned<Trace>>
        Ok(traces
            .into_iter()
            .map(|t| Versioned {
                value: convert_primitive_trace_to_entry(t),
                version: Version::Txn(0),
                timestamp: strata_core::Timestamp::now(),
            })
            .collect())
    }

    fn trace_tree(&self, run: &ApiRunId, root_id: &str) -> StrataResult<Vec<Versioned<TraceEntry>>> {
        let run_id = run.to_run_id();
        let tree = self.trace().get_tree(&run_id, root_id).map_err(convert_error)?;

        match tree {
            Some(trace_tree) => {
                // Flatten tree in pre-order
                let mut traces = Vec::new();
                flatten_trace_tree(trace_tree, &mut traces);

                Ok(traces
                    .into_iter()
                    .map(|t| Versioned {
                        value: convert_primitive_trace_to_entry(t),
                        version: Version::Txn(0),
                        timestamp: strata_core::Timestamp::now(),
                    })
                    .collect())
            }
            None => Ok(vec![]),
        }
    }

    fn trace_update_tags(
        &self,
        _run: &ApiRunId,
        _id: &str,
        _add_tags: Vec<String>,
        _remove_tags: Vec<String>,
    ) -> StrataResult<Version> {
        // STUB: Primitive is append-only - no tag updates supported
        // This would require rewriting the trace and updating indices
        Err(StrataError::invalid_operation(
            strata_core::EntityRef::run(_run.to_run_id()),
            "trace_update_tags not supported - traces are append-only",
        ))
    }

    fn trace_query_by_tag(&self, run: &ApiRunId, tag: &str) -> StrataResult<Vec<Versioned<TraceEntry>>> {
        let run_id = run.to_run_id();
        let traces = self.trace().query_by_tag(&run_id, tag).map_err(convert_error)?;

        Ok(traces
            .into_iter()
            .map(|t| Versioned {
                value: convert_primitive_trace_to_entry(t),
                version: Version::Txn(0),
                timestamp: strata_core::Timestamp::now(),
            })
            .collect())
    }

    fn trace_query_by_time(
        &self,
        run: &ApiRunId,
        start_ms: i64,
        end_ms: i64,
    ) -> StrataResult<Vec<Versioned<TraceEntry>>> {
        let run_id = run.to_run_id();
        let traces = self.trace().query_by_time(&run_id, start_ms, end_ms).map_err(convert_error)?;

        Ok(traces
            .into_iter()
            .map(|t| Versioned {
                value: convert_primitive_trace_to_entry(t),
                version: Version::Txn(0),
                timestamp: strata_core::Timestamp::now(),
            })
            .collect())
    }

    fn trace_count(&self, run: &ApiRunId) -> StrataResult<u64> {
        let run_id = run.to_run_id();
        let count = self.trace().count(&run_id).map_err(convert_error)?;
        Ok(count as u64)
    }

    fn trace_search(&self, run: &ApiRunId, query: &str, k: u64) -> StrataResult<Vec<TraceSearchHit>> {
        let run_id = run.to_run_id();
        let request = strata_core::SearchRequest::new(run_id, query).with_k(k as usize);
        let response = self.trace().search(&request).map_err(convert_error)?;

        Ok(response.hits.into_iter().map(|hit| {
            let id = match hit.doc_ref {
                strata_core::search_types::DocRef::Trace { trace_id, .. } => trace_id,
                _ => String::new(),
            };
            TraceSearchHit {
                id,
                score: hit.score,
            }
        }).collect())
    }
}

/// Flatten a TraceTree into a Vec<Trace> in pre-order
fn flatten_trace_tree(tree: strata_core::TraceTree, result: &mut Vec<strata_core::Trace>) {
    result.push(tree.trace);
    for child in tree.children {
        flatten_trace_tree(child, result);
    }
}

/// Convert substrate TraceType to primitive TraceType
///
/// Since primitive has richer types, we map to Custom with the type name
fn convert_trace_type_to_primitive(t: &TraceType, content: &Value) -> strata_core::TraceType {
    match t {
        TraceType::Thought => strata_core::TraceType::Thought {
            content: serde_json::to_string(content).unwrap_or_default(),
            confidence: None,
        },
        TraceType::Tool => strata_core::TraceType::ToolCall {
            tool_name: extract_tool_name(content),
            arguments: content.clone(),
            result: None,
            duration_ms: None,
        },
        TraceType::Action => strata_core::TraceType::Custom {
            name: "Action".to_string(),
            data: content.clone(),
        },
        TraceType::Observation => strata_core::TraceType::Custom {
            name: "Observation".to_string(),
            data: content.clone(),
        },
        TraceType::Message => strata_core::TraceType::Custom {
            name: "Message".to_string(),
            data: content.clone(),
        },
        TraceType::Custom(name) => strata_core::TraceType::Custom {
            name: name.clone(),
            data: content.clone(),
        },
    }
}

/// Extract tool name from content Value (best effort)
fn extract_tool_name(content: &Value) -> String {
    match content {
        Value::Object(map) => {
            map.get("tool_name")
                .or_else(|| map.get("name"))
                .or_else(|| map.get("tool"))
                .and_then(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "unknown".to_string())
        }
        _ => "unknown".to_string(),
    }
}

/// Convert primitive TraceType back to substrate TraceType
fn convert_primitive_trace_type_to_substrate(t: &strata_core::TraceType) -> TraceType {
    match t {
        strata_core::TraceType::Thought { .. } => TraceType::Thought,
        strata_core::TraceType::ToolCall { .. } => TraceType::Tool,
        strata_core::TraceType::Decision { .. } => TraceType::Custom("Decision".to_string()),
        strata_core::TraceType::Query { .. } => TraceType::Custom("Query".to_string()),
        strata_core::TraceType::Error { .. } => TraceType::Custom("Error".to_string()),
        strata_core::TraceType::Custom { name, .. } => {
            // Map back known custom names
            match name.as_str() {
                "Action" => TraceType::Action,
                "Observation" => TraceType::Observation,
                "Message" => TraceType::Message,
                _ => TraceType::Custom(name.clone()),
            }
        }
    }
}

/// Get the type name for a substrate TraceType (for querying)
fn substrate_trace_type_name(t: &TraceType) -> String {
    match t {
        TraceType::Thought => "Thought".to_string(),
        TraceType::Action => "Action".to_string(),
        TraceType::Observation => "Observation".to_string(),
        TraceType::Tool => "ToolCall".to_string(),
        TraceType::Message => "Message".to_string(),
        TraceType::Custom(name) => name.clone(),
    }
}

/// Convert primitive Trace to substrate TraceEntry
fn convert_primitive_trace_to_entry(t: strata_core::Trace) -> TraceEntry {
    TraceEntry {
        id: t.id,
        trace_type: convert_primitive_trace_type_to_substrate(&t.trace_type),
        parent_id: t.parent_id,
        content: t.metadata, // Primitive uses 'metadata', substrate uses 'content'
        tags: t.tags,
        // Primitive stores timestamp as i64 millis, convert to u64 micros
        created_at: (t.timestamp.max(0) as u64).saturating_mul(1000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trait_is_object_safe() {
        fn _assert_object_safe(_: &dyn TraceStore) {}
    }

    #[test]
    fn test_trace_type_default() {
        assert_eq!(TraceType::default(), TraceType::Thought);
    }

    #[test]
    fn test_trace_type_serialization() {
        let types = vec![
            TraceType::Thought,
            TraceType::Action,
            TraceType::Observation,
            TraceType::Tool,
            TraceType::Message,
            TraceType::Custom("my_type".to_string()),
        ];

        for t in types {
            let json = serde_json::to_string(&t).unwrap();
            let restored: TraceType = serde_json::from_str(&json).unwrap();
            assert_eq!(t, restored);
        }
    }
}
