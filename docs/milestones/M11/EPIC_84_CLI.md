# Epic 84: CLI Implementation

**Goal**: Implement Redis-like CLI with frozen parsing rules

**Dependencies**: M11a complete

---

## Scope

- CLI argument parser with frozen parsing rules
- KV commands (set, get, mget, mset, delete, exists, incr)
- JSON commands (json.set, json.get, json.del, json.merge)
- Event commands (xadd, xrange, xlen)
- Vector commands (vset, vget, vdel)
- State commands (cas.set, cas.get)
- History and run commands
- Output formatting and exit codes

---

## User Stories

| Story | Description | Priority |
|-------|-------------|----------|
| #585 | CLI Argument Parser | FOUNDATION |
| #586 | KV Commands | CRITICAL |
| #587 | JSON Commands | CRITICAL |
| #588 | Event Commands | HIGH |
| #589 | Vector Commands | HIGH |
| #590 | State Commands | HIGH |
| #591 | History and Run Commands | HIGH |
| #592 | Output Formatting and Exit Codes | CRITICAL |

---

## Story #585: CLI Argument Parser

**File**: `crates/cli/src/parser.rs` (NEW)

**Deliverable**: Argument parsing with frozen rules

### Design

The CLI uses Redis-like ergonomics with deterministic parsing:

```
strata <command> [args...]
strata --run=<run_id> <command> [args...]
```

### Implementation

```rust
use crate::value::Value;

/// Parse a CLI argument into a Value
///
/// Parsing rules (FROZEN):
/// - `123` → Int
/// - `-456` → Int
/// - `1.23` → Float
/// - `-1.23` → Float
/// - `"hello"` → String (quotes stripped)
/// - `hello` → String (bare word)
/// - `true`/`false` → Bool
/// - `null` → Null
/// - `{...}` → Object (must be valid JSON)
/// - `[...]` → Array (must be valid JSON)
/// - `b64:SGVsbG8=` → Bytes (base64 decoded)
pub fn parse_value(arg: &str) -> Result<Value, ParseError> {
    // Check for explicit bytes prefix
    if let Some(b64) = arg.strip_prefix("b64:") {
        let bytes = base64::decode(b64)
            .map_err(|e| ParseError::InvalidBase64(e.to_string()))?;
        return Ok(Value::Bytes(bytes));
    }

    // Check for JSON object or array
    if (arg.starts_with('{') && arg.ends_with('}'))
        || (arg.starts_with('[') && arg.ends_with(']'))
    {
        let json: serde_json::Value = serde_json::from_str(arg)
            .map_err(|e| ParseError::InvalidJson(e.to_string()))?;
        return json_to_value(&json);
    }

    // Check for quoted string
    if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
        return Ok(Value::String(arg[1..arg.len()-1].to_string()));
    }

    // Check for keywords
    match arg {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        "null" => return Ok(Value::Null),
        _ => {}
    }

    // Try parsing as integer
    if let Ok(i) = arg.parse::<i64>() {
        return Ok(Value::Int(i));
    }

    // Try parsing as float
    if let Ok(f) = arg.parse::<f64>() {
        return Ok(Value::Float(f));
    }

    // Default: bare word = string
    Ok(Value::String(arg.to_string()))
}

fn json_to_value(json: &serde_json::Value) -> Result<Value, ParseError> {
    match json {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err(ParseError::InvalidNumber)
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(s.clone())),
        serde_json::Value::Array(arr) => {
            let values: Result<Vec<_>, _> = arr.iter().map(json_to_value).collect();
            Ok(Value::Array(values?))
        }
        serde_json::Value::Object(obj) => {
            // Check for special wrappers
            if obj.len() == 1 {
                if let Some(serde_json::Value::String(b64)) = obj.get("$bytes") {
                    let bytes = base64::decode(b64)
                        .map_err(|e| ParseError::InvalidBase64(e.to_string()))?;
                    return Ok(Value::Bytes(bytes));
                }
            }

            let mut map = std::collections::HashMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), json_to_value(v)?);
            }
            Ok(Value::Object(map))
        }
    }
}

#[derive(Debug)]
pub enum ParseError {
    InvalidBase64(String),
    InvalidJson(String),
    InvalidNumber,
    UnknownCommand(String),
    MissingArgument(String),
    TooManyArguments,
}

/// CLI command structure
#[derive(Debug)]
pub struct CliCommand {
    pub run_id: Option<String>,
    pub command: String,
    pub args: Vec<String>,
}

/// Parse command line arguments
pub fn parse_args(args: &[String]) -> Result<CliCommand, ParseError> {
    let mut run_id = None;
    let mut command_start = 0;

    // Check for --run option
    for (i, arg) in args.iter().enumerate() {
        if let Some(rid) = arg.strip_prefix("--run=") {
            run_id = Some(rid.to_string());
            command_start = i + 1;
            break;
        }
    }

    if args.len() <= command_start {
        return Err(ParseError::MissingArgument("command".into()));
    }

    Ok(CliCommand {
        run_id,
        command: args[command_start].clone(),
        args: args[command_start + 1..].to_vec(),
    })
}
```

