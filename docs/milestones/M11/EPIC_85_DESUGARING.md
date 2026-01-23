# Epic 85: Facade-Substrate Desugaring

**Goal**: Verify and document that every facade operation desugars correctly to substrate

**Dependencies**: Epic 82, Epic 83

---

## Scope

- KV desugaring implementation and verification
- JSON/Event/Vector desugaring implementation
- State/History/Run desugaring implementation
- Desugaring verification tests

---

## User Stories

| Story | Description | Priority |
|-------|-------------|----------|
| #593 | KV Desugaring Implementation | CRITICAL |
| #594 | JSON/Event/Vector Desugaring Implementation | CRITICAL |
| #595 | State/History/Run Desugaring Implementation | HIGH |
| #596 | Desugaring Verification Tests | CRITICAL |

---

## Story #593: KV Desugaring Implementation

**File**: `crates/api/tests/desugaring_tests.rs` (NEW)

**Deliverable**: Verified KV desugaring

### Desugaring Table

| Facade | Substrate |
|--------|-----------|
| `set(key, value)` | `begin(); kv_put(default, key, value); commit()` |
| `get(key)` | `kv_get(default, key).map(\|v\| v.value)` |
| `getv(key)` | `kv_get(default, key)` |
| `mget(keys)` | `batch { kv_get(default, k) for k in keys }` |
| `mset(entries)` | `begin(); for (k,v): kv_put(default, k, v); commit()` |
| `delete(keys)` | `begin(); for k: kv_delete(default, k); commit()` — returns count existed |
| `exists(key)` | `kv_get(default, key).is_some()` |
| `exists_many(keys)` | `keys.filter(\|k\| kv_get(default, k).is_some()).count()` |
| `incr(key, delta)` | `kv_incr(default, key, delta)` — **atomic engine operation** |

### Implementation

```rust
#[cfg(test)]
mod kv_desugaring_tests {
    use super::*;

    /// Test: set(key, value) desugars correctly
    #[test]
    fn test_set_desugar() {
        let db = setup_test_db();
        let run = RunId::default_run();

        // Via facade
        db.facade().set("key", Value::Int(42)).unwrap();

        // Via substrate (same operations)
        let txn = db.substrate().begin(&run).unwrap();
        db.substrate().kv_put(&txn, "key2", Value::Int(42)).unwrap();
        db.substrate().commit(txn).unwrap();

        // Both should produce equivalent state
        let v1 = db.substrate().kv_get(&run, "key").unwrap().unwrap().value;
        let v2 = db.substrate().kv_get(&run, "key2").unwrap().unwrap().value;
        assert_eq!(v1, v2);
    }

    /// Test: get(key) desugars to kv_get().map(|v| v.value)
    #[test]
    fn test_get_desugar() {
        let db = setup_test_db();
        let run = RunId::default_run();

        db.facade().set("key", Value::Int(42)).unwrap();

        // Facade get
        let facade_result = db.facade().get("key").unwrap();

        // Substrate equivalent
        let substrate_result = db.substrate()
            .kv_get(&run, "key")
            .unwrap()
            .map(|v| v.value);

        assert_eq!(facade_result, substrate_result);
    }

    /// Test: getv(key) returns full Versioned<Value>
    #[test]
    fn test_getv_desugar() {
        let db = setup_test_db();
        let run = RunId::default_run();

        db.facade().set("key", Value::Int(42)).unwrap();

        let facade_result = db.facade().getv("key").unwrap().unwrap();
        let substrate_result = db.substrate().kv_get(&run, "key").unwrap().unwrap();

        assert_eq!(facade_result.value, substrate_result.value);
        assert_eq!(facade_result.version, substrate_result.version);
        assert_eq!(facade_result.timestamp, substrate_result.timestamp);
    }

    /// Test: delete returns count of existing keys
    #[test]
    fn test_delete_count() {
        let db = setup_test_db();

        db.facade().set("a", Value::Int(1)).unwrap();
        db.facade().set("b", Value::Int(2)).unwrap();
        // "c" doesn't exist

        let count = db.facade().delete(&["a", "b", "c"]).unwrap();
        assert_eq!(count, 2); // Only a and b existed
    }

    /// Test: incr is atomic engine operation
    #[test]
    fn test_incr_atomicity() {
        let db = setup_test_db();

        // Concurrent increments should not lose updates
        let handles: Vec<_> = (0..10)
            .map(|_| {
                let db = db.clone();
                std::thread::spawn(move || {
                    for _ in 0..100 {
                        db.facade().incr("counter", 1).unwrap();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let final_value = db.facade().get("counter").unwrap().unwrap();
        assert_eq!(final_value, Value::Int(1000)); // 10 threads * 100 increments
    }
}
```

