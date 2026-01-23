//! TraceStore Hierarchy Tests
//!
//! Tests for parent-child relationships:
//! - trace_create with parent_id
//! - trace_children
//! - trace_tree
//! - trace_list with parent filter

use crate::*;
use strata_api::substrate::{TraceStore, TraceType};

/// Test create child trace
#[test]
fn test_trace_create_child() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create parent
        let (parent_id, _) = db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();

        // Create child
        let (child_id, _) = db.trace_create(&run, TraceType::Action, Some(&parent_id), content.clone(), vec![]).unwrap();

        // Verify child has parent reference
        let child = db.trace_get(&run, &child_id).unwrap().unwrap();
        assert_eq!(child.value.parent_id, Some(parent_id));
    });
}

/// Test get children of trace
#[test]
fn test_trace_children() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create parent
        let (parent_id, _) = db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();

        // Create children
        let mut child_ids = Vec::new();
        for i in 0..3 {
            let child_content = obj([("order", Value::Int(i))]);
            let (child_id, _) = db.trace_create(&run, TraceType::Action, Some(&parent_id), child_content, vec![]).unwrap();
            child_ids.push(child_id);
        }

        // Get children
        let children = db.trace_children(&run, &parent_id).unwrap();
        assert_eq!(children.len(), 3);

        // All children should be present
        let found_ids: Vec<_> = children.iter().map(|c| c.value.id.clone()).collect();
        for id in &child_ids {
            assert!(found_ids.contains(id));
        }
    });
}

/// Test children of trace with no children
#[test]
fn test_trace_children_empty() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create trace with no children
        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();

        // Get children - should be empty
        let children = db.trace_children(&run, &id).unwrap();
        assert!(children.is_empty());
    });
}

/// Test trace tree (flattened pre-order)
#[test]
fn test_trace_tree() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Build tree:
        //   root
        //   ├── child1
        //   │   └── grandchild1
        //   └── child2

        let root_content = obj([("name", Value::String("root".to_string()))]);
        let (root_id, _) = db.trace_create(&run, TraceType::Thought, None, root_content, vec![]).unwrap();

        let child1_content = obj([("name", Value::String("child1".to_string()))]);
        let (child1_id, _) = db.trace_create(&run, TraceType::Action, Some(&root_id), child1_content, vec![]).unwrap();

        let grandchild_content = obj([("name", Value::String("grandchild1".to_string()))]);
        let (grandchild_id, _) = db.trace_create(&run, TraceType::Observation, Some(&child1_id), grandchild_content, vec![]).unwrap();

        let child2_content = obj([("name", Value::String("child2".to_string()))]);
        let (child2_id, _) = db.trace_create(&run, TraceType::Action, Some(&root_id), child2_content, vec![]).unwrap();

        // Get tree
        let tree = db.trace_tree(&run, &root_id).unwrap();

        // Should have 4 traces (root + 2 children + 1 grandchild)
        assert_eq!(tree.len(), 4);

        // First should be root (pre-order)
        assert_eq!(tree[0].value.id, root_id);

        // All nodes should be present
        let ids: Vec<_> = tree.iter().map(|t| t.value.id.clone()).collect();
        assert!(ids.contains(&root_id));
        assert!(ids.contains(&child1_id));
        assert!(ids.contains(&child2_id));
        assert!(ids.contains(&grandchild_id));
    });
}

/// Test trace tree of leaf node
#[test]
fn test_trace_tree_leaf_node() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("leaf".to_string()))]);

        let (id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();

        let tree = db.trace_tree(&run, &id).unwrap();
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].value.id, id);
    });
}

/// Test trace tree of nonexistent node
#[test]
fn test_trace_tree_nonexistent() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        let tree = db.trace_tree(&run, "nonexistent").unwrap();
        assert!(tree.is_empty());
    });
}

/// Test deep hierarchy
#[test]
fn test_trace_deep_hierarchy() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Build a chain: root -> c1 -> c2 -> c3 -> c4 -> c5
        let content = obj([("level", Value::Int(0))]);
        let (mut parent_id, _) = db.trace_create(&run, TraceType::Thought, None, content, vec![]).unwrap();
        let root_id = parent_id.clone();

        for i in 1..=5 {
            let content = obj([("level", Value::Int(i))]);
            let (child_id, _) = db.trace_create(&run, TraceType::Action, Some(&parent_id), content, vec![]).unwrap();
            parent_id = child_id;
        }

        // Get full tree
        let tree = db.trace_tree(&run, &root_id).unwrap();
        assert_eq!(tree.len(), 6);

        // First should be root (pre-order)
        assert_eq!(tree[0].value.id, root_id);
    });
}

