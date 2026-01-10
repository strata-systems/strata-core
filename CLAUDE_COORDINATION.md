# Claude Coordination Guide

Quick reference for coordinating multiple Claude instances working on this project.

## Current Work Assignment

**Update this table as Claudes start working on stories!**

| Claude | Current Story | Branch | Status | Started | ETA |
|--------|---------------|--------|--------|---------|-----|
| Claude 1 | - | - | Available | - | - |
| Claude 2 | - | - | Available | - | - |
| Claude 3 | - | - | Available | - | - |
| Claude 4 | - | - | Available | - | - |

## Epic 1 Parallelization Plan

**Story #6 MUST complete first** (creates workspace), then all others can run in parallel.

### Phase 1: Foundation (Sequential)
- ✅ Story #6: Cargo workspace (Claude 1) - **BLOCKS EVERYTHING**

### Phase 2: Core Types (4 Claudes in parallel)
Once #6 is merged to epic-1 branch:

| Story | Claude | Estimated Time | Dependencies |
|-------|--------|----------------|--------------|
| #11 Storage Trait | Claude 1 | 2-3 hours | #6 complete |
| #7 RunId/Namespace | Claude 2 | 3-4 hours | #6 complete |
| #8 Key/TypeTag | Claude 3 | 4-5 hours | #6 complete |
| #9 Value/VersionedValue | Claude 4 | 4-5 hours | #6 complete |

### Phase 3: Error Types (After core types)
- Story #10: Error types (Any Claude) - 2 hours - Depends on #7-9

**Total Epic 1 Time**: ~8-10 hours with parallelization (vs. 20+ hours sequential)

## Epic 2 Parallelization Plan

**Story #12 MUST complete first** (UnifiedStore), then some parallelization possible.

### Phase 1: Foundation (Sequential)
- Story #12: UnifiedStore (Claude 1) - **BLOCKS EPIC 2**

### Phase 2: Extensions (3 Claudes in parallel)
Once #12 is merged:

| Story | Claude | Estimated Time | Dependencies |
|-------|--------|----------------|--------------|
| #13 Secondary Indices | Claude 1 | 4-5 hours | #12 complete |
| #14 TTL Index | Claude 2 | 3-4 hours | #12 complete |
| #15 ClonedSnapshotView | Claude 3 | 4-5 hours | #12 complete |

### Phase 3: Testing (After extensions)
- Story #16: Storage Tests (Claude 4) - 4-5 hours - Depends on #12-15

**Total Epic 2 Time**: ~12-15 hours with parallelization (vs. 25+ hours sequential)

## Epic 3 Parallelization Plan

Good parallelization opportunities after #17.

### Phase 1: Entry Types (Sequential)
- Story #17: WAL Entry Types (Claude 1) - **BLOCKS EPIC 3**

### Phase 2: Implementation (3 Claudes in parallel)
Once #17 is merged:

| Story | Claude | Estimated Time | Dependencies |
|-------|--------|----------------|--------------|
| #18 Encoding/Decoding | Claude 2 | 4-5 hours | #17 complete |
| #19 File Operations | Claude 3 | 5-6 hours | #17 complete |
| #21 CRC/Checksums | Claude 4 | 3-4 hours | #17 complete |

### Phase 3: Durability (Sequential)
- Story #20: Durability Modes (Claude 1) - 3-4 hours - Depends on #19

### Phase 4: Testing (After all)
- Story #22: Corruption Tests (Claude 2) - 4-5 hours - Depends on #18-21

**Total Epic 3 Time**: ~15-18 hours with parallelization (vs. 28+ hours sequential)

## Communication Protocol

### Before Starting Work

**Post in issue comment:**
```
@anibjoshi I'm starting work on Story #7 (RunId/Namespace).

Branch: epic-1-story-7-runid-namespace
ETA: 3-4 hours
Dependencies: Waiting for #6 to merge to epic-1
```

### When Blocked

**Post in blocking issue:**
```
@anibjoshi Blocked on Story #7. I need the Storage trait definition
from #11 to complete my tests.

Can I assume the trait will have these methods?
- get(&Key) -> Option<VersionedValue>
- put(Key, Value) -> Result<u64>
- delete(&Key) -> Result<()>

Or should I wait for #11 to merge?
```

### When Complete

**Post in issue:**
```
@anibjoshi Story #7 complete!

✅ PR created: #XYZ
✅ All tests passing
✅ Ready for review

Next story: I can work on #10 (Error types) if available.
```

## Merge Coordination

### Epic Branch Merges (Critical!)

When merging a story to epic branch:

1. **Check for conflicts** with other merged stories
2. **Run full test suite** after merge (not just your story tests!)
3. **Update this document** if your work affects other stories

**Example conflict scenario:**

- Claude 1 merges #7 (defines RunId in core/src/types.rs)
- Claude 2 merges #8 (also modifies core/src/types.rs for Key)
- **Conflict!** Both modified same file

**Resolution:**
- Claude 2 (or whoever merges second) must:
  ```bash
  git checkout epic-1-story-8-key-typetag
  git pull origin epic-1-workspace-core-types
  # Resolve conflicts (both RunId and Key should coexist)
  git add core/src/types.rs
  git commit -m "Merge latest epic-1 changes (RunId from #7)"
  git push
  ```

## Dependency Tracking

### Story Dependencies (Critical Paths)

