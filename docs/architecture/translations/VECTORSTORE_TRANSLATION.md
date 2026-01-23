# VectorStore: Substrate → Primitive Translation

## Contract Promises (M11_CONTRACT.md)

### Facade API (Section 10.4)

| Operation | Signature | Return | Notes |
|-----------|-----------|--------|-------|
| `vset` | `(key, vector: f32[], metadata: Object)` | `()` | Store vector with metadata |
| `vget` | `(key)` | `Option<Versioned<{vector, metadata}>>` | **Returns Versioned** |
| `vdel` | `(key)` | `bool` | Delete vector |

**Dimension rules:**
- Vector dimensions: 1 to `max_vector_dim` (default 8192)
- If key exists with different dimension: return `ConstraintViolation` with reason `vector_dim_mismatch`
- Dimension changes are not allowed; delete and re-create if needed

### Substrate API (Section 11.7)

```rust
vector_set(run, key, vector: Vec<f32>, metadata: Value::Object) -> Version
vector_get(run, key) -> Option<Versioned<{vector, metadata}>>
vector_delete(run, key) -> bool
vector_history(run, key, limit?, before?) -> Vec<Versioned<Value>>
```

Note: Contract mentions `vector_history` as optional history access.

---

## Substrate API Promises

| Method | Signature | Purpose |
|--------|-----------|---------|
| `vector_upsert` | `(run, collection, key, vector, metadata) → Version` | Insert/update vector |
| `vector_get` | `(run, collection, key) → Option<Versioned<VectorData>>` | Get vector by key |
| `vector_delete` | `(run, collection, key) → bool` | Delete vector |
| `vector_search` | `(run, collection, query, k, filter, metric) → Vec<VectorMatch>` | Similarity search |
| `vector_collection_info` | `(run, collection) → Option<(dim, count, metric)>` | Get collection info |
| `vector_create_collection` | `(run, collection, dim, metric) → Version` | Create collection |
| `vector_drop_collection` | `(run, collection) → bool` | Delete collection |

## Primitive Provides

| Method | Signature | Version Exposed? |
|--------|-----------|------------------|
| `create_collection(run_id, name, config)` | `→ VectorResult<Versioned<CollectionInfo>>` | ✅ Yes |
| `delete_collection(run_id, name)` | `→ VectorResult<()>` | ❌ No |
| `get_collection(run_id, name)` | `→ VectorResult<Option<Versioned<CollectionInfo>>>` | ✅ Yes |
| `insert(run_id, collection, key, embedding, metadata)` | `→ VectorResult<Version>` | ✅ Yes |
| `get(run_id, collection, key)` | `→ VectorResult<Option<Versioned<VectorEntry>>>` | ✅ Yes |
| `delete(run_id, collection, key)` | `→ VectorResult<bool>` | ❌ No |
| `search(run_id, collection, query, k, filter)` | `→ VectorResult<Vec<VectorMatch>>` | ✅ (in results) |
| `collection_exists(run_id, name)` | `→ VectorResult<bool>` | ❌ No |

## Type Differences

| Substrate | Primitive | Conversion Needed |
|-----------|-----------|-------------------|
| `ApiRunId` | `RunId` | `run.to_run_id()` |
| `Value` (metadata) | `Option<JsonValue>` | `serde_json::Value` conversion |
| `VectorData = (Vec<f32>, Value)` | `VectorEntry` | Extract fields |
| `substrate::DistanceMetric` | `vector::DistanceMetric` | Same enum, different module |
| `substrate::SearchFilter` | `MetadataFilter` | Structural mapping |
| `substrate::VectorMatch` | `vector::VectorMatch` | Similar structure |
| `dimension: usize` + `metric` | `VectorConfig` | Create config struct |

---

## Method Translations

### `vector_upsert` - DIRECT MAPPING

**Substrate**: Upsert vector with metadata.

**Primitive**: `insert()` has upsert semantics.

**Translation**:
```rust
fn vector_upsert(
    &self,
    run: &ApiRunId,
    collection: &str,
    key: &str,
    vector: &[f32],
    metadata: Option<Value>,
) -> StrataResult<Version> {
    let run_id = run.to_run_id();

    // Convert Value → JsonValue for metadata
    let json_metadata = metadata
        .map(|v| serde_json::to_value(v))
        .transpose()?;

    match self.vector.insert(run_id, collection, key, vector, json_metadata) {
        Ok(version) => Ok(version),
        Err(VectorError::CollectionNotFound { .. }) => {
            // Auto-create collection if needed (substrate semantics)
            let config = VectorConfig {
                dimension: vector.len(),
                metric: DistanceMetric::Cosine,  // Default
            };
            self.vector.create_collection(run_id, collection, config)?;
            self.vector.insert(run_id, collection, key, vector, json_metadata)
                .map_err(|e| e.into())
        }
        Err(e) => Err(e.into()),
    }
}
```