### Acceptance Criteria

- [ ] `set` desugars to begin/kv_put/commit
- [ ] `get` desugars to kv_get with value extraction
- [ ] `getv` returns identical Versioned<Value>
- [ ] `mget` preserves order and returns None for missing
- [ ] `mset` is atomic (all-or-nothing)
- [ ] `delete` returns count of keys that existed
- [ ] `incr` is atomic (no lost updates)

---

## Story #594: JSON/Event/Vector Desugaring Implementation

**File**: `crates/api/tests/desugaring_tests.rs`

**Deliverable**: Verified JSON/Event/Vector desugaring

### Desugaring Tables

**JSON:**
| Facade | Substrate |
|--------|-----------|
| `json_set(key, path, value)` | `begin(); json_set(default, key, path, value); commit()` |
| `json_get(key, path)` | `json_get(default, key, path).map(\|v\| v.value)` |
| `json_getv(key, path)` | `json_get(default, key, path)` — **document-level version** |
| `json_del(key, path)` | `begin(); json_delete(default, key, path); commit()` |
| `json_merge(key, path, value)` | `begin(); json_merge(default, key, path, value); commit()` |

**Event:**
| Facade | Substrate |
|--------|-----------|
| `xadd(stream, payload)` | `event_append(default, stream, payload)` |
| `xrange(stream, start, end, limit)` | `event_range(default, stream, start, end, limit)` |
| `xlen(stream)` | `event_range(default, stream, None, None, None).len()` |

**Vector:**
| Facade | Substrate |
|--------|-----------|
| `vset(key, vector, metadata)` | `begin(); vector_set(default, key, vector, metadata); commit()` |
| `vget(key)` | `vector_get(default, key)` |
| `vdel(key)` | `begin(); vector_delete(default, key); commit()` |

### Implementation

```rust
#[cfg(test)]
mod json_desugaring_tests {
    use super::*;

    /// Test: json_getv returns document-level version
    #[test]
    fn test_json_getv_document_version() {
        let db = setup_test_db();

        // Set up document
        db.facade().json_set("doc", "$", json!({"a": 1})).unwrap();
        let v1 = db.facade().json_getv("doc", "$.a").unwrap().unwrap();

        // Modify document at different path
        db.facade().json_set("doc", "$.b", Value::Int(2)).unwrap();
        let v2 = db.facade().json_getv("doc", "$.a").unwrap().unwrap();

        // Version should have changed even though $.a wasn't modified
        assert!(v2.version.value() > v1.version.value());

        // Both paths return document version
        let va = db.facade().json_getv("doc", "$.a").unwrap().unwrap();
        let vb = db.facade().json_getv("doc", "$.b").unwrap().unwrap();
        assert_eq!(va.version, vb.version);
    }
}

#[cfg(test)]
mod event_desugaring_tests {
    use super::*;

    /// Test: xlen desugars to xrange().len()
    #[test]
    fn test_xlen_desugar() {
        let db = setup_test_db();

        for i in 0..5 {
            db.facade().xadd("stream", json!({"i": i})).unwrap();
        }

        let len_facade = db.facade().xlen("stream").unwrap();
        let len_range = db.facade().xrange("stream", None, None, None).unwrap().len();

        assert_eq!(len_facade, len_range as u64);
        assert_eq!(len_facade, 5);
    }
}

#[cfg(test)]
mod vector_desugaring_tests {
    use super::*;

    /// Test: vget returns Versioned
    #[test]
    fn test_vget_returns_versioned() {
        let db = setup_test_db();

        db.facade().vset("vec", vec![1.0, 2.0, 3.0], json!({})).unwrap();

        let result = db.facade().vget("vec").unwrap().unwrap();
        assert!(matches!(result.version, Version::Txn(_)));
        assert_eq!(result.value.vector, vec![1.0, 2.0, 3.0]);
    }
}
```

