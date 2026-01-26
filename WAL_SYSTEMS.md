# WAL Systems Architecture

## Overview

The strata-durability crate contains two WAL (Write-Ahead Log) systems that serve different purposes.

## System 1: MVCC WAL (wal.rs, encoding.rs, recovery.rs)

**Purpose**: Versioned key-value operations with MVCC support

**Used by**:
- strata-concurrency (transaction management, MVCC)
- strata-engine (database durability layer)
- strata-primitives (vector operations, run index)

**Key Types**:
- `WAL` - File handle with append/read operations
- `WALEntry` - Enum with BeginTxn, Write, Delete, CommitTxn, AbortTxn, Vector*, JSON*, etc.
- `DurabilityMode` - Strict/Batched/None fsync modes

**Features**:
- Rich `Key` type (namespace + type_tag + user_key)
- `Value` enum (Int, String, Bytes, etc.)
- Version tracking per entry (for MVCC)
- u64 transaction IDs

## System 2: Transaction Log WAL (wal_writer.rs, wal_reader.rs, wal_types.rs, etc.)

**Purpose**: Cross-primitive atomic transactions with run scoping

**Used by**:
- cross_primitive_atomicity tests
- (Future: SDK-level transaction API)

**Key Types**:
- `WalWriter` - Writer with transaction framing
- `WalReader` - Reader with corruption detection
- `WalEntry` - Struct with entry_type, tx_id, run_id, payload
- `WalEntryType` - Enum (0x00-0x7F range-based)
- `TxId` - UUID-based transaction ID
- `Transaction` - Builder for multi-primitive operations

**Features**:
- Run ID tracking (filter replay by run)
- UUID transaction IDs
- Opaque byte payloads
- Progress callbacks during recovery
- Point-in-time recovery (stop_at_version)

## Why Both Exist

1. **Different Data Models**: MVCC WAL uses rich types (Key, Value, version), Transaction Log uses opaque bytes
2. **Different ID Schemes**: MVCC uses u64, Transaction Log uses UUID
3. **Different Use Cases**: MVCC is for versioned storage, Transaction Log is for atomic commits

## Shared Code

Both systems share:
- `DurabilityMode` enum (re-exported)
- General patterns (CRC checksums, fsync modes)

## Future Consolidation

A full unification would require:
1. Migrating all crates to use opaque payloads
2. Adding version tracking to Transaction Log (or removing from MVCC)
3. Standardizing on one ID scheme
4. Extensive testing

This is tracked for future consideration.