**Gap**: Substrate auto-creates collection on first insert; Primitive requires explicit creation.

---

### `vector_get` - TYPE CONVERSION

**Substrate**: Returns `Versioned<VectorData>` where `VectorData = (Vec<f32>, Value)`.

**Primitive**: Returns `Versioned<VectorEntry>`.

**Translation**:
```rust
fn vector_get(
    &self,
    run: &ApiRunId,
    collection: &str,
    key: &str,
) -> StrataResult<Option<Versioned<VectorData>>> {
    let run_id = run.to_run_id();

    match self.vector.get(run_id, collection, key)? {
        Some(versioned) => {
            let entry = versioned.value;
            // Convert JsonValue → Value for metadata
            let metadata = entry.metadata
                .map(|jv| serde_json::from_value(jv))
                .transpose()?
                .unwrap_or(Value::Null);

            Ok(Some(Versioned {
                value: (entry.embedding, metadata),
                version: versioned.version,
                timestamp: versioned.timestamp,
            }))
        }
        None => Ok(None),
    }
}
```

---

### `vector_delete` - RETURN TYPE

**Substrate**: Returns `bool` (existed).

**Primitive**: Same.

**Translation**:
```rust
fn vector_delete(&self, run: &ApiRunId, collection: &str, key: &str) -> StrataResult<bool> {
    let run_id = run.to_run_id();
    Ok(self.vector.delete(run_id, collection, key)?)
}
```

**No gap** - direct mapping.

---

### `vector_search` - FILTER + METRIC MAPPING

**Substrate**: Takes `SearchFilter` enum and optional `DistanceMetric`.

**Primitive**: Takes `MetadataFilter` and uses collection default metric.

**Translation**:
```rust
fn vector_search(
    &self,
    run: &ApiRunId,
    collection: &str,
    query: &[f32],
    k: u64,
    filter: Option<SearchFilter>,
    metric: Option<DistanceMetric>,
) -> StrataResult<Vec<substrate::VectorMatch>> {
    let run_id = run.to_run_id();

    // Convert substrate SearchFilter → primitive MetadataFilter
    let prim_filter = filter.map(|f| convert_filter(f));

    // Note: primitive uses collection's default metric
    // Substrate allows per-query metric override
    if metric.is_some() {
        // TODO: Primitive doesn't support per-query metric override
        // For now, ignore metric parameter
    }

    let matches = self.vector.search(run_id, collection, query, k as usize, prim_filter)?;

    // Convert primitive VectorMatch → substrate VectorMatch
    Ok(matches
        .into_iter()
        .map(|m| substrate::VectorMatch {
            key: m.key,
            score: m.score,
            vector: m.embedding,
            metadata: json_to_value(m.metadata),
            version: m.version,
        })
        .collect())
}

fn convert_filter(f: SearchFilter) -> MetadataFilter {
    match f {
        SearchFilter::Equals { field, value } => MetadataFilter::Equals { field, value: to_json(value) },
        SearchFilter::Prefix { field, prefix } => MetadataFilter::Prefix { field, prefix },
        SearchFilter::Range { field, min, max } => MetadataFilter::Range { field, min: to_json(min), max: to_json(max) },
        SearchFilter::And(filters) => MetadataFilter::And(filters.into_iter().map(convert_filter).collect()),
        SearchFilter::Or(filters) => MetadataFilter::Or(filters.into_iter().map(convert_filter).collect()),
        SearchFilter::Not(f) => MetadataFilter::Not(Box::new(convert_filter(*f))),
    }
}
```

**Gap**: Substrate allows per-query metric override; Primitive uses collection default.

---

### `vector_collection_info` - DIFFERENT RETURN TYPE

**Substrate**: Returns `Option<(usize, u64, DistanceMetric)>` (dimension, count, metric).

**Primitive**: Returns `Option<Versioned<CollectionInfo>>`.

**Translation**:
```rust
fn vector_collection_info(
    &self,
    run: &ApiRunId,
    collection: &str,
) -> StrataResult<Option<(usize, u64, DistanceMetric)>> {
    let run_id = run.to_run_id();

    match self.vector.get_collection(run_id, collection)? {
        Some(versioned) => {
            let info = versioned.value;
            Ok(Some((info.dimension, info.count, info.metric)))
        }
        None => Ok(None),
    }
}
```

**No gap** - just tuple extraction.

---

### `vector_create_collection` - CONFIG CONSTRUCTION

**Substrate**: Takes dimension and metric separately.

**Primitive**: Takes `VectorConfig` struct.

**Translation**:
```rust
fn vector_create_collection(
    &self,
    run: &ApiRunId,
    collection: &str,
    dimension: usize,
    metric: DistanceMetric,
) -> StrataResult<Version> {
    let run_id = run.to_run_id();

    let config = VectorConfig {
        dimension,
        metric,
    };

    let versioned = self.vector.create_collection(run_id, collection, config)?;
    Ok(versioned.version)
}
```