### Acceptance Criteria

- [ ] `json_getv` returns document-level version (not subpath)
- [ ] `xlen` equals `xrange().len()`
- [ ] `vget` returns Versioned<VectorEntry>
- [ ] All operations target default run

---

## Story #595: State/History/Run Desugaring Implementation

**File**: `crates/api/tests/desugaring_tests.rs`

**Deliverable**: Verified State/History/Run desugaring

### Desugaring Tables

**State/CAS:**
| Facade | Substrate |
|--------|-----------|
| `cas_set(key, expected, new)` | `state_cas(default, key, expected, new)` |
| `cas_get(key)` | `state_get(default, key).map(\|v\| v.value)` |

**History:**
| Facade | Substrate |
|--------|-----------|
| `history(key, limit, before)` | `kv_history(default, key, limit, before)` |
| `get_at(key, version)` | `kv_get_at(default, key, version)` |
| `latest_version(key)` | `kv_get(default, key).map(\|v\| v.version)` |

**Run:**
| Facade | Substrate |
|--------|-----------|
| `runs()` | `run_list()` |
| `use_run(run_id)` | Returns facade with `default = run_id` (client-side binding) |
| `capabilities()` | Returns system capabilities object |

### Implementation

```rust
#[cfg(test)]
mod state_desugaring_tests {
    use super::*;

    /// Test: cas_set create-if-not-exists
    #[test]
    fn test_cas_create_if_not_exists() {
        let db = setup_test_db();

        // expected = None means create-if-not-exists
        let success = db.facade().cas_set("key", None, Value::Int(1)).unwrap();
        assert!(success);

        // Should fail if key exists
        let success = db.facade().cas_set("key", None, Value::Int(2)).unwrap();
        assert!(!success);

        // Value should still be 1
        assert_eq!(db.facade().cas_get("key").unwrap(), Some(Value::Int(1)));
    }
}

#[cfg(test)]
mod history_desugaring_tests {
    use super::*;

    /// Test: history returns newest first
    #[test]
    fn test_history_ordering() {
        let db = setup_test_db();

        for i in 1..=5 {
            db.facade().set("key", Value::Int(i)).unwrap();
        }

        let history = db.facade().history("key", None, None).unwrap();
        assert_eq!(history.len(), 5);

        // Newest first
        assert_eq!(history[0].value, Value::Int(5));
        assert_eq!(history[4].value, Value::Int(1));
    }
}

#[cfg(test)]
mod run_desugaring_tests {
    use super::*;

    /// Test: use_run scopes operations
    #[test]
    fn test_use_run_scoping() {
        let db = setup_test_db();

        // Create a new run via substrate
        let run_id = db.substrate().run_create(Value::Null).unwrap();

        // Set value in default run
        db.facade().set("key", Value::Int(1)).unwrap();

        // Set different value in custom run
        let scoped = db.facade().use_run(&run_id.to_string()).unwrap();
        scoped.set("key", Value::Int(2)).unwrap();

        // Each run has its own value
        assert_eq!(db.facade().get("key").unwrap(), Some(Value::Int(1)));
        assert_eq!(scoped.get("key").unwrap(), Some(Value::Int(2)));
    }
}
```

### Acceptance Criteria

- [ ] `cas_set` with `None` is create-if-not-exists
- [ ] `history` returns newest first
- [ ] `use_run` scopes operations to specified run
- [ ] `use_run` returns NotFound for non-existent run

---

## Story #596: Desugaring Verification Tests

**File**: `crates/api/tests/desugaring_tests.rs`

**Deliverable**: Comprehensive desugaring test suite

### Implementation

