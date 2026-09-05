# Local session service

This crate contains a replay-backed Rust session library and a single-user
loopback HTTP adapter. The executable embeds the local React browser player
and its exact asset allowlist. This is not a public release,
Internet-facing server, or multi-user authentication service. Saves are exported
to the caller; the server does not automatically persist them to disk.

```bash
cargo run --locked -p forge-server
# Optional port; 0 selects a free loopback port:
cargo run --locked -p forge-server -- --port 0
```

Open the printed `http://127.0.0.1:PORT` address exactly. Host aliases, proxying,
and forwarding headers are unsupported. Ctrl-C stops the process; export saves
before stopping, because all sessions and delivery ledgers are memory-only.

The trusted caller supplies production `CompiledContent` in an `Arc` and starts
a `SessionService` with an authored preset or public custom-character recipe.
Cloning a service shares its session; constructing another service starts an
independent playthrough. Callers must not give a session handle to someone who
should not control that session.

## Rust service contract

`start_options` projects only preset and creation-choice IDs, names, summaries,
and slot order. It never returns a derived character or an authored patch.
`StartRequest::from_json` and `ActionRequest::from_json` reject duplicate and
unknown fields and requests larger than 128 KiB.

`observe` returns a revision, the last kernel observation, and the first action
page. `catalog` takes the expected state ID, offset, and requested page size;
following every `next_offset` covers the entire kernel catalog. Page-size and
response-byte limits do not truncate the legal set. Stale page requests fail.

`act` accepts only an opaque current action ID, expected revision/state ID, and
a command ID. The service resolves that ID from complete kernel enumeration.
It never accepts an effect, outcome, raw state, character sheet, or free-text
interpretation as a command.

Retries with the same command ID and payload return the original acknowledged
view, even after later turns. Reusing that ID with another payload fails.
Callers should not treat a historical retry acknowledgment as the newest view;
use its revision or call `observe`. Concurrent commands from one revision can
commit at most one transition. Observing, paging, and saving consume no turns.

`save` exports only a player-safe replay recipe. `resume` verifies that recipe
against the exact build before creating a new service with a fresh command
ledger. Command acknowledgments are not portable-save data. Use new command IDs
after explicit resume; this does not offer crash-durable exactly-once delivery.

`close` closes the service, not the story. It is idempotent and does not change
world time. Saving remains available afterward; new actions and ordinary views
are closed. An exact retry of a previously committed action still returns its
historical acknowledgment.

## Commit boundary and limits

Each mutation holds the session lock, reconstructs a candidate session from the
stored safe trace, and records only a kernel-enumerated action. It prepares the
next safe save, public view, and idempotency entry before replacing stored data.
A checked preparation failure leaves the old save, revision, view, and command
ledger unchanged. A lost response after commit can be retried without another
turn.

Default limits are 16 MiB each for serialized saves and the acknowledgment
ledger, 128 KiB per response, 32 actions in an initial page, and 128 per requested
page. The ledger never silently evicts old command IDs. Quota exhaustion is an
explicit error; export/resume creates a new service and ledger. These are
serialized-data budgets, not hard RAM, CPU, process, or game-action limits.

The provisional implementation replays history for mutations and catalog
requests. Mutation work is O(history), and a long playthrough can therefore
require O(turns²) total replay work. Cached observation reads avoid replay. This
tradeoff keeps ownership and transactional failure handling simple without
unsafe lifetimes or a second rules implementation; it is not a world-scale
server performance claim.

## HTTP contract

All JSON numeric fields are exact decimal **strings**, including seeds,
revisions, supply quantities, world time, action costs, catalog counts/offsets,
and creation-slot order. Nulls and booleans retain their JSON types. Unsigned
inputs accept only `0` or a nonzero digit followed by digits, within the declared
Rust range; signs, leading zeros, numeric literals, floats, and exponents fail.
This is a transport encoding, not a second rules implementation.

The exception is the downloadable save. Treat it as opaque text: download the
bytes, and import those exact bytes as `save_json`. Parsing and reserializing a
save through JavaScript numbers could corrupt a full-width seed.

| Method and path | Body / result |
| --- | --- |
| `GET /api/bootstrap` | Returns `{token, instance_id}` to a same-origin fetch only |
| `GET /api/current` | Returns the sole active `{session_id, view}` or null; authenticated and read-only |
| `GET /api/options` | Public preset and custom-choice metadata |
| `POST /api/sessions` | `{creation_id, start}` → `{session_id, view}` |
| `POST /api/resume` | `{creation_id, save_json}` → `{session_id, view}` |
| `GET /api/sessions/{id}` | Current public view |
| `POST /api/sessions/{id}/catalog` | `{expected_state_id, offset, page_size}` → complete kernel page |
| `POST /api/sessions/{id}/actions` | `{command_id, expected_revision, expected_state_id, action_id}` → acknowledged view |
| `GET /api/sessions/{id}/save` | Raw player-safe JSON download |
| `POST /api/sessions/{id}/close` | `{}` → `{closed: true}` |

