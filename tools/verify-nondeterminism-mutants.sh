#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
PATCH_PATH="$REPO_DIR/tools/mutants/nondeterminism-env.patch"
BOUNDARY_PATCH_PATH="$REPO_DIR/tools/mutants/boundary-env.patch"
MUTANT_TMP_PREFIX="${TMPDIR:-/tmp}/forge-nondeterminism-mutants."
MUTANT_WORKSPACE=""
MUTANT_SELECTORS=(
    FORGE_MUTANT_ACTION_ORDER
    FORGE_MUTANT_FRONTIER_ORDER
    FORGE_MUTANT_PAGE_ORDER
    FORGE_MUTANT_PROCESS_RECEIPT
    FORGE_MUTANT_STALE_ACTION
    FORGE_MUTANT_SUPPLY_LEAK
)
MUTANT_UNSET_ARGS=()
for selector in "${MUTANT_SELECTORS[@]}"; do
    MUTANT_UNSET_ARGS+=(-u "$selector")
done

fail() {
    echo "nondeterminism mutant verification failed: $*" >&2
    exit 1
}

cleanup() {
    if [[ -n "$MUTANT_WORKSPACE" && -d "$MUTANT_WORKSPACE" ]]; then
        case "$MUTANT_WORKSPACE" in
            "$MUTANT_TMP_PREFIX"*) rm -rf -- "$MUTANT_WORKSPACE" ;;
            *)
                echo "refusing to remove unexpected mutant workspace: $MUTANT_WORKSPACE" >&2
                ;;
        esac
    fi
}
trap cleanup EXIT INT TERM

for command in cargo cp grep mkdir mktemp mv patch rm sed; do
    command -v "$command" >/dev/null 2>&1 || fail "required command is unavailable: $command"
done
[[ -f "$PATCH_PATH" ]] || fail "mutant patch is missing"
[[ -f "$BOUNDARY_PATCH_PATH" ]] || fail "boundary mutant patch is missing"

MUTANT_WORKSPACE="$(mktemp -d "${MUTANT_TMP_PREFIX}XXXXXX")"
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
patch \
    --directory="$MUTANT_WORKSPACE" \
    --strip=1 \
    --batch \
    --forward \
    --input="$BOUNDARY_PATCH_PATH" >/dev/null || fail "could not apply the boundary mutant patch"

unset "${MUTANT_SELECTORS[@]}"
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
env "${MUTANT_UNSET_ARGS[@]}" "$MUTANT_VERIFIER" crawl >"$BASELINE_REPORT"
mv -f -- "$BASELINE_REPORT" "$MUTANT_WORKSPACE/evidence/crawls/split-tide.json"

run_neutral_test() {
    local test_target="$1"
    local test_name="$2"
    local log_path="$MUTANT_WORKSPACE/neutral-$test_target-$test_name.log"

    if ! (
        cd -- "$MUTANT_WORKSPACE"
        env "${MUTANT_UNSET_ARGS[@]}" \
            cargo test --locked -p forge-verify --test "$test_target" "$test_name" -- --exact
    ) >"$log_path" 2>&1; then
        sed -n '1,220p' "$log_path" >&2
        fail "neutral control did not pass: $test_target/$test_name"
    fi
    if ! grep -Fq 'running 1 test' "$log_path"; then
        sed -n '1,220p' "$log_path" >&2
        fail "neutral control did not run exactly one test: $test_target/$test_name"
    fi
}

run_neutral_test "clean_process" "clean_process_crawls_match_each_other_and_checked_report"
run_neutral_test "boundaries" "stale_action_rejection_preserves_session"
run_neutral_test "boundaries" "public_observation_excludes_npc_stock"

assert_mutant_killed() {
    local mutant_name="$1"
    local selector="$2"
    local test_target="$3"
    local test_name="$4"
    local expected_pattern="$5"
    local log_path="$MUTANT_WORKSPACE/$mutant_name.log"
    local status

    set +e
    (
        cd -- "$MUTANT_WORKSPACE"
        env "${MUTANT_UNSET_ARGS[@]}" \
            "$selector=enabled" \
            cargo test --locked -p forge-verify --test "$test_target" "$test_name" -- --exact
    ) >"$log_path" 2>&1
    status=$?
    set -e

    if [[ "$status" -eq 0 ]]; then
        fail "$mutant_name survived its boundary test"
    fi
    if ! grep -Fq 'running 1 test' "$log_path"; then
        sed -n '1,220p' "$log_path" >&2
        fail "$mutant_name did not run exactly one test"
    fi
    if ! grep -Eq "$expected_pattern" "$log_path"; then
        sed -n '1,220p' "$log_path" >&2
        fail "$mutant_name failed for an unrelated reason"
    fi
    echo "KILLED $mutant_name"
}

CRAWL_TEST_TARGET="clean_process"
CRAWL_TEST_NAME="clean_process_crawls_match_each_other_and_checked_report"
CRAWL_FAILURE_PATTERN='first crawl failed|second crawl failed|clean-process crawls diverged|checked crawl report is stale'
assert_mutant_killed \
    "ambient-action-order" "FORGE_MUTANT_ACTION_ORDER" \
    "$CRAWL_TEST_TARGET" "$CRAWL_TEST_NAME" "$CRAWL_FAILURE_PATTERN"
assert_mutant_killed \
    "ambient-page-order" "FORGE_MUTANT_PAGE_ORDER" \
    "$CRAWL_TEST_TARGET" "$CRAWL_TEST_NAME" "$CRAWL_FAILURE_PATTERN"
assert_mutant_killed \
    "ambient-frontier-order" "FORGE_MUTANT_FRONTIER_ORDER" \
    "$CRAWL_TEST_TARGET" "$CRAWL_TEST_NAME" "$CRAWL_FAILURE_PATTERN"
assert_mutant_killed \
    "process-receipt" "FORGE_MUTANT_PROCESS_RECEIPT" \
    "$CRAWL_TEST_TARGET" "$CRAWL_TEST_NAME" "$CRAWL_FAILURE_PATTERN"
assert_mutant_killed \
    "stale-action" "FORGE_MUTANT_STALE_ACTION" \
    "boundaries" "stale_action_rejection_preserves_session" \
    'stale action bypassed state binding'
assert_mutant_killed \
    "npc-stock-leak" "FORGE_MUTANT_SUPPLY_LEAK" \
    "boundaries" "public_observation_excludes_npc_stock" \
    'public observation leaked NPC stock'
echo "PASS mutation corpus: 4 nondeterminism + stale-action + NPC-stock = 6/6 killed after 3 neutral controls"