**No gap** - just config construction.

---

### `vector_drop_collection` - RETURN TYPE

**Substrate**: Returns `bool` (existed).

**Primitive**: `delete_collection` returns `()` or error.

**Translation**:
```rust
fn vector_drop_collection(&self, run: &ApiRunId, collection: &str) -> StrataResult<bool> {
    let run_id = run.to_run_id();

    match self.vector.delete_collection(run_id, collection) {
        Ok(()) => Ok(true),
        Err(VectorError::CollectionNotFound { .. }) => Ok(false),
        Err(e) => Err(e.into()),
    }
}
```

**Gap**: Primitive throws error if collection doesn't exist; Substrate wants bool return.

---

## Summary Table

| Substrate Method | Primitive Method | Gap |
|-----------------|------------------|-----|
| `vector_upsert` | `insert` | Auto-create collection semantics |
| `vector_get` | `get` | Type conversion (VectorEntry → VectorData) |
| `vector_delete` | `delete` | None - direct |
| `vector_search` | `search` | Per-query metric not supported |
| `vector_collection_info` | `get_collection` | Extract fields from CollectionInfo |
| `vector_create_collection` | `create_collection` | Wrap params in VectorConfig |
| `vector_drop_collection` | `delete_collection` | Error → bool conversion |

## Gaps Requiring Primitive Enhancement

| Method | What's Needed |
|--------|---------------|
| `vector_search` | Support per-query distance metric override |

## Gaps Handled in Substrate

| Method | How Handled |
|--------|-------------|
| `vector_upsert` | Auto-create collection on CollectionNotFound |
| `vector_drop_collection` | Convert CollectionNotFound → false |
| Type conversions | JsonValue ↔ Value, VectorEntry ↔ VectorData |

## Error Mapping

| Primitive Error | Substrate Error |
|-----------------|-----------------|
| `VectorError::CollectionNotFound` | `StrataError::NotFound` |
| `VectorError::DimensionMismatch` | `StrataError::ConstraintViolation` |
| `VectorError::InvalidKey` | `StrataError::InvalidKey` |
| `VectorError::Storage` | `StrataError::StorageError` |
| `VectorError::Serialization` | `StrataError::SerializationError` |

## Additional Notes

1. **Metadata Type**: Substrate uses `strata_core::Value`, Primitive uses `serde_json::Value`. These should be the same underlying type but may need explicit conversion.

2. **Version Type**: Both use `Version` from strata_core. Primitive returns `Version::Counter` for inserts.

3. **Collection Auto-Creation**: Substrate `vector_upsert` should auto-create collection if it doesn't exist (dimension inferred from first vector, metric defaults to Cosine).

---

## Contract Gap Summary

### Facade → Substrate: FULLY COVERED

| Facade | Substrate | Status |
|--------|-----------|--------|
| `vset(key, vector, metadata)` | `begin(); vector_set(default, ...); commit()` | ✅ |
| `vget(key)` | `vector_get(default, key)` | ✅ |
| `vdel(key)` | `begin(); vector_delete(default, key); commit()` | ✅ |

### Substrate → Primitive: MOSTLY COVERED

| Substrate Method | Primitive Support | Gap |
|------------------|-------------------|-----|
| `vector_set` | `insert()` ✅ | Auto-create collection semantics |
| `vector_get` | `get()` ✅ | Type conversion only |
| `vector_delete` | `delete()` ✅ | None - direct |
| `vector_history` | ❌ **MISSING** | P2: Optional per contract |
| `vector_search` | `search()` ✅ | Not in facade, substrate-only |
| `vector_collection_info` | `get_collection()` ✅ | Not in facade, substrate-only |
| `vector_create_collection` | `create_collection()` ✅ | Not in facade, substrate-only |
| `vector_drop_collection` | `delete_collection()` ✅ | Not in facade, substrate-only |

### Collection Abstraction

Contract mentions vectors by key only (no collection concept in facade). Implementation options:

1. **Default collection**: All facade vectors go to a default collection per run
2. **Key-as-collection**: Parse key format `collection:key`
3. **Single collection**: One collection per run (simplest)

**Recommendation:** Single default collection per run for facade. Substrate exposes full collection API.

### Dimension Validation

Contract requires dimension mismatch detection:

```rust
fn vector_set(run, key, vector, metadata) -> Result<Version> {
    // Check if key exists with different dimension
    if let Some(existing) = self.vector_get(run, key)? {
        if existing.value.vector.len() != vector.len() {
            return Err(StrataError::ConstraintViolation {
                reason: "vector_dim_mismatch".into(),
                details: json!({
                    "existing_dim": existing.value.vector.len(),
                    "new_dim": vector.len()
                })
            });
        }
    }
    // Proceed with insert/update
    self.primitive.insert(...)
}
```
