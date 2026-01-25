# Storage and Durability Naming Analysis

## The Problem

StrataDB currently has confusing terminology around "in-memory" that conflates two orthogonal concepts:

1. **Where data is stored** (persistence location)
2. **How data survives crashes** (durability guarantees)

This creates user confusion because `.in_memory()` doesn't mean what users expect.

---

## Current Architecture

### Storage Layer (ShardedStore)

The storage layer is **always in-memory**:

```rust
pub struct ShardedStore {
    shards: DashMap<RunId, Shard>,  // Always in RAM
    version: AtomicU64,
}
```

Data lives in a DashMap regardless of configuration. This is not configurable.

### Durability Layer (WAL)

The WAL controls crash recovery:

```rust
pub enum DurabilityMode {
    InMemory,   // WAL bypassed, no fsync
    Buffered,   // Periodic fsync
    Strict,     // fsync every commit
}
```

### File Creation

**Critical issue**: Even with `DurabilityMode::InMemory`, the database still creates files:

```rust
// In Database::open_with_mode()
std::fs::create_dir_all(&data_dir)?;           // Creates data directory
std::fs::create_dir_all(&wal_dir)?;            // Creates WAL directory
let wal = WAL::open(&wal_path, durability_mode)?;  // Creates WAL file
```

And in `WAL::open()`:
```rust
std::fs::create_dir_all(parent)?;              // Creates parent dirs
let file = OpenOptions::new()
    .create(true)
    .append(true)
    .open(&path)?;                              // Creates/opens file
```

---

## The Confusion

### What Users Expect

```rust
// User thinks: "No disk files, everything in memory"
let db = StrataBuilder::new()
    .in_memory()
    .open_temp()?;
```

### What Actually Happens

```rust
// Reality: Creates temp directory with WAL file, just doesn't fsync
// /tmp/inmem-test-uuid/
// └── wal/
//     └── current.wal    <-- File exists!
```

### The Two Orthogonal Concepts

| Concept | Description | Current API |
|---------|-------------|-------------|
| **Persistence Location** | Where files are stored | `.path()`, `.open()`, `.open_temp()` |
| **Durability Mode** | How WAL syncs to disk | `.in_memory()`, `.buffered()`, `.strict()` |

These are independent axes but share confusing terminology:

```
                    Durability Mode

         None        Buffered       Strict
         (no sync)   (periodic)     (every write)
           │            │              │
    ┌──────┼────────────┼──────────────┼──────┐
    │      │            │              │      │
No  │   Current      Not            Not      │  P
Disk│   "in_memory"  Possible?      Possible?│  e
    │      │            │              │      │  r
    ├──────┼────────────┼──────────────┼──────┤  s
    │      │            │              │      │  i
Temp│   Current      Current        Current  │  s
Dir │   Behavior     Behavior       Behavior │  t
    │      │            │              │      │  e
    ├──────┼────────────┼──────────────┼──────┤  n
    │      │            │              │      │  c
User│   Possible     Current        Current  │  e
Path│   (unusual)    Production     Critical │
    │      │            │              │      │
    └──────┴────────────┴──────────────┴──────┘
```

---

## Use Cases

### 1. Unit Tests (Fast, Disposable)
- **Need**: Maximum speed, no cleanup needed
- **Want**: No disk files at all
- **Current**: `.in_memory().open_temp()` - Creates temp files anyway

### 2. Integration Tests (Fast, Isolated)
- **Need**: Speed, test isolation
- **Want**: Temp directory, minimal durability
- **Current**: `.in_memory().open_temp()` - Works but naming confusing

### 3. Development (Fast Iteration)
- **Need**: Speed, data survives restarts during session
- **Want**: Temp directory, buffered durability
- **Current**: `.buffered().open_temp()` - Works

### 4. Production (Balanced)
- **Need**: Good performance, crash recovery
- **Want**: User path, buffered durability
- **Current**: `.path("./data").buffered().open()` - Works

### 5. Critical Data (Maximum Safety)
- **Need**: Zero data loss
- **Want**: User path, strict durability
- **Current**: `.path("./data").strict().open()` - Works