For example, preset `start` is
`{"kind":"preset","character_preset_id":"rook","seed":"71"}`.
Custom `start` uses `kind: "custom"`, a public `selection` with `name` and
`choices: [{slot_id, choice_id}]`, and a string seed. Every request rejects
unknown and duplicate JSON fields. POST bodies require `application/json`
(optionally `; charset=utf-8`). No compressed bodies or query parameters are
accepted. Errors contain only `{error: "stable_code"}`; no stack, host path,
hidden state, or detailed verifier failure is exposed.

Creation IDs follow the command-ID format: 1–128 ASCII letters, digits, `_`, or
`-`. A repeated creation ID and identical typed start, or identical raw imported
save, returns the original handle and initial view—even after progress or close.
Another operation or payload conflicts. Failed creation does not reserve its ID.
Do not install a historical creation/action acknowledgment as the newest view.

The registry retains at most eight total records per process, with one active
at a time. Close before starting another playthrough. Closed records preserve
save access and accepted retries; closing never needs another record slot.
There is no silent eviction. Once eight records are retained, export any saves
and explicitly restart to reclaim capacity. Restart is not crash recovery and
does not preserve idempotency acknowledgments.

## Local access and limits

The executable binds only `127.0.0.1`. It generates a 256-bit process capability
from OS randomness, independently of game entropy. Only `/api/bootstrap` may
return it. Keep the capability in browser memory, never in a URL, HTML, log,
cookie, or localStorage. Every other API request needs `Authorization: Bearer
TOKEN`. The browser must use same-origin fetches: exact Host,
`Sec-Fetch-Site: same-origin`, and `Sec-Fetch-Dest: empty` are required. Mutations
also require the exact Origin. GETs may omit Origin, but any supplied Origin
must match. There is no CORS allowance or cross-site bootstrap. The public root
has strict CSP and no embedded capability. Only the checked browser asset
allowlist is served; there is no runtime filesystem or SPA fallback. The
browser's capability stays in memory and is never included in saved recovery
records. The public process-instance ID distinguishes restart recovery from
same-process request retries.

All responses are `no-store`, `nosniff`, and unframeable. Duplicate security
headers, forwarded headers, host aliases, absolute request URIs, and query-token
alternatives fail closed. This is protection against cross-site browser
requests, not malicious local processes, compromised browsers/extensions, or
same-origin script injection. Do not expose or reverse-proxy this listener.

HTTP defaults allow 256 KiB per safe save, 2 MiB per session command ledger,
128 KiB per pre-encoding service view, 128 KiB ordinary input, and at most
1,576,960 bytes for a JSON-escaped imported save wrapper. Numeric string encoding
adds at most two bytes per integer; responses remain complete. Save size is
checked again after unescaping. These limits do not cap programmed legal actions.

At most two game requests are admitted to buffer or run, and one blocking
worker executes at a time. Body reading has a five-second deadline before
mutation. Once dispatched, work retains both permits until it actually finishes,
even if the client disconnects. There is no misleading action-cancellation
timeout: a lost response must be retried with its original ID and payload.
`busy` is an admission rejection, not a committed action. Serialized quotas and
worker admission are not hard RAM, connection-count, CPU-time, or Internet-DoS
isolation. Full-history replay costs still require measurement before scaling.

## Verification

`cargo test --locked -p forge-server` checks the public contract against direct
canonical sessions. `./tools/verify-session-service.sh`, also included in
`./verify`, runs the trusted `replay_smoke` example, retries each command,
resumes after its paid-route checkpoint, and passes the exported ten-action
player save to the independent `forge-verify check-player` process. Each run
keeps its safe trace and trusted result under `artifacts/local/session-service/`.
This is scripted adapter/replay evidence, not a blind or browser playtest.

`./tools/verify-http-service.sh`, also in `./verify`, starts the real executable
on a free loopback port and drives ten canonical actions over HTTP sockets. It
discards the payment response body, retries without another turn, closes and
resumes the checkpoint, finishes the relief route, and sends the exact exported
save to the separate trusted verifier. Safe artifacts are retained under
`artifacts/local/http-service/`. Unit/integration checks additionally cover
cross-site and malformed requests, exact integer encoding, lifecycle limits,
concurrency, and a dropped response future while its committed worker still
owns both permits. Real-browser bootstrap/header checks supplement these tests;
they are not a completed browser game or blind-player evidence.
