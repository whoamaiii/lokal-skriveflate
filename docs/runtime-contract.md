# Runtime Contract

## release target

- Apple Silicon macOS only
- one blessed local model only
- local-only runtime behavior

## executable strategy

Release behavior is intended to use bundled executables first:

1. packaged Codex sidecar
2. packaged `llama-server` sidecar
3. documented development overrides only for local debugging

For packaged smoke validation in this repo, runtime discovery now prefers:

1. the packaged `Contents/MacOS/llama-server` sidecar derived from the app bundle next to `Contents/Resources/resources`
2. release-only packaged sidecar discovery from the running executable
3. development overrides only in debug/local experimentation

Release builds now stage:

- `Contents/MacOS/codex`
- `Contents/MacOS/llama-server`
- `Contents/Resources/resources/embedded-runtime/manifest.json`
- `Contents/Resources/resources/embedded-runtime/staged/neurologg-q4_k_m.gguf`

Because the Homebrew `llama-server` binary is dynamically linked, release staging also copies the required `llama.cpp` dylibs into:

- `src-tauri/resources/embedded-runtime/staged/lib/`

and `npm run tauri:build` syncs those libraries into the app bundle at:

- `Contents/lib`

so the packaged `llama-server` sidecar can start from inside the `.app`.

The local runtime now starts with:

- `--ctx-size 16384`
- `--no-warmup`

to keep packaged first-turn startup deterministic while still fitting the Codex prompt envelope used for local structured turns.

## model strategy

- one blessed model only: `neurologg-q4_k_m`
- no user-facing model switching in v1
- `activate_model` may remain internally, but any non-default model choice is normalized back to the v1 standard model

## readiness contract

`RuntimeStatus` is the frontend source of truth.

Important fields:

- `runtime_state`
- `can_send`
- `will_start_on_demand`
- `blocking_reason`

Expected UI behavior:

- `can_send = true`: prompt send and quick actions are enabled
- `will_start_on_demand = true`: runtime may start during the first send
- `blocking_reason != null`: the UI should direct the user to setup or repair instead of guessing from message text

## command error contract

Commands should fail with a small structured payload:

- `code`
- `message`
- `retryable`
- optional `action`

Current v1 codes:

- `document_conflict`
- `runtime_not_ready`
- `runtime_start_failed`
- `runtime_repair_required`
- `assistant_timeout`
- `assistant_bridge_error`
- `export_failed`
- `storage_error`
- `unknown`

## assistant edit contract

- assistant output may include one `editor_action`
- document edits are preview-only until explicitly applied in the editor
- previews are stored per document in memory
- previews are not persisted across restart in v1

For the local Responses proxy path:

- Codex talks to a local `/v1/responses` proxy
- the proxy translates that request to `llama.cpp` `/v1/chat/completions`
- system/developer guidance is collapsed into one leading system message
- consecutive same-role messages are merged to satisfy Gemma-style chat templates
- common assistant replies are normalized back into the expected `{ assistant_reply, editor_action }` JSON shape before being streamed back to Codex

## current proof status

- frontend runtime contract: implemented
- local runtime lifecycle cleanup: implemented in app code
- real Apple Silicon sidecars and one real model: staged locally
- packaged `.app` build: passing
- packaged `.app` launch on the primary machine: passing
- packaged resource-root resolution from inside the bundle: proven by startup logs
- packaged headless assistant turn on the primary machine: passing
- same-machine packaged turn repeat after a fresh smoke run: passing
- packaged `llama-server` sidecar now starts from the bundle after dylib sync
- packaged `codex` sidecar now completes local turns against the bundled runtime through the local Responses proxy
- signing/notarization proof: blocked by missing valid codesigning identity on this machine
- second clean-environment proof: still pending