```rust
/// Comprehensive desugaring verification
///
/// For each facade operation, verify:
/// 1. Same result as desugared substrate operations
/// 2. Same state changes
/// 3. Same error behavior
/// 4. No hidden semantics

#[cfg(test)]
mod comprehensive_desugaring_tests {
    use super::*;

    /// Macro to test facade/substrate equivalence
    macro_rules! test_desugar {
        ($name:ident, $facade:expr, $substrate:expr) => {
            #[test]
            fn $name() {
                let db = setup_test_db();
                let run = RunId::default_run();

                let facade_result = $facade(&db);
                let substrate_result = $substrate(&db, &run);

                assert_eq!(facade_result, substrate_result);
            }
        };
    }

    /// Test all KV operations for parity
    #[test]
    fn test_all_kv_parity() {
        let db = setup_test_db();
        let run = RunId::default_run();

        // Test each operation produces same result
        let operations = vec![
            ("set", || db.facade().set("k", Value::Int(1))),
            ("get", || db.facade().get("k")),
            ("getv", || db.facade().getv("k").map(|v| v.map(|x| x.value))),
            ("mget", || db.facade().mget(&["k", "missing"])),
            ("delete", || db.facade().delete(&["k"]).map(|_| ())),
            ("exists", || db.facade().exists("k").map(|_| ())),
        ];

        for (name, op) in operations {
            let result = op();
            assert!(result.is_ok(), "Operation {} failed: {:?}", name, result);
        }
    }

    /// Verify no hidden semantics in facade
    #[test]
    fn test_no_hidden_semantics() {
        let db = setup_test_db();
        let run = RunId::default_run();

        // Capture state before
        let before = db.substrate().kv_get(&run, "key").unwrap();

        // Facade operation
        db.facade().set("key", Value::Int(42)).unwrap();

        // Capture state after
        let after = db.substrate().kv_get(&run, "key").unwrap().unwrap();

        // Verify only the expected change occurred
        assert!(before.is_none());
        assert_eq!(after.value, Value::Int(42));

        // Verify through substrate directly
        let txn = db.substrate().begin(&run).unwrap();
        db.substrate().kv_put(&txn, "key2", Value::Int(42)).unwrap();
        db.substrate().commit(txn).unwrap();

        let key2 = db.substrate().kv_get(&run, "key2").unwrap().unwrap();

        // Both should have Txn version type
        assert!(matches!(after.version, Version::Txn(_)));
        assert!(matches!(key2.version, Version::Txn(_)));
    }

    /// Verify errors propagate unchanged
    #[test]
    fn test_error_propagation() {
        let db = setup_test_db();

        // Facade error
        db.facade().set("key", Value::String("not int".into())).unwrap();
        let facade_err = db.facade().incr("key", 1).unwrap_err();

        // Should be WrongType
        assert_eq!(facade_err.code(), "WrongType");

        // Same error from substrate
        let run = RunId::default_run();
        let substrate_err = db.substrate().kv_incr(&run, "key", 1).unwrap_err();
        assert_eq!(substrate_err.code(), "WrongType");
    }
}
```

### Acceptance Criteria

- [ ] Every facade operation produces same result as desugared substrate
- [ ] No hidden state changes
- [ ] Errors propagate unchanged
- [ ] All FAC invariants verified (FAC-1 through FAC-5)

---

## Testing

All tests are in `crates/api/tests/desugaring_tests.rs`.

The test suite must verify all FAC invariants:

| Invariant | Test Strategy |
|-----------|---------------|
| FAC-1 | Every facade operation maps to deterministic substrate operations | Desugaring unit tests |
| FAC-2 | Facade adds no semantic behavior beyond defaults | Parity tests facade vs substrate |
| FAC-3 | Facade never swallows substrate errors | Error propagation tests |
| FAC-4 | Facade does not reorder operations | Ordering verification tests |
| FAC-5 | All behavior traces to explicit substrate operation | Audit all code paths |

---

## Files Modified/Created

| File | Action |
|------|--------|
| `crates/api/tests/desugaring_tests.rs` | CREATE - Desugaring verification tests |
| `crates/api/src/facade/mod.rs` | MODIFY - Ensure desugaring is mechanical |
