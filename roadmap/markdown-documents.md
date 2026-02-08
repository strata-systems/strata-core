# Document: A Composite Primitive

**Theme**: `put_document("abc.md")` → searchable via hybrid search. Zero new storage primitives.

Agents produce and consume documents — research notes, meeting summaries, analysis reports, chain-of-thought logs. Today these are stored as opaque strings in KV. Auto-embed generates one embedding for the entire value, which is fine for short strings but terrible for a 50-section document. Search can't distinguish section 3 from section 47.

Document is a composite primitive. It stores the raw content, parses it into sections, embeds each section individually, and registers the document for fast listing. From the caller's perspective: one call in, searchable via the existing `Search` command.

---

## The API

Four commands. Follows the existing pattern: optional branch/space, serializable, maps to existing `Output` variants.

```rust
// ==================== Document (4) ====================

/// Store or update a document.
/// Parses into sections, embeds each section, updates registry.
/// Returns: Output::Version
DocPut {
    branch: Option<BranchId>,
    space: Option<String>,
    key: String,       // e.g. "notes/standup.md"
    content: String,   // raw document text
}

/// Get the raw document content.
/// Returns: Output::MaybeVersioned
DocGet {
    branch: Option<BranchId>,
    space: Option<String>,
    key: String,
}

/// Delete a document and all derived data (chunks, embeddings, registry entry).
/// Returns: Output::Bool (true if document existed)
DocDelete {
    branch: Option<BranchId>,
    space: Option<String>,
    key: String,
}

/// List documents with metadata.
/// Returns: Output::MaybeVersioned (registry JSON)
DocList {
    branch: Option<BranchId>,
    space: Option<String>,
    prefix: Option<String>,
}
```

Search is not a Document command. It's the existing `Search` command — document sections participate in hybrid search automatically, just like auto-embedded KV entries do today.

---

## What happens on `DocPut`

```
db.doc_put("notes/standup.md", "# Standup\n\n## Monday\n- Fixed bug\n\n## Tuesday\n...")
    │
    ├─► KV        _doc:notes/standup.md  →  raw content string (source of truth)
    │
    ├─► JSON      _doc:notes/standup.md  →  chunk index (section ids, headings, byte ranges)
    │
    ├─► JSON      _doc_registry          →  updated at path $.docs["notes/standup.md"]
    │
    └─► Vector    _system_embed_doc      →  one embedding per section (best-effort)
```

Three primitives: KV, JSON, Vector. The first two are atomic. Vector is best-effort (failures logged, never propagated — same as `embed_hook`).

### Step by step

1. **Parse** — Run content through the chunking fallback chain (headings → paragraphs → windows). Works regardless of format or extension.

2. **Store raw content** — `kv.put("_doc:notes/standup.md", Value::String(content))`. Source of truth. Versioned automatically.

3. **Store chunk index** — `json.set("_doc:notes/standup.md", "$", chunk_index)`. Internal metadata the handler uses to hydrate search results:

```json
{
  "format": "markdown",
  "chunks": [
    { "id": "s0", "heading": "Monday", "level": 2, "byte_range": [12, 45] },
    { "id": "s1", "heading": "Tuesday", "level": 2, "byte_range": [46, 89] }
  ]
}
```

4. **Update registry** — `json.set("_doc_registry", "$.docs[\"notes/standup.md\"]", entry)`:

```json
{
  "title": "Standup",
  "word_count": 48,
  "chunk_count": 2,
  "format": "markdown",
  "created_at": 1717200000,
  "updated_at": 1717372800
}
```

5. **Embed chunks** — For each section, embed the heading + body text into `_system_embed_doc` with metadata:

| Vector key | Metadata |
|---|---|
| `notes/standup.md\x1fs0` | `{ "doc": "notes/standup.md", "chunk": "s0", "heading": "Monday" }` |
| `notes/standup.md\x1fs1` | `{ "doc": "notes/standup.md", "chunk": "s1", "heading": "Tuesday" }` |

On update, old chunk embeddings are deleted and new ones inserted (section count may change).

---

## Search integration

No new search command. The existing `Search` command gains document awareness:

```python
db.doc_put("notes/standup.md", standup_content)
db.doc_put("notes/retro.md", retro_content)

# This already exists — now it also searches document sections
results = db.search("bug fix")
```

When hybrid search encounters hits from `_system_embed_doc`, it hydrates them using the chunk index (JSON) to add heading context:

```python
SearchResultHit {
    entity: "notes/standup.md#s0",    # doc_key#chunk_id
    primitive: "document",
    score: 0.82,
    rank: 1,
    snippet: "Monday — Fixed bug in checkout flow...",
}
```

Document sections participate in RRF alongside KV, JSON, and Event hits. No separate search pipeline.

---

## Document registry

A single JSON document `_doc_registry` per (branch, space), maintained by the handler:

```json
{
  "docs": {
    "notes/standup.md": {
      "title": "Standup",
      "word_count": 48,
      "chunk_count": 2,
      "format": "markdown",
      "created_at": 1717200000,
      "updated_at": 1717372800
    },
    "docs/readme.md": {
      "title": "README",
      "word_count": 1200,
      "chunk_count": 12,
      "format": "markdown",
      "created_at": 1717100000,
      "updated_at": 1717286400
    }
  },
  "count": 2
}
```

