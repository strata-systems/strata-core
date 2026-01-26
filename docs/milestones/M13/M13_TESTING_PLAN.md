# M13 Testing Plan

This document defines the testing strategy for M13 (Command Execution Layer), following the [Testing Methodology](../../testing/TESTING_METHODOLOGY.md).

**Guiding Principle**: Tests exist to find bugs, not inflate test counts.

---

## Critical Context

**After M13, strata-api is deleted.** The executor becomes the sole API surface for Strata. This means:

1. All existing `tests/substrate_api_comprehensive/` tests must be **ported** to use Commands
2. The executor tests are **the API tests** - not a separate translation layer
3. Test scope includes **durability, concurrency, invariants** - everything

```
BEFORE M13:                          AFTER M13:
┌─────────────────────┐              ┌─────────────────────┐
│  Python/MCP         │              │  Python/MCP         │
└─────────┬───────────┘              └─────────┬───────────┘
          │                                    │
          ▼                                    │
┌─────────────────────┐                        │
│  strata-api         │  ◄── DELETED           │
│  (Substrate/Facade) │                        │
└─────────┬───────────┘                        │
          │                                    ▼
          ▼                          ┌─────────────────────┐
┌─────────────────────┐              │  strata-executor    │
│  strata-engine      │              │  (Commands)         │
└─────────────────────┘              └─────────┬───────────┘
                                               │
                                               ▼
                                     ┌─────────────────────┐
                                     │  strata-engine      │
                                     └─────────────────────┘
```

---

## What We're Testing

The executor is the **complete API surface**. We test:

| Category | What's Tested | Source |
|----------|---------------|--------|
| **Correctness** | Commands produce correct results | Port from substrate tests |
| **Durability** | Data survives crash/recovery | Port from substrate tests |
| **Concurrency** | Thread-safe execution | Port from substrate tests |
| **Invariants** | Seven Invariants hold | Port from substrate tests |
| **Edge cases** | Boundaries, unicode, limits | Port from substrate tests |
| **Serialization** | Commands/Outputs survive JSON | NEW for M13 |
| **Error mapping** | Correct error types returned | NEW for M13 |

---

## Tests to Port from substrate_api_comprehensive

### Inventory of Existing Tests

```
tests/substrate_api_comprehensive/
├── kv/
│   ├── basic_ops.rs          → executor/kv/basic_ops.rs
│   ├── atomic_ops.rs         → executor/kv/atomic_ops.rs
│   ├── batch_ops.rs          → executor/kv/batch_ops.rs
│   ├── scan_ops.rs           → executor/kv/scan_ops.rs
│   ├── edge_cases.rs         → executor/kv/edge_cases.rs
│   ├── value_types.rs        → executor/kv/value_types.rs
│   ├── durability.rs         → executor/kv/durability.rs
│   ├── concurrency.rs        → executor/kv/concurrency.rs
│   ├── transactions.rs       → executor/kv/transactions.rs
│   └── recovery_invariants.rs→ executor/kv/recovery.rs
├── jsonstore/
│   ├── basic_ops.rs          → executor/json/basic_ops.rs
│   ├── path_ops.rs           → executor/json/path_ops.rs
│   ├── merge_ops.rs          → executor/json/merge_ops.rs
│   ├── history_ops.rs        → executor/json/history_ops.rs
│   ├── tier1_ops.rs          → executor/json/tier1_ops.rs
│   ├── tier2_ops.rs          → executor/json/tier2_ops.rs
│   ├── tier3_ops.rs          → executor/json/tier3_ops.rs
│   ├── edge_cases.rs         → executor/json/edge_cases.rs
│   ├── durability.rs         → executor/json/durability.rs
│   └── concurrency.rs        → executor/json/concurrency.rs
├── eventlog/
│   ├── basic_ops.rs          → executor/event/basic_ops.rs
│   ├── streams.rs            → executor/event/streams.rs
│   ├── invariants.rs         → executor/event/invariants.rs
│   ├── immutability.rs       → executor/event/immutability.rs
│   ├── edge_cases.rs         → executor/event/edge_cases.rs
│   ├── durability.rs         → executor/event/durability.rs
│   ├── concurrency.rs        → executor/event/concurrency.rs
│   └── recovery_invariants.rs→ executor/event/recovery.rs
├── statecell/
│   ├── basic_ops.rs          → executor/state/basic_ops.rs
│   ├── cas_ops.rs            → executor/state/cas_ops.rs
│   ├── transitions.rs        → executor/state/transitions.rs
│   ├── invariants.rs         → executor/state/invariants.rs
│   ├── edge_cases.rs         → executor/state/edge_cases.rs
│   ├── durability.rs         → executor/state/durability.rs
│   └── concurrency.rs        → executor/state/concurrency.rs
├── vectorstore/
│   ├── basic_ops.rs          → executor/vector/basic_ops.rs
│   ├── search.rs             → executor/vector/search.rs
│   ├── collections.rs        → executor/vector/collections.rs
│   ├── batch.rs              → executor/vector/batch.rs
│   ├── history.rs            → executor/vector/history.rs
│   ├── edge_cases.rs         → executor/vector/edge_cases.rs
│   ├── durability.rs         → executor/vector/durability.rs
│   └── concurrency.rs        → executor/vector/concurrency.rs
├── runindex/
│   ├── basic_ops.rs          → executor/run/basic_ops.rs
│   ├── lifecycle.rs          → executor/run/lifecycle.rs
│   ├── tags.rs               → executor/run/tags.rs
│   ├── hierarchy.rs          → executor/run/hierarchy.rs
│   ├── queries.rs            → executor/run/queries.rs
│   ├── retention.rs          → executor/run/retention.rs
│   ├── delete.rs             → executor/run/delete.rs
│   ├── invariants.rs         → executor/run/invariants.rs
│   ├── edge_cases.rs         → executor/run/edge_cases.rs
│   └── concurrency.rs        → executor/run/concurrency.rs
└── main.rs                   → executor/main.rs
```

