# TDD Lessons Learned

This document captures critical lessons learned about Test-Driven Development failures and how to prevent them.

---

## Lesson 1: Never Modify Tests to Hide Bugs (Issue #51)

**Date**: 2026-01-11
**Epic**: Epic 3 - WAL Implementation
**Story**: Story #22 - Corruption simulation tests
**Severity**: CRITICAL

### What Happened

During Story #22 (corruption simulation testing), a test discovered a **critical bug**: the WAL decoder panicked with integer underflow when encountering zero-length entries.

**The Correct Approach** (TDD):
1. Test finds bug → Write test that exposes panic
2. Fix the bug → Add validation in `decode_entry()`
3. Test passes → Verify fix works

**What Actually Happened** (TDD Failure):
1. Test finds bug → Test with zeros causes panic
2. **WRONG**: Changed test to use non-zero garbage to avoid the bug ❌
3. Test passes → Bug still exists in production code

### The Bug

**File**: `crates/durability/src/encoding.rs:172`

```rust
// BEFORE (panics on zero-length):
let payload_len = total_len - 1 - 4;  // Underflows when total_len < 5
```

**Root Cause**: No validation that `total_len >= 5` before subtraction.

**When it occurs**:
- Filesystem bugs cause trailing zeros
- Pre-allocation fills unused space with zeros
- Disk corruption zeros out data

### The Fix

```rust
// AFTER (graceful error):
// Validate minimum length before arithmetic (prevent underflow)
if total_len < 5 {
    return Err(Error::Corruption(format!(
        "offset {}: Invalid entry length {} (minimum is 5 bytes: type(1) + crc(4))",
        offset, total_len
    )));
}

let payload_len = total_len - 1 - 4;  // Safe now
```

### Why This Is Critical

**Tests are the safety net.** When a test finds a bug:

✅ **CORRECT**:
- Test exposes the bug
- Bug is fixed in implementation
- Test now passes
- Production code is safe

❌ **WRONG**:
- Test exposes the bug
- Test is modified to hide the bug
- Test now passes
- **Production code still has the bug!**

### Impact

- **Severity**: Medium-High (panic in production)
- **Scope**: Any WAL with trailing zeros would crash
- **Detection**: Would only be found in production or comprehensive review
- **Time to fix**: 5 minutes (if caught early)
- **Time wasted**: Hours (if tests were trusted without review)

### Prevention

Added to **EPIC_3_REVIEW.md** (Phase 3: Code Review):

1. **TDD Integrity Checks**:
   - Review git history for suspicious test changes
   - Look for red flags: "changed test", "workaround", "adjusted"
   - Verify tests expose bugs rather than hiding them
   - Check comments for "temporary fix" or "TODO: fix properly"

2. **Verification Commands**:
   ```bash
   git log --oneline --all -- 'crates/durability/tests/*.rs'
   git log -p --all -- '*test*.rs' | grep -B5 -A5 "workaround\|bypass\|skip"
   ```

3. **Review Policy**:
   - If TDD violations found → **REJECT epic**
   - Fix bugs properly
   - Restore correct tests
   - Re-review after fixes

### Key Takeaways

1. **Tests validate implementation** - If a test fails, the IMPLEMENTATION is wrong, not the test.

2. **Failing tests are valuable** - They found a bug! Don't silence them by changing the test.

3. **Code review is essential** - Even with 100% passing tests, code can be wrong if tests were weakened.

4. **TDD discipline matters** - Red → Green → Refactor, NOT Red → Change Test → Green.

5. **Document patterns** - When bugs are found, add them to the test suite as regression tests.

### References

- **Issue**: #51 - Fix decoder panic on zero-length entry
- **PR**: #52 - BUGFIX: Fix decoder panic on zero-length entry
- **Epic**: Epic 3 - WAL Implementation
- **Story**: Story #22 - Corruption simulation tests

---

## Best Practices Going Forward

### When a Test Fails

1. **Understand why it failed**
   - Read the error message carefully
   - Identify the root cause
   - Determine if it's a bug or a flaky test

2. **Fix the implementation**
   - Add validation, bounds checks, error handling
   - Don't modify the test unless it's genuinely wrong

3. **Add regression tests**
   - If the bug wasn't caught by existing tests, add a new test
   - Make the test case explicit and well-documented

4. **Verify the fix**
   - Run all tests
   - Check that the specific bug is now prevented
   - Ensure no new bugs were introduced

### When to Modify a Test

Tests should ONLY be modified when:

✅ **Legitimate reasons**:
- Test logic is incorrect (wrong assertion)
- Test is flaky due to timing/randomness
- API changed and test needs updating
- Test is testing the wrong thing

❌ **NEVER modify a test to**:
- Make it pass when implementation is buggy
- Work around a bug in production code
- Skip a failing assertion
- Use different data to avoid triggering a bug

### Code Review Checklist

When reviewing test changes:

- [ ] Why was this test modified?
- [ ] Was a bug found and then hidden?
- [ ] Is the change documented with a good reason?
- [ ] Does the modified test still validate the same behavior?
- [ ] If data changed, why? (Red flag if to avoid a bug)

---

## Template for Future Lessons

When a new TDD lesson is learned, add it here with:

1. **What Happened** - Describe the incident
2. **The Bug** - Show the buggy code
3. **The Fix** - Show the correct code
4. **Why This Is Critical** - Impact analysis
5. **Prevention** - How to prevent recurrence
6. **Key Takeaways** - Lessons learned
7. **References** - Links to issues, PRs, commits

---

**Last Updated**: 2026-01-11
**Maintainer**: Project Team
**Review Frequency**: After each epic completion
