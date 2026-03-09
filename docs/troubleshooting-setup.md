# Setup and Troubleshooting

## local setup

Recommended baseline:

- Node `20.19.0`
- npm from the same Node install
- Rust stable
- Apple Silicon macOS

Install and verify:

```bash
npm ci
npm run verify:backend
npm run verify:frontend
```

To stage real Apple Silicon release assets locally:

```bash
export LOKAL_RELEASE_CODEX_SOURCE="/full/path/to/codex-aarch64-apple-darwin"
export LOKAL_RELEASE_LLAMA_SOURCE="/full/path/to/llama-server"
export LOKAL_RELEASE_MODEL_SOURCE="/full/path/to/neurologg-q4_k_m.gguf"
npm run prepare:release-assets
```

Then build the packaged app:

```bash
npm run tauri:build -- --bundles app
```

## current status and blockers

### packaged assistant turn now passes in headless smoke

Current state:

- real Apple Silicon assets can be staged locally
- the packaged `.app` builds successfully
- the packaged `.app` launches on the primary machine
- the packaged `llama-server` sidecar needs `llama.cpp` dylibs under `Contents/lib`
- `npm run tauri:build` now syncs the staged dylibs into the app bundle automatically
- the packaged `codex` sidecar responds to bare `initialize`
- `codex app-server` now completes turns against the local provider configuration used for the bridge
- the packaged smoke remains headless evidence; signed/notarized proof and second-environment GUI validation are still pending

If you re-test this path:

1. restage the real assets with `npm run prepare:release-assets`
2. rebuild the `.app` with `npm run tauri:build -- --bundles app`
3. verify `Contents/MacOS/codex`, `Contents/MacOS/llama-server`, and `Contents/lib/*.dylib` exist
4. run `cargo test --manifest-path src-tauri/Cargo.toml packaged_primary_machine_turn_smoke -- --ignored --nocapture`
5. inspect `~/Library/Application Support/no.quentin.lokalskriveflate/logs/lokal-skriveflate.log`

### signing and notarization are still unavailable on this machine

Current state:

- `security find-identity -v -p codesigning` returns `0 valid identities found`
- the current `.app` is ad-hoc signed only

### frontend verification

Current state:

- `npm run verify:frontend` now passes deterministically
- the earlier stall was caused by Vitest discovering tests under a `node_modules.bak*` backup directory

If frontend verification stalls again:

1. check for backup directories like `node_modules.bak*`
2. keep Vitest scoped to `src/**/*.{test,spec}.{ts,tsx}`
3. rerun `npm ci`
4. keep CI pinned to Node `20.19.0`

## clean environment checklist

Before feasibility validation:

1. remove any prepared app data
2. ensure no host-installed Codex is available if you are claiming clean-machine proof
3. ensure no `LOKAL_AI_*` environment variables are set
4. verify there is no leftover runtime state from earlier runs

## logs

Local runtime and bridge tracing is written under the app data directory:

- `.../logs/lokal-skriveflate.log`
