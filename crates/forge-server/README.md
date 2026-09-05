# Session service foundation

This crate is an in-process Rust library for the planned browser adapter. It is
not yet an HTTP server, browser client, authentication service, or public release.
No port is opened and no save file is written by this crate.

The trusted caller supplies production `CompiledContent` in an `Arc` and starts
a `SessionService` with an authored preset or public custom-character recipe.
Cloning a service shares its session; constructing another service starts an
independent playthrough. Callers must not give a session handle to someone who
should not control that session.

## Public contract

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

## Before HTTP or browser access

A transport still needs a session registry and idempotent creation, loopback-only
binding, strict host/origin checks, authorization, streaming request limits,
bounded work/lifecycle handling, coherent save downloads, and sanitized HTTP
errors. A browser must render public views and submit action identities without
duplicating game rules. Neither surface nor remote multi-user security is
claimed by this library.

## Verification

`cargo test --locked -p forge-server` checks the public contract against direct
canonical sessions. `./tools/verify-session-service.sh`, also included in
`./verify`, runs the trusted `replay_smoke` example, retries each command,
resumes after its paid-route checkpoint, and passes the exported ten-action
player save to the independent `forge-verify check-player` process. Each run
keeps its safe trace and trusted result under `artifacts/local/session-service/`.
This is scripted adapter/replay evidence, not a blind or browser playtest.