`DocList` returns this as a `MaybeVersioned` — single read, all metadata included. Prefix filtering happens server-side in the handler.

Why not just `kv.list(prefix="_doc:")`? That gives keys but no metadata. The registry gives titles, word counts, chunk counts, and timestamps in one read.

---

## Chunking

Not all Markdown is well-formatted. Many `.md` files are just prose — no headings, no structure, barely any formatting. The chunker must handle this gracefully. It's a fallback chain, not a format branch:

```
Document content
    │
    ▼
1. Heading split (pulldown-cmark)
    → Found headings? → sections by heading
    → No headings?  ▼
2. Paragraph split (double newlines)
    → Found paragraphs? → sections by paragraph
    → Single block?   ▼
3. Window split (token-based sliding window)
    → 256 tokens, 128-token stride
```

Every document goes through this chain regardless of extension. A `.md` file with no headings gets paragraph-chunked. A `.txt` file gets the same treatment. The extension is stored as metadata in the registry, but it doesn't change the chunking logic.

### Step 1: Heading split

Parse with `pulldown-cmark` (handles plain text gracefully — no headings means no splits). If the parser finds headings, each heading starts a new chunk. The offset iterator gives byte ranges back into the source for lossless extraction.

### Step 2: Paragraph split

If step 1 produces a single chunk (no headings found, or content before the first heading is the whole document), split on double newlines (`\n\n`). Each paragraph becomes a chunk.

### Step 3: Window split

Any chunk from step 1 or 2 that exceeds 256 tokens gets split into overlapping windows (256 tokens, 128-token stride) with `_w0`, `_w1` suffixes on the chunk id. This is the final safety net — no chunk is ever too large for MiniLM.

### Result

| Input | Chunks |
|---|---|
| Well-structured Markdown (headings) | One chunk per heading section |
| Markdown with no headings, just paragraphs | One chunk per paragraph |
| Wall of text, no structure | Overlapping sliding windows |
| Mixed (some headings, some long sections) | Heading sections, with long ones windowed |

The chunker always produces reasonable chunks. Garbage in, reasonable chunks out.

---

## Handler

### File: `crates/executor/src/handlers/document.rs`

```rust
pub fn doc_put(
    p: &Arc<Primitives>,
    branch: BranchId,
    space: String,
    key: String,
    content: String,
) -> Result<Output> {
    let core_bid = to_core_branch_id(&branch)?;
    let internal_key = format!("_doc:{}", key);

    // 1. Parse into chunks (headings → paragraphs → windows)
    let chunks = chunk_document(&content);

    // 2. Store raw content in KV
    let version = convert_result(
        p.kv.put(&core_bid, &space, &internal_key, Value::String(content.clone()))
    )?;

    // 3. Store chunk index in JSON
    let chunk_index = build_chunk_index(&chunks);
    p.json.set_or_create(&core_bid, &space, &internal_key, "$", chunk_index)?;

    // 4. Update registry
    let title = extract_title(&chunks);
    let format = detect_format(&key); // extension-based, for metadata only
    let registry_entry = serde_json::json!({
        "title": title,
        "word_count": count_words(&content),
        "chunk_count": chunks.len(),
        "format": format,
        "created_at": /* preserve on update */,
        "updated_at": timestamp_micros(),
    });
    ensure_registry_exists(p, core_bid, &space);
    p.json.set(&core_bid, &space, "_doc_registry",
        &format!("$.docs[\"{}\"]", key), registry_entry)?;

    // 5. Embed chunks (best-effort)
    #[cfg(feature = "embed")]
    embed_document_chunks(p, core_bid, &space, &key, &chunks);

    Ok(Output::Version(version))
}
```

### Embed helper (follows `embed_hook` pattern)

```rust
#[cfg(feature = "embed")]
fn embed_document_chunks(
    p: &Arc<Primitives>,
    branch_id: BranchId,
    space: &str,
    doc_key: &str,
    chunks: &[Chunk],
) {
    // Delete old embeddings for this document (chunk count may have changed)
    delete_old_doc_embeddings(p, branch_id, doc_key);

    // Ensure shadow collection exists (384-dim cosine, same as other shadows)
    ensure_shadow_collection(p, branch_id, "_system_embed_doc");

    for chunk in chunks {
        let text = format!("{} — {}", chunk.heading.as_deref().unwrap_or(""), chunk.body);
        // embed + insert (same model, same pattern as embed_hook.rs)
        maybe_embed_text(p, branch_id, space,
            "_system_embed_doc",
            &format!("{}\x1f{}", doc_key, chunk.id),
            &text,
            EntityRef::document(doc_key, &chunk.id),
        );
    }
}

#[cfg(not(feature = "embed"))]
fn embed_document_chunks(..) {} // no-op
```

---

## Markdown parser

**pulldown-cmark** — the parser behind `rustdoc`.