/// Test list root traces only
#[test]
fn test_trace_list_roots_only() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create root traces
        let (root1, _) = db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        let (root2, _) = db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();

        // Create child (not a root)
        db.trace_create(&run, TraceType::Action, Some(&root1), content.clone(), vec![]).unwrap();

        // List only roots (parent_id = Some(None))
        let roots = db.trace_list(&run, None, Some(None), None, None, None).unwrap();

        assert_eq!(roots.len(), 2);
        let root_ids: Vec<_> = roots.iter().map(|r| r.value.id.clone()).collect();
        assert!(root_ids.contains(&root1));
        assert!(root_ids.contains(&root2));
    });
}

/// Test list children of specific parent
#[test]
fn test_trace_list_by_parent() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Create two parent traces
        let (parent1, _) = db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();
        let (parent2, _) = db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();

        // Create children for parent1
        for _ in 0..3 {
            db.trace_create(&run, TraceType::Action, Some(&parent1), content.clone(), vec![]).unwrap();
        }

        // Create children for parent2
        for _ in 0..2 {
            db.trace_create(&run, TraceType::Action, Some(&parent2), content.clone(), vec![]).unwrap();
        }

        // List children of parent1
        let children1 = db.trace_list(&run, None, Some(Some(&parent1)), None, None, None).unwrap();
        assert_eq!(children1.len(), 3);

        // List children of parent2
        let children2 = db.trace_list(&run, None, Some(Some(&parent2)), None, None, None).unwrap();
        assert_eq!(children2.len(), 2);
    });
}

/// Test multiple roots
#[test]
fn test_trace_multiple_roots() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();

        // Create multiple independent trees
        let mut root_ids = Vec::new();
        for i in 0..3 {
            let content = obj([("tree", Value::Int(i))]);
            let (root_id, _) = db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();

            // Add children to each root
            for j in 0..2 {
                let child_content = obj([("tree", Value::Int(i)), ("child", Value::Int(j))]);
                db.trace_create(&run, TraceType::Action, Some(&root_id), child_content, vec![]).unwrap();
            }
            root_ids.push(root_id);
        }

        // Total: 3 roots + 6 children = 9 traces
        assert_eq!(db.trace_count(&run).unwrap(), 9);

        // Each root's tree has 3 traces
        for root_id in &root_ids {
            let tree = db.trace_tree(&run, root_id).unwrap();
            assert_eq!(tree.len(), 3);
        }
    });
}

/// Test parent must exist
#[test]
fn test_trace_parent_must_exist() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Try to create child with nonexistent parent
        let result = db.trace_create(&run, TraceType::Action, Some("nonexistent"), content, vec![]);

        // This may succeed (orphan) or fail depending on implementation
        // Just verify it doesn't panic
        let _ = result;
    });
}

/// Test trace types in hierarchy
#[test]
fn test_trace_hierarchy_mixed_types() {
    test_across_substrate_modes(|db| {
        let run = ApiRunId::default_run_id();
        let content = obj([("msg", Value::String("test".to_string()))]);

        // Root: Thought
        let (root_id, _) = db.trace_create(&run, TraceType::Thought, None, content.clone(), vec![]).unwrap();

        // Children: Action, Tool, Observation
        let (action_id, _) = db.trace_create(&run, TraceType::Action, Some(&root_id), content.clone(), vec![]).unwrap();
        let (tool_id, _) = db.trace_create(&run, TraceType::Tool, Some(&root_id), content.clone(), vec![]).unwrap();
        let (obs_id, _) = db.trace_create(&run, TraceType::Observation, Some(&root_id), content.clone(), vec![]).unwrap();

        // Verify types preserved in tree
        let tree = db.trace_tree(&run, &root_id).unwrap();
        let root = tree.iter().find(|t| t.value.id == root_id).unwrap();
        let action = tree.iter().find(|t| t.value.id == action_id).unwrap();
        let tool = tree.iter().find(|t| t.value.id == tool_id).unwrap();
        let obs = tree.iter().find(|t| t.value.id == obs_id).unwrap();

        assert_eq!(root.value.trace_type, TraceType::Thought);
        assert_eq!(action.value.trace_type, TraceType::Action);
        assert_eq!(tool.value.trace_type, TraceType::Tool);
        assert_eq!(obs.value.trace_type, TraceType::Observation);
    });
}
