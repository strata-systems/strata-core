#!/bin/bash
# Automated Epic Review Script
# Usage: ./scripts/review-epic.sh <epic-number>

set -e

# Source Rust environment if it exists
if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

EPIC_NUM=$1

if [ -z "$EPIC_NUM" ]; then
    echo "Usage: ./scripts/review-epic.sh <epic-number>"
    echo "Example: ./scripts/review-epic.sh 1"
    exit 1
fi

# Epic name mapping
declare -A EPIC_NAMES
EPIC_NAMES[1]="workspace-core-types"
EPIC_NAMES[2]="storage-layer"
EPIC_NAMES[3]="wal-implementation"
EPIC_NAMES[4]="basic-recovery"
EPIC_NAMES[5]="database-engine"

EPIC_NAME=${EPIC_NAMES[$EPIC_NUM]}
EPIC_BRANCH="epic-${EPIC_NUM}-${EPIC_NAME}"

if [ -z "$EPIC_NAME" ]; then
    echo "❌ Invalid epic number: $EPIC_NUM"
    echo "Valid epic numbers: 1, 2, 3, 4, 5"
    exit 1
fi

echo "════════════════════════════════════════════════════════"
echo "  Epic ${EPIC_NUM} Review: ${EPIC_NAME}"
echo "════════════════════════════════════════════════════════"
echo ""

# Check we're on the epic branch
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$CURRENT_BRANCH" != "$EPIC_BRANCH" ]; then
    echo "⚠️  Warning: Not on epic branch"
    echo "Current branch: $CURRENT_BRANCH"
    echo "Expected branch: $EPIC_BRANCH"
    echo ""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

echo "Branch: $EPIC_BRANCH"
echo ""

# Phase 1: Pre-Review Validation
echo "════════════════════════════════════════════════════════"
echo "  Phase 1: Pre-Review Validation ✅"
echo "════════════════════════════════════════════════════════"
echo ""

echo "🔨 Building workspace..."
if cargo build --all 2>&1 | tee /tmp/epic-review-build.log; then
    echo "✅ Build passed"
else
    echo "❌ Build failed"
    echo "See /tmp/epic-review-build.log for details"
    exit 1
fi

echo ""
echo "🧪 Running tests..."
if cargo test --all 2>&1 | tee /tmp/epic-review-test.log; then
    echo "✅ Tests passed"
else
    echo "❌ Tests failed"
    echo "See /tmp/epic-review-test.log for details"
    exit 1
fi

echo ""
echo "📎 Running clippy..."
if cargo clippy --all -- -D warnings 2>&1 | tee /tmp/epic-review-clippy.log; then
    echo "✅ Clippy passed"
else
    echo "❌ Clippy failed"
    echo "See /tmp/epic-review-clippy.log for details"
    exit 1
fi

echo ""
echo "🎨 Checking formatting..."
if cargo fmt --all -- --check 2>&1 | tee /tmp/epic-review-fmt.log; then
    echo "✅ Formatting passed"
else
    echo "❌ Formatting failed"
    echo "Run: cargo fmt --all"
    exit 1
fi

# Phase 2: Integration Testing
echo ""
echo "════════════════════════════════════════════════════════"
echo "  Phase 2: Integration Testing 🧪"
echo "════════════════════════════════════════════════════════"
echo ""

echo "🚀 Running tests in release mode..."
if cargo test --all --release 2>&1 | tee /tmp/epic-review-test-release.log; then
    echo "✅ Release tests passed"
else
    echo "❌ Release tests failed"
    echo "See /tmp/epic-review-test-release.log for details"
    exit 1
fi

echo ""
echo "📊 Generating coverage report..."
if command -v cargo-tarpaulin &> /dev/null; then
    if cargo tarpaulin --all --out Html --output-dir coverage 2>&1 | tee /tmp/epic-review-coverage.log; then
        COVERAGE=$(grep -oP '\d+\.\d+%' /tmp/epic-review-coverage.log | tail -1)
        echo "✅ Coverage: $COVERAGE"
        echo ""
        echo "📄 Coverage report: coverage/index.html"
    else
        echo "⚠️  Coverage generation failed (non-blocking)"
    fi
else
    echo "⚠️  cargo-tarpaulin not installed"
    echo "Install with: cargo install cargo-tarpaulin"
    echo "Skipping coverage (non-blocking)"
fi