**Epic 1:**
```
#6 (Workspace)
  └─> #7, #8, #9, #11 (parallel)
       └─> #10 (Error types - needs all)
```

**Epic 2:**
```
#12 (UnifiedStore)
  └─> #13, #14, #15 (parallel)
       └─> #16 (Tests - needs all)
```

**Epic 3:**
```
#17 (WAL Entries)
  └─> #18, #19, #21 (parallel)
       └─> #20 (needs #19)
            └─> #22 (Tests - needs all)
```

**Epic 4:** (Mostly sequential)
```
#23 (WAL Replay)
  └─> #24 (Incomplete Txns)
       └─> #25 (Database::open)
            └─> #26, #27 (parallel tests)
```

**Epic 5:** (Some parallelization)
```
#28 (Database Struct)
  └─> #29, #30 (parallel)
       └─> #31 (needs #30)
            └─> #32 (Integration test - needs all)
```

## File Ownership (Reduce Conflicts)

To minimize merge conflicts, stories should primarily modify these files:

| Story | Primary Files | May Touch |
|-------|--------------|-----------|
| #6 | Cargo.toml, crate manifests | None |
| #7 | core/src/types.rs (RunId, Namespace) | core/src/lib.rs |
| #8 | core/src/types.rs (Key, TypeTag) | core/src/lib.rs |
| #9 | core/src/value.rs | core/src/lib.rs |
| #10 | core/src/error.rs | All crates (error usage) |
| #11 | core/src/traits.rs | core/src/lib.rs |
| #12 | storage/src/unified.rs | storage/src/lib.rs |
| #13 | storage/src/index.rs | storage/src/unified.rs |
| #14 | storage/src/ttl.rs | storage/src/unified.rs |
| #15 | storage/src/snapshot.rs | storage/src/lib.rs |
| #16 | storage/tests/* | None |

**Conflict Zones** (expect merges):
- `core/src/types.rs` - Stories #7, #8 will conflict
- `storage/src/unified.rs` - Stories #13, #14 may conflict
- Cargo.toml files - Most stories add dependencies

## Tips for Smooth Parallelization

### Do's ✅
1. **Pull from epic branch** before starting work each day
2. **Commit frequently** with clear messages
3. **Run tests locally** before pushing
4. **Communicate dependencies** in issue comments
5. **Merge quickly** - don't let PRs sit for days
6. **Rebase on conflicts** rather than merge commits

### Don'ts ❌
1. **Don't work on same file** as another Claude (check assignments!)
2. **Don't merge to wrong branch** (story → epic, NOT story → develop)
3. **Don't skip CI checks** (format, clippy, tests)
4. **Don't leave broken tests** in epic branch
5. **Don't assume API** - ask if you need types from unmerged story
6. **Don't force push** to epic branches (use --force-with-lease)

## Quick Commands

### Check what others are working on:
```bash
gh pr list --base epic-1-workspace-core-types
```

### See merged stories in epic:
```bash
git log origin/epic-1-workspace-core-types --oneline
```

### See who's blocked:
```bash
gh issue list --label "blocked"
```

### Claim a story:
```bash
# Add comment to issue
gh issue comment <issue-number> --body "I'm starting work on this. ETA: 3 hours"
```

## Example Coordination Session

**Day 1, Morning:**

```
[Claude 1 starts]
- Posts: "Starting #6 (Workspace), ETA 1 hour"
- Creates branch: epic-1-story-6-cargo-workspace
- After 1 hour, merges to epic-1-workspace-core-types
- Posts: "#6 complete! Epic branch ready for #7-11"

[Claude 2, 3, 4 see notification]
- Claude 2: "Starting #7 (RunId), ETA 3 hours"
- Claude 3: "Starting #8 (Key), ETA 4 hours"
- Claude 4: "Starting #9 (Value), ETA 4 hours"
- Claude 1: "Starting #11 (Traits), ETA 2 hours"

[All work in parallel, no conflicts (different files)]
```

**Day 1, Afternoon:**

```
[Claude 1 finishes #11 first]
- Merges to epic-1
- Epic branch now has: #6, #11

[Claude 2 finishes #7]
- Pulls epic-1 (gets #11)
- Merges to epic-1
- Epic branch now has: #6, #11, #7

[Claude 3 finishes #8]
- Pulls epic-1 (gets #11, #7)
- CONFLICT in core/src/types.rs (both #7 and #8 modified)
- Resolves: both RunId and Key should coexist
- Merges to epic-1

[Claude 4 finishes #9]
- Different file (value.rs), no conflict
- Merges to epic-1

[Epic 1 ready for #10]
- Claude 2: "Starting #10 (Errors), ETA 2 hours"
```

**Day 1, End:**
```
Epic 1 complete! (6 stories, ~10 hours wall time with 4 Claudes)
Ready to start Epic 2 tomorrow.
```

## Emergency Procedures

### Epic branch is broken (tests fail):
1. Identify which merge broke it (git bisect)
2. Revert the merge: `git revert <commit>`
3. Notify Claude who merged
4. Fix in story branch, re-merge

### Two Claudes accidentally work on same story:
1. Compare branches (who started first?)
2. First Claude continues
3. Second Claude picks different story from backlog

### Blocked on unmerged story:
1. Check if you can mock/stub the dependency
2. Ask in issue if you can assume API shape
3. If critical, wait for merge (work on different story)

---

**Remember**: Communication is key! When in doubt, post in issue comments.