### 6. Embedded/Caching (No Persistence)
- **Need**: Fast cache, no disk usage
- **Want**: No files at all, data lost on close
- **Current**: Not truly supported (files still created)

---

## Options

### Option A: Rename Durability Methods

Keep current behavior but use clearer names:

```rust
// Before (confusing)
.in_memory()    // Sounds like "no disk"
.buffered()
.strict()

// After (clearer)
.no_durability()    // or .volatile() or .ephemeral_durability()
.buffered()         // unchanged
.strict()           // unchanged
```

**Pros:**
- Minimal code change
- No new features needed
- Backwards compatible with deprecation

**Cons:**
- Still creates files even with "no durability"
- Doesn't address the "truly no disk" use case

---

### Option B: Add True No-Disk Mode

Add a new storage mode that uses no disk files:

```rust
// New API
let db = StrataBuilder::new()
    .memory_only()      // No disk files at all
    .open()?;

// vs disk-based (existing)
let db = StrataBuilder::new()
    .path("./data")
    .buffered()
    .open()?;
```

Implementation would require:
- Optional WAL (None for memory-only)
- No directory creation
- Clear lifecycle (data gone when db dropped)

**Pros:**
- Addresses the "truly no disk" use case
- Clean separation of concepts
- Better for unit tests and caching

**Cons:**
- More implementation work
- New code paths to test
- Need to handle recovery (no-op for memory-only)

---

### Option C: Two-Axis Configuration

Make both axes explicit:

```rust
// Persistence: where data goes
enum Persistence {
    MemoryOnly,           // No disk files
    TempDirectory,        // Auto-generated temp path
    Directory(PathBuf),   // User-specified path
}

// Durability: how WAL syncs
enum Durability {
    None,      // No WAL / no sync
    Buffered,  // Periodic sync
    Strict,    // Immediate sync
}

// API
let db = StrataBuilder::new()
    .persistence(Persistence::TempDirectory)
    .durability(Durability::Buffered)
    .open()?;

// Or with convenience methods
let db = StrataBuilder::new()
    .temp_directory()
    .buffered()
    .open()?;
```

**Pros:**
- Most explicit and clear
- Covers all combinations
- Self-documenting

**Cons:**
- Larger API surface
- Some combinations are nonsensical (MemoryOnly + Strict)
- Breaking change

---

### Option D: Simplify to Common Cases

Only support the combinations users actually need:

```rust
// Testing: fast, no cleanup
let db = Strata::testing()?;

// Development: temp dir, survives restart
let db = Strata::development()?;

// Production: user path, good durability
let db = Strata::open("./data")?;

// Critical: user path, max durability
let db = Strata::open_strict("./audit")?;
```

**Pros:**
- Simplest API
- Hard to misconfigure
- Clear intent

**Cons:**
- Less flexible
- May not cover all use cases
- Opinionated

---

### Option E: Hybrid Approach

Combine Options A and B:

```rust
// Convenience methods for common cases
Strata::open(path)           // Production default (buffered)
Strata::open_temp()          // Testing (temp dir, no durability)
Strata::memory_only()        // Pure in-memory (no files)

// Builder for full control
StrataBuilder::new()
    .path("./data")          // or .temp_dir() or .memory_only()
    .durability(Durability::Buffered)  // or ::None or ::Strict
    .open()?
```

And rename/deprecate the confusing methods:
```rust
// Deprecated
.in_memory()  -> .no_durability()  // clearer name

// Keep
.buffered()   // unchanged
.strict()     // unchanged
```

**Pros:**
- Simple defaults for common cases
- Full control available
- Backwards compatible path
- Addresses all use cases

**Cons:**
- Multiple ways to do things
- Documentation burden

---

## Recommendation

**Option E (Hybrid)** provides the best balance:

1. **Rename `.in_memory()` to `.no_durability()`** - Immediate clarity improvement
2. **Add `Strata::memory_only()`** - True no-disk mode for testing/caching
3. **Keep builder for advanced cases** - Full flexibility when needed

### Migration Path

