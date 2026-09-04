#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
PATCH_PATH="$REPO_DIR/tools/mutants/nondeterminism-env.patch"
MUTANT_WORKSPACE=""

fail() {
    echo "nondeterminism mutant verification failed: $*" >&2
    exit 1
}

cleanup() {
    if [[ -n "$MUTANT_WORKSPACE" && -d "$MUTANT_WORKSPACE" ]]; then
        rm -rf -- "$MUTANT_WORKSPACE"
    fi
}
trap cleanup EXIT INT TERM

for command in cargo cp grep mkdir mktemp mv patch rm sed; do
    command -v "$command" >/dev/null 2>&1 || fail "required command is unavailable: $command"
done
[[ -f "$PATCH_PATH" ]] || fail "mutant patch is missing"

MUTANT_WORKSPACE="$(mktemp -d "${TMPDIR:-/tmp}/forge-nondeterminism-mutants.XXXXXX")"
cp -- \
    "$REPO_DIR/Cargo.toml" \
    "$REPO_DIR/Cargo.lock" \
    "$REPO_DIR/rust-toolchain.toml" \
    "$MUTANT_WORKSPACE/"
for directory in content crates evidence; do
    cp -R -- "$REPO_DIR/$directory" "$MUTANT_WORKSPACE/$directory"
done

patch \
    --directory="$MUTANT_WORKSPACE" \
    --strip=1 \
    --batch \
    --forward \
    --input="$PATCH_PATH" >/dev/null || fail "could not apply the reviewed mutant patch"

unset FORGE_MUTANT_ACTION_ORDER FORGE_MUTANT_FRONTIER_ORDER FORGE_MUTANT_PAGE_ORDER FORGE_MUTANT_PROCESS_RECEIPT
MUTANT_TARGET="$REPO_DIR/target/nondeterminism-mutants"
export CARGO_TARGET_DIR="$MUTANT_TARGET"
export CARGO_INCREMENTAL=0

(
    cd -- "$MUTANT_WORKSPACE"
    cargo build --locked --quiet -p forge-verify --bin forge-verify
)
MUTANT_VERIFIER="$MUTANT_TARGET/debug/forge-verify"
[[ -x "$MUTANT_VERIFIER" ]] || fail "mutated verifier binary was not built"

# Regenerate the crawl fixture with the mutated source but neither ambient
# selector. This neutralizes build/verifier identity changes as a reason for
# later failures, so only the activated behavior change can kill a mutant.
BASELINE_REPORT="$MUTANT_WORKSPACE/evidence/crawls/split-tide.json.next"
env \
    -u FORGE_MUTANT_ACTION_ORDER \
    -u FORGE_MUTANT_FRONTIER_ORDER \
    -u FORGE_MUTANT_PAGE_ORDER \
    -u FORGE_MUTANT_PROCESS_RECEIPT \
    "$MUTANT_VERIFIER" crawl >"$BASELINE_REPORT"
mv -f -- "$BASELINE_REPORT" "$MUTANT_WORKSPACE/evidence/crawls/split-tide.json"

TEST_NAME="clean_process_crawls_match_each_other_and_checked_report"
CONTROL_LOG="$MUTANT_WORKSPACE/control.log"
if ! (
    cd -- "$MUTANT_WORKSPACE"
    env \
        -u FORGE_MUTANT_ACTION_ORDER \
        -u FORGE_MUTANT_FRONTIER_ORDER \
        -u FORGE_MUTANT_PAGE_ORDER \
        -u FORGE_MUTANT_PROCESS_RECEIPT \
        cargo test --locked -p forge-verify --test clean_process "$TEST_NAME" -- --exact
) >"$CONTROL_LOG" 2>&1; then
    sed -n '1,220p' "$CONTROL_LOG" >&2
    fail "neutral control did not pass"
fi

assert_mutant_killed() {
    local mutant_name="$1"
    local selector="$2"
    local log_path="$MUTANT_WORKSPACE/$mutant_name.log"
    local status

    set +e
    (
        cd -- "$MUTANT_WORKSPACE"
        env \
            -u FORGE_MUTANT_ACTION_ORDER \
            -u FORGE_MUTANT_FRONTIER_ORDER \
            -u FORGE_MUTANT_PAGE_ORDER \
            -u FORGE_MUTANT_PROCESS_RECEIPT \
            "$selector=enabled" \
            cargo test --locked -p forge-verify --test clean_process "$TEST_NAME" -- --exact
    ) >"$log_path" 2>&1
    status=$?
    set -e

    if [[ "$status" -eq 0 ]]; then
        fail "$mutant_name survived the clean-process crawl gate"
    fi
    if ! grep -Eq \
        'first crawl failed|second crawl failed|clean-process crawls diverged|checked crawl report is stale' \
        "$log_path"; then
        sed -n '1,220p' "$log_path" >&2
        fail "$mutant_name failed for an unrelated reason"
    fi
    echo "KILLED $mutant_name"
}

assert_mutant_killed "ambient-action-order" "FORGE_MUTANT_ACTION_ORDER"
assert_mutant_killed "ambient-page-order" "FORGE_MUTANT_PAGE_ORDER"
assert_mutant_killed "ambient-frontier-order" "FORGE_MUTANT_FRONTIER_ORDER"
assert_mutant_killed "process-receipt" "FORGE_MUTANT_PROCESS_RECEIPT"
echo "PASS nondeterminism mutants: 4/4 killed after a passing neutral control"