| Criterion | Value |
|---|---|
| Downloads/month | ~4,000,000 |
| Required deps | 3 (bitflags, memchr, unicase) |
| CommonMark + GFM | Yes |
| Key feature | `into_offset_iter()` → byte ranges into source |

The offset iterator is decisive: we slice the original source by byte ranges to extract section text. No lossy reconstruction.

---

## Feature gating

```toml
# Root Cargo.toml
[features]
document = ["strata-executor/document"]

# crates/executor/Cargo.toml
[features]
document = ["strata-intelligence/document"]

# crates/intelligence/Cargo.toml
[features]
document = ["dep:pulldown-cmark"]
```

Independent of `embed`:

| `document` | `embed` | Behavior |
|---|---|---|
| off | off | No document commands |
| on | off | Store, retrieve, list documents. No search. |
| on | on | Full pipeline: store, chunk, embed, searchable via `Search` |
| off | on | No document commands. Auto-embed works for KV/JSON/Event/State. |

---

## SDK surface

### Python

```python
db = stratadb.Strata.open("/data", auto_embed=True)

db.doc_put("notes/standup.md", open("standup.md").read())
db.doc_put("notes/retro.md", open("retro.md").read())

raw = db.doc_get("notes/standup.md")
docs = db.doc_list(prefix="notes/")
db.doc_delete("notes/retro.md")

# Search finds document sections alongside KV, JSON, Event hits
results = db.search("bug fix")
```

### Node.js

```javascript
const db = Strata.open('/data', { autoEmbed: true });

db.docPut('notes/standup.md', fs.readFileSync('standup.md', 'utf8'));
const raw = db.docGet('notes/standup.md');
const docs = db.docList({ prefix: 'notes/' });

const results = db.search('bug fix');
```

### CLI

```
strata doc put notes/standup.md < standup.md
strata doc get notes/standup.md
strata doc list --prefix "notes/"
strata doc delete notes/retro.md
strata search "bug fix"
```

### MCP

Four new tools:

| Tool | Description |
|---|---|
| `doc_put` | Store/update a document |
| `doc_get` | Get raw document content |
| `doc_list` | List documents with metadata |
| `doc_delete` | Delete a document |

Search already exists as a tool — it now returns document hits automatically.

---

## What you get for free

Because Document is built on existing primitives, every capability composes:

- **Branching** — `doc_put` on a branch, diff, merge. Documents follow branch isolation.
- **Transactions** — Atomic multi-document writes via `txn_begin` / `txn_commit`.
- **Version history** — KV versioning tracks every write. `kv_getv("_doc:key")` gives full history.
- **Spaces** — Namespace documents: `doc_put("readme.md", content, space="docs")`.
- **Hybrid search** — Document sections participate in RRF alongside all other primitives.

---

## What's NOT in v1

These are deliberate omissions, not oversights:

- **`DocGetTree`** — Parsed section tree as a user-facing command. The chunk index is stored internally for search hydration. If callers need the structure, they parse the raw source client-side.
- **`DocGetSection`** — Navigate to a specific section by heading path. Useful, but not core. Can be added later without changing the storage layout.
- **`DocSearch`** — Separate search command for documents only. Unnecessary — the existing `Search` command handles it. Filter with `primitives: ["document"]` if needed.
- **EventLog per edit** — Section-level diff tracking. KV versioning provides the audit trail. Edit events can be layered on later.
- **Frontmatter parsing** — YAML/TOML frontmatter into metadata. Adds parser complexity. Can be added when there's demand.

---

## Implementation sequence

### Phase 1: Store + List

`DocPut`, `DocGet`, `DocDelete`, `DocList`. Markdown parser. Chunk index. Registry. No embeddings.

**Scope**: `crates/intelligence/src/document/` (parser + chunker), `crates/executor/src/handlers/document.rs`, command.rs (4 variants), tests.

### Phase 2: Search

`embed_document_chunks()`. Wire `_system_embed_doc` into hybrid search RRF. Add `"document"` to `PrimitiveType` enum and `SearchResultHit` hydration.

### Phase 3: SDK surface

Wire through CLI, Python, Node.js, MCP. Four commands across four surfaces.

---

## Open questions

- **Max chunk size**: What's the right windowing threshold? MiniLM's context is 256 tokens, but embedding quality degrades well before that. 128 tokens per chunk may be better.
- **Chunk overlap**: Overlapping windows preserve context across chunk boundaries but increase storage. Is 128-token stride the right default, or should it be configurable?
- **Minimum chunk size**: Very short paragraphs (one-liners, bullet items) produce low-quality embeddings. Should consecutive short chunks be merged up to a minimum token count?

---

## Dependencies

- v0.5 (six primitives, spaces, branches) — **shipped**
- v0.7 (auto-embedding pipeline) — **shipped**, required only for Phase 2
- Independent of v0.8–v0.12

## Key files (planned)

| Area | Files |
|---|---|
| Markdown parser + chunker | `crates/intelligence/src/document/` |
| Executor handler | `crates/executor/src/handlers/document.rs` |
| Commands | `crates/executor/src/command.rs` (4 new variants) |
| Search hydration | `crates/executor/src/handlers/search.rs` |
| Feature gate | intelligence, executor, root `Cargo.toml` |