### Acceptance Criteria

- [ ] `123` → Int
- [ ] `-456` → Int
- [ ] `1.23` → Float
- [ ] `-1.23` → Float
- [ ] `"hello"` → String (quotes stripped)
- [ ] `hello` → String (bare word)
- [ ] `true`/`false` → Bool
- [ ] `null` → Null
- [ ] `{...}` → Object (must be valid JSON)
- [ ] `[...]` → Array (must be valid JSON)
- [ ] `b64:SGVsbG8=` → Bytes (base64 decoded)
- [ ] `--run=<run_id>` option supported

---

## Story #586: KV Commands

**File**: `crates/cli/src/commands/kv.rs` (NEW)

**Deliverable**: All KV CLI commands

### Implementation

```rust
use crate::output::{Output, format_value, format_nil, format_integer};
use crate::parser::parse_value;

pub fn cmd_set(facade: &impl KvFacade, args: &[String]) -> Result<Output, CliError> {
    if args.len() < 2 {
        return Err(CliError::usage("set <key> <value>"));
    }
    let key = &args[0];
    let value = parse_value(&args[1])?;
    facade.set(key, value)?;
    Ok(Output::Ok)
}

pub fn cmd_get(facade: &impl KvFacade, args: &[String]) -> Result<Output, CliError> {
    if args.is_empty() {
        return Err(CliError::usage("get <key>"));
    }
    match facade.get(&args[0])? {
        Some(v) => Ok(Output::Value(v)),
        None => Ok(Output::Nil),
    }
}

pub fn cmd_mget(facade: &impl KvFacade, args: &[String]) -> Result<Output, CliError> {
    if args.is_empty() {
        return Err(CliError::usage("mget <key> [key ...]"));
    }
    let keys: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let values = facade.mget(&keys)?;
    Ok(Output::Array(values))
}

pub fn cmd_mset(facade: &impl KvFacade, args: &[String]) -> Result<Output, CliError> {
    if args.len() < 2 || args.len() % 2 != 0 {
        return Err(CliError::usage("mset <key> <value> [key value ...]"));
    }
    let mut entries = Vec::new();
    for pair in args.chunks(2) {
        let key = &pair[0];
        let value = parse_value(&pair[1])?;
        entries.push((key.as_str(), value));
    }
    facade.mset(&entries)?;
    Ok(Output::Ok)
}

pub fn cmd_delete(facade: &impl KvFacade, args: &[String]) -> Result<Output, CliError> {
    if args.is_empty() {
        return Err(CliError::usage("delete <key> [key ...]"));
    }
    let keys: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let count = facade.delete(&keys)?;
    Ok(Output::Integer(count as i64))
}

pub fn cmd_exists(facade: &impl KvFacade, args: &[String]) -> Result<Output, CliError> {
    if args.is_empty() {
        return Err(CliError::usage("exists <key>"));
    }
    let exists = facade.exists(&args[0])?;
    Ok(Output::Integer(if exists { 1 } else { 0 }))
}

pub fn cmd_incr(facade: &impl KvFacade, args: &[String]) -> Result<Output, CliError> {
    if args.is_empty() {
        return Err(CliError::usage("incr <key> [delta]"));
    }
    let delta = if args.len() > 1 {
        args[1].parse::<i64>().map_err(|_| CliError::usage("delta must be integer"))?
    } else {
        1
    };
    let result = facade.incr(&args[0], delta)?;
    Ok(Output::Integer(result))
}
```

### Acceptance Criteria

- [ ] `strata set x 123` works
- [ ] `strata get x` prints value or `(nil)`
- [ ] `strata mget a b c` prints array
- [ ] `strata mset a 1 b 2 c 3` atomic multi-set
- [ ] `strata delete x y` prints count
- [ ] `strata exists x` prints `(integer) 1` or `0`
- [ ] `strata incr counter` prints new value

---

## Story #587: JSON Commands

**File**: `crates/cli/src/commands/json.rs` (NEW)