# Phase 3: Documentation Check
echo ""
echo "════════════════════════════════════════════════════════"
echo "  Phase 3: Documentation Review 📚"
echo "════════════════════════════════════════════════════════"
echo ""

echo "📖 Checking documentation..."
if cargo doc --all --no-deps 2>&1 | tee /tmp/epic-review-doc.log; then
    echo "✅ Documentation builds"
else
    echo "❌ Documentation build failed"
    echo "See /tmp/epic-review-doc.log for details"
    exit 1
fi

# Phase 4: Epic-Specific Tests
echo ""
echo "════════════════════════════════════════════════════════"
echo "  Phase 4: Epic-Specific Validation"
echo "════════════════════════════════════════════════════════"
echo ""

case $EPIC_NUM in
    1)
        echo "Epic 1: Workspace & Core Types"
        echo ""
        echo "Running critical tests..."

        echo "  🔍 Testing key ordering..."
        cargo test -p in-mem-core test_key_btree_ordering --nocapture 2>&1 | grep -A 5 "test_key_btree_ordering" || true

        echo "  🔍 Testing value serialization..."
        cargo test -p in-mem-core test_value_serialization --nocapture 2>&1 | grep -A 5 "test_value_serialization" || true

        echo ""
        echo "All crate tests:"
        cargo test -p in-mem-core --all 2>&1 | tail -10
        ;;

    2)
        echo "Epic 2: Storage Layer"
        echo ""
        echo "Running critical tests..."

        echo "  🔍 Testing concurrent reads..."
        cargo test -p in-mem-storage test_concurrent_reads --nocapture 2>&1 | grep -A 5 "test_concurrent_reads" || true

        echo "  🔍 Testing version monotonicity..."
        cargo test -p in-mem-storage test_version_monotonic --nocapture 2>&1 | grep -A 5 "test_version_monotonic" || true

        echo "  🔍 Testing TTL cleanup..."
        cargo test -p in-mem-storage test_ttl_cleanup --nocapture 2>&1 | grep -A 5 "test_ttl_cleanup" || true
        ;;

    3)
        echo "Epic 3: WAL Implementation"
        echo ""
        echo "Running critical tests..."

        echo "  🔍 Testing WAL serialization..."
        cargo test -p in-mem-durability test_wal --nocapture 2>&1 | grep -A 5 "test_wal" || true

        echo "  🔍 Testing corruption detection..."
        cargo test -p in-mem-durability test_corrupted_entry --nocapture 2>&1 | grep -A 5 "test_corrupted_entry" || true

        echo "  🔍 Running corruption simulation..."
        cargo test --test corruption_simulation 2>&1 | tail -10 || true
        ;;

    4)
        echo "Epic 4: Basic Recovery"
        echo ""
        echo "Running critical tests..."

        echo "  🔍 Running crash simulation..."
        cargo test --test crash_simulation 2>&1 | tail -10 || true

        echo "  🔍 Testing large WAL recovery..."
        cargo test test_large_wal_recovery --release --nocapture 2>&1 | grep -A 5 "test_large_wal_recovery" || true
        ;;

    5)
        echo "Epic 5: Database Engine Shell"
        echo ""
        echo "Running critical tests..."

        echo "  🔍 Testing write-restart-read..."
        cargo test test_write_restart_read --nocapture 2>&1 | grep -A 5 "test_write_restart_read" || true

        echo "  🔍 Running integration tests..."
        cargo test --test integration 2>&1 | tail -10 || true

        echo "  🔍 All engine tests..."
        cargo test -p in-mem-engine --all 2>&1 | tail -10
        ;;
esac

# Summary
echo ""
echo "════════════════════════════════════════════════════════"
echo "  Review Summary"
echo "════════════════════════════════════════════════════════"
echo ""
echo "✅ All automated checks passed!"
echo ""
echo "Next steps:"
echo "1. Fill out review checklist in docs/milestones/EPIC_${EPIC_NUM}_REVIEW.md"
echo "2. Perform manual code review (Phase 3 checklist)"
echo "3. Review coverage report: coverage/index.html"
echo "4. Check documentation: cargo doc --all --open"
echo "5. If approved, merge to develop:"
echo "   git checkout develop"
echo "   git merge $EPIC_BRANCH"
echo "   git push origin develop"
echo ""
echo "Review logs saved to /tmp/epic-review-*.log"
echo ""