```rust
// Phase 1: Deprecate confusing name
#[deprecated(note = "Use .no_durability() instead - this sets WAL mode, not storage location")]
pub fn in_memory(self) -> Self { ... }

pub fn no_durability(self) -> Self { ... }

// Phase 2: Add true memory-only mode
pub fn memory_only(self) -> Self { ... }  // No disk files at all

// Phase 3: Documentation update
// Make the two axes crystal clear in docs
```

### Resulting API

```rust
// === Common Cases (Simple) ===

// Production: persistent, crash-safe
let db = Strata::open("./data")?;

// Testing: temporary, fast
let db = Strata::open_temp()?;

// Caching: no disk at all
let db = Strata::memory_only()?;


// === Full Control (Builder) ===

// Custom durability
let db = StrataBuilder::new()
    .path("./data")
    .strict()              // or .buffered() or .no_durability()
    .open()?;

// Custom temp with durability
let db = StrataBuilder::new()
    .no_durability()       // Don't sync WAL
    .open_temp()?;         // Use temp directory
```

---

## Summary Table

| Use Case | Current API | Proposed API | Files Created |
|----------|-------------|--------------|---------------|
| Unit tests | `.in_memory().open_temp()` | `Strata::memory_only()` | None |
| Integration tests | `.in_memory().open_temp()` | `Strata::open_temp()` | Temp dir |
| Development | `.buffered().open_temp()` | Same | Temp dir |
| Production | `.path(p).buffered().open()` | `Strata::open(p)` | User dir |
| Critical | `.path(p).strict().open()` | `.path(p).strict().open()` | User dir |

---

## Questions to Resolve

1. Should `memory_only()` even support durability modes? (Probably no - it's inherently volatile)
2. What happens to `memory_only()` data on `db.flush()`? (No-op)
3. Should we support persistence without durability? (Unusual but valid for append-only logs)
4. Timeline for deprecation of `.in_memory()`?

---

## Implementation Status

**Status: IMPLEMENTED (M13)**

Option E (Hybrid) has been implemented with the following changes:

### Implemented Features

1. **`Strata::ephemeral()`** - True no-disk mode (renamed from `memory_only()` per user request)
   - Creates no files or directories
   - Has no WAL
   - All data lost on drop
   - `db.is_ephemeral()` returns `true`

2. **`.no_durability()`** - Renamed from `.in_memory()`
   - Sets `DurabilityMode::InMemory` (no fsync)
   - Files still created (WAL exists but doesn't sync)
   - Data can survive restarts if process doesn't crash

3. **Deprecation of `.in_memory()`**
   - Method still works but shows deprecation warning
   - Message directs users to `.no_durability()` or `Strata::ephemeral()`

### New API

```rust
// === Ephemeral (No Disk) ===
let db = Strata::ephemeral()?;
assert!(db.is_ephemeral());

// === Temp Directory (Disk, No Sync) ===
let db = StrataBuilder::new()
    .no_durability()
    .open_temp()?;
assert!(!db.is_ephemeral());

// === Production (Disk, Durable) ===
let db = Strata::open("./data")?;
```

### Summary Table (Updated)

| Use Case | API | Files Created | WAL | Recovery |
|----------|-----|---------------|-----|----------|
| Unit tests | `Strata::ephemeral()` | None | None | No |
| Integration tests | `.no_durability().open_temp()` | Temp dir | Yes (no sync) | Yes |
| Development | `.buffered().open_temp()` | Temp dir | Yes (sync) | Yes |
| Production | `Strata::open(p)` | User dir | Yes (sync) | Yes |
| Critical | `.path(p).strict().open()` | User dir | Yes (immediate sync) | Yes |

### Resolved Questions

1. **Should `ephemeral()` support durability modes?** → No, it ignores durability settings
2. **What happens on `db.flush()` for ephemeral?** → No-op, returns `Ok(())`
3. **Persistence without durability?** → Yes, supported via `.no_durability().open(path)`
4. **Deprecation timeline** → `.in_memory()` deprecated in v0.14.0, removal in v0.16.0