**Deliverable**: All JSON CLI commands

### Implementation

```rust
pub fn cmd_json_set(facade: &impl JsonFacade, args: &[String]) -> Result<Output, CliError> {
    if args.len() < 3 {
        return Err(CliError::usage("json.set <key> <path> <value>"));
    }
    let key = &args[0];
    let path = &args[1];
    let value = parse_value(&args[2])?;
    facade.json_set(key, path, value)?;
    Ok(Output::Ok)
}

pub fn cmd_json_get(facade: &impl JsonFacade, args: &[String]) -> Result<Output, CliError> {
    if args.len() < 2 {
        return Err(CliError::usage("json.get <key> <path>"));
    }
    match facade.json_get(&args[0], &args[1])? {
        Some(v) => Ok(Output::Value(v)),
        None => Ok(Output::Nil),
    }
}

pub fn cmd_json_del(facade: &impl JsonFacade, args: &[String]) -> Result<Output, CliError> {
    if args.len() < 2 {
        return Err(CliError::usage("json.del <key> <path>"));
    }
    let count = facade.json_del(&args[0], &args[1])?;
    Ok(Output::Integer(count as i64))
}

pub fn cmd_json_merge(facade: &impl JsonFacade, args: &[String]) -> Result<Output, CliError> {
    if args.len() < 3 {
        return Err(CliError::usage("json.merge <key> <path> <value>"));
    }
    let value = parse_value(&args[2])?;
    facade.json_merge(&args[0], &args[1], value)?;
    Ok(Output::Ok)
}
```

### Acceptance Criteria

- [ ] `strata json.set doc $.name "Alice"` works
- [ ] `strata json.get doc $.name` prints value
- [ ] `strata json.del doc $.temp` prints count
- [ ] `strata json.merge doc $ '{"age": 30}'` works

---

## Story #588: Event Commands

**File**: `crates/cli/src/commands/event.rs` (NEW)

**Deliverable**: All Event CLI commands

### Implementation

```rust
pub fn cmd_xadd(facade: &impl EventFacade, args: &[String]) -> Result<Output, CliError> {
    if args.len() < 2 {
        return Err(CliError::usage("xadd <stream> <payload>"));
    }
    let payload = parse_value(&args[1])?;
    let version = facade.xadd(&args[0], payload)?;
    Ok(Output::Version(version))
}

pub fn cmd_xrange(facade: &impl EventFacade, args: &[String]) -> Result<Output, CliError> {
    if args.is_empty() {
        return Err(CliError::usage("xrange <stream> [--limit N]"));
    }
    // Parse optional limit
    let limit = parse_limit_option(args);
    let events = facade.xrange(&args[0], None, None, limit)?;
    Ok(Output::VersionedArray(events))
}

pub fn cmd_xlen(facade: &impl EventFacade, args: &[String]) -> Result<Output, CliError> {
    if args.is_empty() {
        return Err(CliError::usage("xlen <stream>"));
    }
    let count = facade.xlen(&args[0])?;
    Ok(Output::Integer(count as i64))
}
```

### Acceptance Criteria

- [ ] `strata xadd stream '{"type":"login"}'` prints version
- [ ] `strata xrange stream` prints events
- [ ] `strata xlen stream` prints count

---

## Story #589: Vector Commands

**File**: `crates/cli/src/commands/vector.rs` (NEW)

**Deliverable**: All Vector CLI commands

### Implementation

```rust
pub fn cmd_vset(facade: &impl VectorFacade, args: &[String]) -> Result<Output, CliError> {
    if args.len() < 3 {
        return Err(CliError::usage("vset <key> <vector> <metadata>"));
    }
    let vector = parse_vector(&args[1])?;
    let metadata = parse_value(&args[2])?;
    facade.vset(&args[0], vector, metadata)?;
    Ok(Output::Ok)
}

pub fn cmd_vget(facade: &impl VectorFacade, args: &[String]) -> Result<Output, CliError> {
    if args.is_empty() {
        return Err(CliError::usage("vget <key>"));
    }
    match facade.vget(&args[0])? {
        Some(v) => Ok(Output::VectorEntry(v)),
        None => Ok(Output::Nil),
    }
}

pub fn cmd_vdel(facade: &impl VectorFacade, args: &[String]) -> Result<Output, CliError> {
    if args.is_empty() {
        return Err(CliError::usage("vdel <key>"));
    }
    let deleted = facade.vdel(&args[0])?;
    Ok(Output::Integer(if deleted { 1 } else { 0 }))
}

fn parse_vector(arg: &str) -> Result<Vec<f32>, CliError> {
    let json: Vec<f64> = serde_json::from_str(arg)
        .map_err(|e| CliError::parse(format!("invalid vector: {}", e)))?;
    Ok(json.into_iter().map(|f| f as f32).collect())
}
```