### Porting Strategy

**Transform pattern**: Direct substrate call → Command execution

```rust
// BEFORE (substrate_api_comprehensive)
#[test]
fn test_kv_put_get_roundtrip() {
    let (_, substrate) = quick_setup();
    let run = ApiRunId::default();

    let version = substrate.kv_put(&run, "key", Value::Int(42)).unwrap();
    let result = substrate.kv_get(&run, "key").unwrap().unwrap();

    assert_eq!(result.value, Value::Int(42));
}

// AFTER (executor_comprehensive)
#[test]
fn test_kv_put_get_roundtrip() {
    let executor = quick_executor();
    let run = RunId::default();

    let put_result = executor.execute(Command::KvPut {
        run: run.clone(),
        key: "key".into(),
        value: Value::Int(42),
    }).unwrap();

    let get_result = executor.execute(Command::KvGet {
        run: run.clone(),
        key: "key".into(),
    }).unwrap();

    match get_result {
        Output::MaybeVersioned(Some(v)) => assert_eq!(v.value, Value::Int(42)),
        _ => panic!("Expected MaybeVersioned with value"),
    }
}
```

**Key differences**:
1. `substrate.method()` → `executor.execute(Command::Variant { ... })`
2. Direct return types → `Output` enum that must be matched
3. Same assertions on the actual values

---

## New Tests (M13-Specific)

### 1. Serialization Round-Trip Tests

Commands must survive JSON serialization for Python/MCP clients.

```rust
/// All command variants serialize and deserialize correctly
#[test]
fn test_all_commands_json_roundtrip() {
    let commands = generate_all_command_variants();

    for (name, cmd) in commands {
        let json = serde_json::to_string(&cmd)
            .expect(&format!("Failed to serialize {}", name));
        let restored: Command = serde_json::from_str(&json)
            .expect(&format!("Failed to deserialize {}", name));

        assert_eq!(cmd, restored, "Command {} failed round-trip", name);
    }
}

/// Special float values in commands survive round-trip
#[test]
fn test_special_floats_in_commands() {
    let test_cases = vec![
        ("infinity", f64::INFINITY),
        ("neg_infinity", f64::NEG_INFINITY),
        ("neg_zero", -0.0_f64),
    ];

    for (name, float) in test_cases {
        let cmd = Command::KvPut {
            run: RunId::default(),
            key: name.into(),
            value: Value::Float(float),
        };

        let json = serde_json::to_string(&cmd).unwrap();
        let restored: Command = serde_json::from_str(&json).unwrap();

        if let Command::KvPut { value: Value::Float(f), .. } = restored {
            if float.is_infinite() {
                assert!(f.is_infinite() && f.signum() == float.signum());
            } else if float == -0.0 {
                assert!(f == 0.0 && f.is_sign_negative());
            }
        } else {
            panic!("Wrong command type after round-trip");
        }
    }
}

/// Binary data in commands survives round-trip
#[test]
fn test_bytes_in_commands_roundtrip() {
    let test_bytes = vec![
        vec![],
        vec![0x00, 0xFF],
        (0..=255).collect::<Vec<u8>>(),
    ];

    for bytes in test_bytes {
        let cmd = Command::KvPut {
            run: RunId::default(),
            key: "bytes".into(),
            value: Value::Bytes(bytes.clone()),
        };

        let json = serde_json::to_string(&cmd).unwrap();
        let restored: Command = serde_json::from_str(&json).unwrap();

        if let Command::KvPut { value: Value::Bytes(b), .. } = restored {
            assert_eq!(b, bytes);
        } else {
            panic!("Bytes lost in round-trip");
        }
    }
}
```

