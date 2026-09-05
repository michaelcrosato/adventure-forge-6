#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
PATCH_PATH="$REPO_DIR/tools/mutants/nondeterminism-env.patch"
BOUNDARY_PATCH_PATH="$REPO_DIR/tools/mutants/boundary-env.patch"
HASH_ENTROPY_PATCH_PATH="$REPO_DIR/tools/mutants/hash-entropy-env.patch"
MEMORY_PROSE_PATCH_PATH="$REPO_DIR/tools/mutants/memory-prose-env.patch"
MANIFEST_PATCH_PATH="$REPO_DIR/tools/mutants/manifest-env.patch"
RECIPE_PATCH_PATH="$REPO_DIR/tools/mutants/recipe-env.patch"
DEFERRED_PATCH_PATH="$REPO_DIR/tools/mutants/deferred-env.patch"
SALVAGE_PATCH_PATH="$REPO_DIR/tools/mutants/salvage-env.patch"
COLLATERAL_PATCH_PATH="$REPO_DIR/tools/mutants/collateral-env.patch"
MUTANT_TMP_PREFIX="${TMPDIR:-/tmp}/forge-nondeterminism-mutants."
MUTANT_WORKSPACE=""
MUTANT_SELECTORS=(
    FORGE_MUTANT_ACTION_ORDER
    FORGE_MUTANT_FRONTIER_ORDER
    FORGE_MUTANT_PAGE_ORDER
    FORGE_MUTANT_PROCESS_RECEIPT
    FORGE_MUTANT_STALE_ACTION
    FORGE_MUTANT_SUPPLY_LEAK
    FORGE_MUTANT_HASH_CANONICALIZATION
    FORGE_MUTANT_ENTROPY
    FORGE_MUTANT_REMOTE_MEMORY
    FORGE_MUTANT_PROSE
    FORGE_MUTANT_MANIFEST_INPUT
    FORGE_MUTANT_RECIPE_CONSUMPTION
    FORGE_MUTANT_RECIPE_OUTPUT
    FORGE_MUTANT_DEFERRED_ABSOLUTE_TIME
    FORGE_MUTANT_DEFERRED_REMOTE_PAUSE
    FORGE_MUTANT_SALVAGE_CHANCE
    FORGE_MUTANT_COLLATERAL_PAYMENT
    FORGE_MUTANT_COLLATERAL_FUEL
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

for command in cargo cat cp grep mkdir mktemp mv patch rm sed; do
    command -v "$command" >/dev/null 2>&1 || fail "required command is unavailable: $command"
done
[[ -f "$PATCH_PATH" ]] || fail "mutant patch is missing"
[[ -f "$BOUNDARY_PATCH_PATH" ]] || fail "boundary mutant patch is missing"
for patch_path in "$HASH_ENTROPY_PATCH_PATH" "$MEMORY_PROSE_PATCH_PATH" "$MANIFEST_PATCH_PATH" "$RECIPE_PATCH_PATH" "$DEFERRED_PATCH_PATH" "$SALVAGE_PATCH_PATH" "$COLLATERAL_PATCH_PATH"; do
    [[ -f "$patch_path" ]] || fail "mutation patch is missing: $patch_path"
done

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
for patch_path in "$HASH_ENTROPY_PATCH_PATH" "$MEMORY_PROSE_PATCH_PATH" "$MANIFEST_PATCH_PATH" "$RECIPE_PATCH_PATH" "$DEFERRED_PATCH_PATH" "$SALVAGE_PATCH_PATH" "$COLLATERAL_PATCH_PATH"; do
    patch --directory="$MUTANT_WORKSPACE" --strip=1 --batch --forward \
        --input="$patch_path" >/dev/null || fail "could not apply mutation patch: $patch_path"
done

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

# Regenerate the crawl and its separately checked canonical seed witness
# with the patched source and every selector absent. Both carry build-bound
# lineage. This neutralizes identity drift so only activated behavior can
# kill a mutant; the clean-process test retains its exact seed assertions.
BASELINE_HOLD="$MUTANT_WORKSPACE/evidence/witnesses/m1-outcome-hold-market.json.next"
env "${MUTANT_UNSET_ARGS[@]}" "$MUTANT_VERIFIER" emit m1-outcome-hold-market >"$BASELINE_HOLD"
env "${MUTANT_UNSET_ARGS[@]}" "$MUTANT_VERIFIER" check "$BASELINE_HOLD" >/dev/null
mv -f -- "$BASELINE_HOLD" "$MUTANT_WORKSPACE/evidence/witnesses/m1-outcome-hold-market.json"
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
            cargo test --locked -p forge-verify --test "$test_target" "$test_name" -- --exact --nocapture
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
run_neutral_test "hash_entropy" "canonical_hash_preserves_order_and_authoritative_inputs"
run_neutral_test "hash_entropy" "entropy_known_answers_survive_canonical_actions_and_resume"
run_neutral_test "memory_prose" "remote_npc_memory_survives_movement_save_replay_and_return"
run_neutral_test "memory_prose" "production_prose_rejects_sentence_over_eighteen_words"
run_neutral_test "manifest" "generated_manifest_matches_independent_input_digests"
run_neutral_test "recipes" "production_recipe_consumes_owned_inputs_once"
run_neutral_test "recipes" "production_recipe_produces_exact_finished_quantity"
run_neutral_test "deferred_events" "production_deferred_deadlines_use_ignition_world_time"
run_neutral_test "deferred_events" "production_deferred_spoil_consumes_claim_away_from_kiln"

run_neutral_test "salvage_entropy_boundary" "production_salvage_chance_respects_strict_75_percent_boundary"
run_neutral_test "collateral_boundary" "production_collateral_purchase_charges_exact_four_coins"
run_neutral_test "collateral_boundary" "production_collateral_settlement_deposits_the_exact_fuel_lot"

# Check actual incremental build sensitivity in this disposable copy. Updating
# an existing source and adding an unreferenced source must both change the
# manifest AND compiled production build; restoring each must restore both.
# These probes also catch missing rerun directives, without mutating the repo.
manifest_commitments() {
    local line
    line="$(sed -n '/^MANIFEST_PROBE /p' "$MUTANT_WORKSPACE/neutral-manifest-generated_manifest_matches_independent_input_digests.log")"
    [[ "$line" =~ ^MANIFEST_PROBE\ [a-f0-9]{64}\ [a-f0-9]{64}$ ]] || fail "missing exact manifest commitments"
    printf '%s\n' "$line"
}

assert_changed_commitments() {
    local changed
    changed="$(manifest_commitments)"
    local marker old_manifest old_build new_manifest new_build
    read -r marker old_manifest old_build <<<"$ORIGINAL_COMMITMENTS"
    read -r marker new_manifest new_build <<<"$changed"
    [[ "$old_manifest" != "$new_manifest" && "$old_build" != "$new_build" ]] || \
        fail "authoritative input change did not change both manifest and game build"
}

ORIGINAL_COMMITMENTS="$(manifest_commitments)"
PROBE_SOURCE="$MUTANT_WORKSPACE/crates/forge-kernel/src/entropy.rs"
cp -- "$PROBE_SOURCE" "$MUTANT_WORKSPACE/entropy.rs.original"
cat >>"$PROBE_SOURCE" <<'EOF'

// Disposable manifest sensitivity probe: changed source bytes.
EOF
run_neutral_test "manifest" "generated_manifest_matches_independent_input_digests"
assert_changed_commitments
cp -- "$MUTANT_WORKSPACE/entropy.rs.original" "$PROBE_SOURCE"
run_neutral_test "manifest" "generated_manifest_matches_independent_input_digests"
[[ "$(manifest_commitments)" == "$ORIGINAL_COMMITMENTS" ]] || fail "restored source changed commitments"

ADDED_SOURCE="$MUTANT_WORKSPACE/crates/forge-kernel/src/manifest_sensitivity_probe.rs"
[[ ! -e "$ADDED_SOURCE" ]] || fail "manifest probe path already exists"
cat >"$ADDED_SOURCE" <<'EOF'
// Disposable manifest sensitivity probe: newly discovered source bytes.
EOF
run_neutral_test "manifest" "generated_manifest_matches_independent_input_digests"
assert_changed_commitments
rm -- "$ADDED_SOURCE"
run_neutral_test "manifest" "generated_manifest_matches_independent_input_digests"
[[ "$(manifest_commitments)" == "$ORIGINAL_COMMITMENTS" ]] || fail "removed source changed commitments"
echo "PASS manifest sensitivity: edit/add inputs change both commitments; restore/remove reproduce both"

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
assert_mutant_killed \
    "canonical-hash" "FORGE_MUTANT_HASH_CANONICALIZATION" \
    "hash_entropy" "canonical_hash_preserves_order_and_authoritative_inputs" \
    'canonical hash lost ordered input'
assert_mutant_killed \
    "entropy-cursor" "FORGE_MUTANT_ENTROPY" \
    "hash_entropy" "entropy_known_answers_survive_canonical_actions_and_resume" \
    'entropy seed cursor sequence changed'
assert_mutant_killed \
    "remote-memory" "FORGE_MUTANT_REMOTE_MEMORY" \
    "memory_prose" "remote_npc_memory_survives_movement_save_replay_and_return" \
    'remote NPC memory was lost or changed'
assert_mutant_killed \
    "prose-admission" "FORGE_MUTANT_PROSE" \
    "memory_prose" "production_prose_rejects_sentence_over_eighteen_words" \
    'production prose admitted a nineteen-word sentence'
assert_mutant_killed \
    "recipe-consumption" "FORGE_MUTANT_RECIPE_CONSUMPTION" \
    "recipes" "production_recipe_consumes_owned_inputs_once" \
    'recipe consumption bypassed owned inputs'
assert_mutant_killed \
    "recipe-output" "FORGE_MUTANT_RECIPE_OUTPUT" \
    "recipes" "production_recipe_produces_exact_finished_quantity" \
    'recipe output duplicated finished goods'
assert_mutant_killed \
    "deferred-relative-time" "FORGE_MUTANT_DEFERRED_ABSOLUTE_TIME" \
    "deferred_events" "production_deferred_deadlines_use_ignition_world_time" \
    'deferred deadlines ignored ignition world time'
assert_mutant_killed \
    "deferred-remote-time" "FORGE_MUTANT_DEFERRED_REMOTE_PAUSE" \
    "deferred_events" "production_deferred_spoil_consumes_claim_away_from_kiln" \
    'deferred batch paused outside its kiln'
assert_mutant_killed \
    "manifest-input" "FORGE_MUTANT_MANIFEST_INPUT" \
    "manifest" "generated_manifest_matches_independent_input_digests" \
    'manifest omitted authoritative kernel input'
assert_mutant_killed \
    "salvage-chance" "FORGE_MUTANT_SALVAGE_CHANCE" \
    "salvage_entropy_boundary" "production_salvage_chance_respects_strict_75_percent_boundary" \
    'salvage chance ignored its 75 percent boundary'
assert_mutant_killed \
    "collateral-payment" "FORGE_MUTANT_COLLATERAL_PAYMENT" \
    "collateral_boundary" "production_collateral_purchase_charges_exact_four_coins" \
    'collateral purchase did not charge four coins'
assert_mutant_killed \
    "collateral-fuel" "FORGE_MUTANT_COLLATERAL_FUEL" \
    "collateral_boundary" "production_collateral_settlement_deposits_the_exact_fuel_lot" \
    'collateral settlement retained its fuel lot'
echo "PASS mutation corpus: 18/18 killed after 15 neutral controls and 4 incremental manifest probes"