### Acceptance Criteria

- [ ] `strata vset doc1 "[0.1, 0.2, 0.3]" '{"tag":"test"}'` works
- [ ] `strata vget doc1` prints vector entry
- [ ] `strata vdel doc1` prints 1 or 0

---

## Story #590: State Commands

**File**: `crates/cli/src/commands/state.rs` (NEW)

**Deliverable**: CAS CLI commands

### Implementation

```rust
pub fn cmd_cas_set(facade: &impl StateFacade, args: &[String]) -> Result<Output, CliError> {
    if args.len() < 3 {
        return Err(CliError::usage("cas.set <key> <expected> <new>"));
    }
    let expected = parse_cas_expected(&args[1])?;
    let new = parse_value(&args[2])?;
    let success = facade.cas_set(&args[0], expected, new)?;
    Ok(Output::Integer(if success { 1 } else { 0 }))
}

pub fn cmd_cas_get(facade: &impl StateFacade, args: &[String]) -> Result<Output, CliError> {
    if args.is_empty() {
        return Err(CliError::usage("cas.get <key>"));
    }
    match facade.cas_get(&args[0])? {
        Some(v) => Ok(Output::Value(v)),
        None => Ok(Output::Nil),
    }
}

fn parse_cas_expected(arg: &str) -> Result<Option<Value>, CliError> {
    // "null" as string means "expect key to be missing"
    // Actual null value requires explicit Value::Null
    if arg == "null" {
        Ok(None) // Create-if-not-exists
    } else {
        Ok(Some(parse_value(arg)?))
    }
}
```

### Acceptance Criteria

- [ ] `strata cas.set mykey null 123` creates if not exists
- [ ] `strata cas.get mykey` returns value
- [ ] `strata cas.set mykey 123 456` updates if matches
- [ ] `strata cas.set mykey 999 0` fails if mismatch

---

## Story #591: History and Run Commands

**File**: `crates/cli/src/commands/history.rs` and `crates/cli/src/commands/run.rs` (NEW)

**Deliverable**: History and run CLI commands

### Implementation

```rust
// history.rs
pub fn cmd_history(facade: &impl HistoryFacade, args: &[String]) -> Result<Output, CliError> {
    if args.is_empty() {
        return Err(CliError::usage("history <key> [--limit N]"));
    }
    let limit = parse_limit_option(args);
    let history = facade.history(&args[0], limit, None)?;
    Ok(Output::VersionedArray(history))
}

// run.rs
pub fn cmd_runs(facade: &impl RunFacade, _args: &[String]) -> Result<Output, CliError> {
    let runs = facade.runs()?;
    Ok(Output::Runs(runs))
}

pub fn cmd_capabilities(facade: &impl SystemFacade, _args: &[String]) -> Result<Output, CliError> {
    let caps = facade.capabilities();
    Ok(Output::Capabilities(caps))
}
```

### Acceptance Criteria

- [ ] `strata history mykey` prints version history
- [ ] `strata history mykey --limit 10` limits results
- [ ] `strata runs` lists all runs
- [ ] `strata capabilities` prints system capabilities

---

## Story #592: Output Formatting and Exit Codes

**File**: `crates/cli/src/output.rs` (NEW)

**Deliverable**: Output formatting and exit codes

### Implementation

