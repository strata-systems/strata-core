# M2 Epic Prompt Header

**Copy this header to the top of every M2 epic prompt file (Epics 7-12).**

---

## 🔴 AUTHORITATIVE SPECIFICATION - READ THIS FIRST

**`docs/architecture/M2_TRANSACTION_SEMANTICS.md` is the GOSPEL for ALL M2 implementation.**

This is not a guideline. This is not a suggestion. This is the **LAW**.

### Rules for Every Story in Every Epic of M2:

1. **Every story MUST implement behavior EXACTLY as specified in the semantics document**
   - No "improvements" that deviate from the spec
   - No "simplifications" that change behavior
   - No "optimizations" that break guarantees

2. **If your code contradicts the spec, YOUR CODE IS WRONG**
   - The spec defines correct behavior
   - Fix the code, not the spec

3. **If your tests contradict the spec, YOUR TESTS ARE WRONG**
   - Tests must validate spec-compliant behavior
   - Never adjust tests to make broken code pass

4. **If the spec seems wrong or unclear:**
   - STOP implementation immediately
   - Raise the issue for discussion
   - Do NOT proceed with assumptions
   - Do NOT implement your own interpretation

5. **No breaking the spec for ANY reason:**
   - Not for "performance"
   - Not for "simplicity"
   - Not for "it's just an edge case"
   - Not for "we can fix it later"

### What the Spec Defines (Read Before Any M2 Work):

| Section | Content | You MUST Follow |
|---------|---------|-----------------|
| Section 1 | Isolation Level | **Snapshot Isolation, NOT Serializability** |
| Section 2 | Visibility Rules | What txns see/don't see/may see |
| Section 3 | Conflict Detection | When aborts happen, first-committer-wins |
| Section 4 | Implicit Transactions | How M1-style ops work in M2 |
| Section 5 | Replay Semantics | No re-validation, single-threaded |
| Section 6 | Version Semantics | Version 0 = never existed, tombstones |

### Before Starting ANY Story:

```bash
# 1. Read the full spec
cat docs/architecture/M2_TRANSACTION_SEMANTICS.md

# 2. Identify which sections apply to your story
# 3. Understand the EXACT behavior required
# 4. Implement EXACTLY that behavior
# 5. Write tests that validate spec compliance
```

**WARNING**: Code review will verify spec compliance. Non-compliant code will be rejected.

---

## 🔴 BRANCHING STRATEGY - READ THIS

### Branch Hierarchy
```
main                          ← Protected: only accepts merges from develop
  └── develop                 ← Integration branch for completed epics
       └── epic-N-name        ← Epic branch (base for all story PRs)
            └── epic-N-story-X-desc  ← Story branches
```

### Critical Rules

1. **Story PRs go to EPIC branch, NOT main**
   ```bash
   # CORRECT: PR base is epic branch
   gh pr create --base epic-7-transaction-semantics --head epic-7-story-83-validation

   # WRONG: Never PR directly to main
   gh pr create --base main --head epic-7-story-83-validation  # ❌ NEVER DO THIS
   ```

2. **Epic branches merge to develop** (after all stories complete)
   ```bash
   git checkout develop
   git merge --no-ff epic-7-transaction-semantics
   ```

3. **develop merges to main** (at milestone boundaries)
   ```bash
   git checkout main
   git merge --no-ff develop -m "M2: Complete"
   ```

4. **main is protected** - requires PR, no direct pushes

### The `complete-story.sh` Script
The script automatically uses the correct base branch:
```bash
./scripts/complete-story.sh 83  # Creates PR to epic-7-transaction-semantics
```

**If you manually create a PR, ALWAYS verify the base branch is the epic branch, not main.**

---

## 🔴 TDD METHODOLOGY

**CRITICAL TESTING RULE** (applies to EVERY story):

- **NEVER adjust tests to make them pass**
- If a test fails, the CODE must be fixed, not the test
- Tests define correct behavior - failed tests reveal bugs in implementation
- Only adjust a test if the test itself is incorrect (wrong assertion logic)
- Tests MUST validate spec-compliant behavior

---

## Tool Paths

Use fully qualified paths:
- Cargo: `~/.cargo/bin/cargo`
- GitHub CLI: `/opt/homebrew/bin/gh`

---

## Story Workflow

1. **Start story**: `./scripts/start-story.sh <epic> <story> <description>`
2. **Read spec**: `cat docs/architecture/M2_TRANSACTION_SEMANTICS.md`
3. **Write tests first** (TDD)
4. **Implement code** to pass tests
5. **Run validation**:
   ```bash
   ~/.cargo/bin/cargo test --all
   ~/.cargo/bin/cargo clippy --all -- -D warnings
   ~/.cargo/bin/cargo fmt --check
   ```
6. **Complete story**: `./scripts/complete-story.sh <story>`

---

*End of M2 Prompt Header - Epic-specific content follows below*