### 2. Output Serialization Tests

Outputs must also survive JSON for client responses.

```rust
/// All output variants serialize correctly
#[test]
fn test_all_outputs_json_roundtrip() {
    let outputs = generate_all_output_variants();

    for (name, output) in outputs {
        let json = serde_json::to_string(&output)
            .expect(&format!("Failed to serialize output {}", name));
        let restored: Output = serde_json::from_str(&json)
            .expect(&format!("Failed to deserialize output {}", name));

        assert_eq!(output, restored, "Output {} failed round-trip", name);
    }
}
```

### 3. Error Serialization Tests

Errors must serialize with structured details preserved.

```rust
/// Error details survive serialization
#[test]
fn test_error_details_preserved() {
    let errors = vec![
        Error::KeyNotFound { key: "missing_key".into() },
        Error::RunNotFound { run: "missing_run".into() },
        Error::DimensionMismatch { expected: 128, actual: 256 },
        Error::VersionConflict { expected: 5, actual: 7 },
    ];

    for err in errors {
        let json = serde_json::to_string(&err).unwrap();
        let restored: Error = serde_json::from_str(&json).unwrap();

        // Verify structured fields preserved
        match (&err, &restored) {
            (Error::KeyNotFound { key: k1 }, Error::KeyNotFound { key: k2 }) => {
                assert_eq!(k1, k2);
            }
            (Error::DimensionMismatch { expected: e1, actual: a1 },
             Error::DimensionMismatch { expected: e2, actual: a2 }) => {
                assert_eq!(e1, e2);
                assert_eq!(a1, a2);
            }
            _ => assert_eq!(err, restored),
        }
    }
}
```

### 4. execute_many Tests

Batch execution semantics.

```rust
/// execute_many preserves order
#[test]
fn test_execute_many_order() {
    let executor = quick_executor();
    let run = RunId::default();

    let commands = vec![
        Command::KvPut { run: run.clone(), key: "k".into(), value: Value::Int(1) },
        Command::KvPut { run: run.clone(), key: "k".into(), value: Value::Int(2) },
        Command::KvPut { run: run.clone(), key: "k".into(), value: Value::Int(3) },
    ];

    executor.execute_many(commands);

    // Final value should be 3
    let result = executor.execute(Command::KvGet {
        run: run.clone(),
        key: "k".into(),
    }).unwrap();

    match result {
        Output::MaybeVersioned(Some(v)) => assert_eq!(v.value, Value::Int(3)),
        _ => panic!("Expected value 3"),
    }
}

/// execute_many returns results in same order as commands
#[test]
fn test_execute_many_results_order() {
    let executor = quick_executor();
    let run = RunId::default();

    // Setup
    executor.execute(Command::KvPut {
        run: run.clone(),
        key: "exists".into(),
        value: Value::Int(1),
    }).unwrap();

    let commands = vec![
        Command::KvGet { run: run.clone(), key: "exists".into() },    // Should succeed
        Command::KvGet { run: run.clone(), key: "missing".into() },   // Should return None
        Command::KvExists { run: run.clone(), key: "exists".into() }, // Should return true
    ];

    let results = executor.execute_many(commands);

    assert_eq!(results.len(), 3);
    assert!(matches!(results[0], Ok(Output::MaybeVersioned(Some(_)))));
    assert!(matches!(results[1], Ok(Output::MaybeVersioned(None))));
    assert!(matches!(results[2], Ok(Output::Bool(true))));
}
```

---

## Test File Structure

