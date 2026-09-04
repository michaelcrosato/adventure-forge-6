#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/live-blind-llm-playtest.sh"

[[ "$LIVE_DEFAULT_MODEL" == "gpt-5.6-luna" ]]
[[ "$LIVE_DEFAULT_REASONING_EFFORT" == "max" ]]

TEST_DIR="$(mktemp -d /tmp/forge-live-auth-test.XXXXXX)"
cleanup_test() {
    case "$TEST_DIR" in
        /tmp/forge-live-auth-test.*) rm -r -- "$TEST_DIR" ;;
    esac
}
trap cleanup_test EXIT INT TERM

FAKE_CODEX="$TEST_DIR/codex"
ENV_RECORD="$TEST_DIR/environment.txt"
# These single-quoted lines intentionally defer expansion to the fake executable.
# shellcheck disable=SC2016
printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    '[[ "${1-}" == "login" && "${2-}" == "status" ]]' \
    'if [[ -n "${OPENAI_API_KEY+x}" || -n "${CODEX_API_KEY+x}" ]]; then' \
    '    printf "key-present\n" >"$FAKE_ENV_RECORD"' \
    'else' \
    '    printf "key-absent\n" >"$FAKE_ENV_RECORD"' \
    'fi' \
    'printf "%s\n" "$FAKE_CODEX_STATUS"' \
    'exit "${FAKE_CODEX_EXIT:-0}"' \
    >"$FAKE_CODEX"
chmod 0700 "$FAKE_CODEX"

export OPENAI_API_KEY="must-not-be-used"
export CODEX_API_KEY="must-not-be-used-either"
export FAKE_ENV_RECORD="$ENV_RECORD"
export FAKE_CODEX_STATUS="Logged in using ChatGPT"
saved_chatgpt_auth_status "$FAKE_CODEX" >/dev/null
[[ "$(<"$ENV_RECORD")" == "key-absent" ]]

export FAKE_CODEX_STATUS="Logged in using an API key"
if saved_chatgpt_auth_status "$FAKE_CODEX" >/dev/null 2>&1; then
    printf 'API-key authentication was incorrectly accepted\n' >&2
    exit 1
fi
[[ "$(<"$ENV_RECORD")" == "key-absent" ]]

export FAKE_CODEX_STATUS="Not logged in"
if saved_chatgpt_auth_status "$FAKE_CODEX" >/dev/null 2>&1; then
    printf 'missing subscription authentication was incorrectly accepted\n' >&2
    exit 1
fi
[[ "$(<"$ENV_RECORD")" == "key-absent" ]]

export FAKE_CODEX_STATUS="Logged in using ChatGPT"
export FAKE_CODEX_EXIT="1"
if saved_chatgpt_auth_status "$FAKE_CODEX" >/dev/null 2>&1; then
    printf 'failed status command was incorrectly accepted\n' >&2
    exit 1
fi
[[ "$(<"$ENV_RECORD")" == "key-absent" ]]

printf 'live blind subscription auth policy: PASS\n'