```rust
use crate::value::Value;

/// Output types
pub enum Output {
    Ok,
    Nil,
    Value(Value),
    Integer(i64),
    Version(Version),
    Array(Vec<Option<Value>>),
    VersionedArray(Vec<Versioned<Value>>),
    VectorEntry(Versioned<VectorEntry>),
    Runs(Vec<RunInfo>),
    Capabilities(Capabilities),
    Error(StrataError),
}

/// Format output to stdout
pub fn format_output(output: &Output) -> String {
    match output {
        Output::Ok => "OK".to_string(),
        Output::Nil => "(nil)".to_string(),
        Output::Value(v) => format_value(v),
        Output::Integer(i) => format!("(integer) {}", i),
        Output::Version(v) => format_version(v),
        Output::Array(arr) => format_array(arr),
        Output::VersionedArray(arr) => format_versioned_array(arr),
        Output::VectorEntry(v) => format_vector_entry(v),
        Output::Runs(runs) => format_runs(runs),
        Output::Capabilities(caps) => serde_json::to_string_pretty(caps).unwrap(),
        Output::Error(e) => format!("(error) {}", e),
    }
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => format!("(integer) {}", if *b { 1 } else { 0 }),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => format!("{}", f),
        Value::String(s) => format!("\"{}\"", s),
        Value::Bytes(b) => {
            let json = serde_json::json!({ "$bytes": base64::encode(b) });
            serde_json::to_string(&json).unwrap()
        }
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string_pretty(&value_to_json(value)).unwrap()
        }
    }
}

fn format_version(v: &Version) -> String {
    serde_json::to_string(&serde_json::json!({
        "type": v.type_name(),
        "value": v.value()
    })).unwrap()
}

/// Exit codes (FROZEN)
pub mod exit_codes {
    /// Success
    pub const SUCCESS: i32 = 0;

    /// General error (NotFound, WrongType, InvalidKey, etc.)
    pub const ERROR: i32 = 1;

    /// Usage error (invalid arguments, unknown command)
    pub const USAGE: i32 = 2;
}

/// Format error to stderr
pub fn format_error(err: &StrataError) -> String {
    serde_json::to_string(&WireError::from(err)).unwrap()
}
```

### Acceptance Criteria

- [ ] Missing value: `(nil)`
- [ ] Integer/count: `(integer) N`
- [ ] Boolean: `(integer) 0` or `(integer) 1`
- [ ] String: `"text"`
- [ ] Null value: `null`
- [ ] Object/Array: JSON formatted
- [ ] Bytes: `{"$bytes": "<base64>"}`
- [ ] Error: JSON on stderr, non-zero exit code
- [ ] Exit code 0: Success
- [ ] Exit code 1: General error
- [ ] Exit code 2: Usage error

---

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_int() {
        assert_eq!(parse_value("123").unwrap(), Value::Int(123));
        assert_eq!(parse_value("-456").unwrap(), Value::Int(-456));
    }

    #[test]
    fn test_parse_float() {
        assert_eq!(parse_value("1.23").unwrap(), Value::Float(1.23));
        assert_eq!(parse_value("-1.23").unwrap(), Value::Float(-1.23));
    }

    #[test]
    fn test_parse_string() {
        assert_eq!(parse_value("\"hello\"").unwrap(), Value::String("hello".into()));
        assert_eq!(parse_value("hello").unwrap(), Value::String("hello".into()));
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_value("true").unwrap(), Value::Bool(true));
        assert_eq!(parse_value("false").unwrap(), Value::Bool(false));
    }

    #[test]
    fn test_parse_null() {
        assert_eq!(parse_value("null").unwrap(), Value::Null);
    }

    #[test]
    fn test_parse_bytes() {
        let result = parse_value("b64:SGVsbG8=").unwrap();
        assert_eq!(result, Value::Bytes(b"Hello".to_vec()));
    }

    #[test]
    fn test_parse_json_object() {
        let result = parse_value(r#"{"a": 1, "b": 2}"#).unwrap();
        if let Value::Object(obj) = result {
            assert_eq!(obj.get("a"), Some(&Value::Int(1)));
            assert_eq!(obj.get("b"), Some(&Value::Int(2)));
        } else {
            panic!("Expected Object");
        }
    }

    #[test]
    fn test_output_formatting() {
        assert_eq!(format_output(&Output::Nil), "(nil)");
        assert_eq!(format_output(&Output::Integer(42)), "(integer) 42");
        assert_eq!(
            format_output(&Output::Value(Value::String("hello".into()))),
            "\"hello\""
        );
    }
}
```

---

## Files Modified/Created

| File | Action |
|------|--------|
| `crates/cli/src/main.rs` | CREATE - CLI entry point |
| `crates/cli/src/parser.rs` | CREATE - Argument parser |
| `crates/cli/src/output.rs` | CREATE - Output formatting |
| `crates/cli/src/commands/mod.rs` | CREATE - Command module |
| `crates/cli/src/commands/kv.rs` | CREATE - KV commands |
| `crates/cli/src/commands/json.rs` | CREATE - JSON commands |
| `crates/cli/src/commands/event.rs` | CREATE - Event commands |
| `crates/cli/src/commands/vector.rs` | CREATE - Vector commands |
| `crates/cli/src/commands/state.rs` | CREATE - State commands |
| `crates/cli/src/commands/history.rs` | CREATE - History commands |
| `crates/cli/src/commands/run.rs` | CREATE - Run commands |
| `Cargo.toml` | MODIFY - Add cli crate |