```
tests/executor_comprehensive/
├── main.rs                      # Test harness, shared utilities
├── test_utils.rs                # quick_executor(), helpers
├── testdata/
│   ├── kv_test_data.jsonl       # Copied from substrate tests
│   ├── edge_cases.jsonl
│   └── serialization_cases.jsonl
│
├── kv/
│   ├── mod.rs
│   ├── basic_ops.rs             # Ported from substrate
│   ├── atomic_ops.rs            # Ported
│   ├── batch_ops.rs             # Ported
│   ├── scan_ops.rs              # Ported
│   ├── edge_cases.rs            # Ported
│   ├── value_types.rs           # Ported
│   ├── durability.rs            # Ported
│   ├── concurrency.rs           # Ported
│   └── transactions.rs          # Ported
│
├── json/                        # Ported from substrate jsonstore/
├── event/                       # Ported from substrate eventlog/
├── state/                       # Ported from substrate statecell/
├── vector/                      # Ported from substrate vectorstore/
├── run/                         # Ported from substrate runindex/
│
├── transaction/                 # NEW - TransactionControl commands
│   ├── basic_ops.rs
│   └── savepoints.rs            # If implemented
│
├── retention/                   # NEW - RetentionSubstrate commands
│   └── basic_ops.rs
│
├── serialization/               # NEW - M13 specific
│   ├── command_roundtrip.rs
│   ├── output_roundtrip.rs
│   ├── error_roundtrip.rs
│   ├── special_values.rs
│   └── bytes_encoding.rs
│
└── batch/                       # NEW - execute_many
    ├── order.rs
    └── error_handling.rs
```

---

## What NOT to Test (Anti-Patterns)

Following [TESTING_METHODOLOGY.md](../../testing/TESTING_METHODOLOGY.md):

### Compiler-Verified Properties
```rust
// DON'T
#[test]
fn test_command_is_clone() { ... }

#[test]
fn test_executor_is_send_sync() { ... }
```

### Shallow Assertions
```rust
// DON'T
#[test]
fn test_kv_put_succeeds() {
    let result = executor.execute(Command::KvPut { ... });
    assert!(result.is_ok());  // Doesn't verify the version
}

// DO
#[test]
fn test_kv_put_returns_version() {
    let result = executor.execute(Command::KvPut { ... }).unwrap();
    match result {
        Output::Version(v) => assert!(v > 0),
        _ => panic!("Expected Version output"),
    }
}
```

### Implementation Details
```rust
// DON'T - tests internal dispatch mechanism
#[test]
fn test_kv_handler_called() {
    // Spy on internal handler...
}

// DO - tests observable behavior
#[test]
fn test_kv_put_stores_value() {
    executor.execute(Command::KvPut { key: "k", value: 42 });
    let result = executor.execute(Command::KvGet { key: "k" });
    // Verify value is stored
}
```

---

## Test Counts (Estimated)

| Category | Tests | Source |
|----------|-------|--------|
| KV | ~80 | Ported from substrate |
| JSON | ~60 | Ported from substrate |
| Event | ~50 | Ported from substrate |
| State | ~40 | Ported from substrate |
| Vector | ~50 | Ported from substrate |
| Run | ~60 | Ported from substrate |
| Transaction | ~15 | New |
| Retention | ~10 | New |
| Serialization | ~30 | New (M13-specific) |
| Batch (execute_many) | ~10 | New |
| **Total** | **~405** | |

---

## Porting Checklist

For each test file in `substrate_api_comprehensive/`:

- [ ] Create corresponding file in `executor_comprehensive/`
- [ ] Transform `substrate.method()` → `executor.execute(Command::...)`
- [ ] Transform return type assertions → `Output` enum matching
- [ ] Verify test still catches the same bug
- [ ] Remove any tests that become compiler-verified (unlikely)
- [ ] Keep test data files (`.jsonl`)

---

## Success Criteria

M13 testing is complete when:

1. **All substrate tests ported** - Same behavioral coverage through Commands
2. **Serialization tests pass** - All Commands/Outputs/Errors survive JSON
3. **Durability tests pass** - Commands + crash + recovery works
4. **Concurrency tests pass** - Thread-safe command execution
5. **No regressions** - Same bugs caught as substrate tests
6. **Tests run in < 60 seconds** - Fast feedback (in-memory mode)

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-01-25 | Initial M13 testing plan |
| 1.1 | 2026-01-25 | Revised: executor replaces strata-api, port all tests |
