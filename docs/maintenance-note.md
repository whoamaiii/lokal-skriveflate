# Maintenance Note

## update path for release assets

When the real release assets are available:

1. set:
   - `LOKAL_RELEASE_CODEX_SOURCE`
   - `LOKAL_RELEASE_LLAMA_SOURCE`
   - `LOKAL_RELEASE_MODEL_SOURCE`
2. run `npm run prepare:release-assets`
3. confirm the staged outputs exist under:
   - `src-tauri/binaries/`
   - `src-tauri/resources/embedded-runtime/staged/`
   - `src-tauri/resources/embedded-runtime/staged/lib/`
4. run `npm run tauri:build -- --bundles app`
5. confirm the packaged app contains:
   - `Contents/MacOS/codex`
   - `Contents/MacOS/llama-server`
   - `Contents/lib/*.dylib`
   - `Contents/Resources/resources/embedded-runtime/manifest.json`
   - `Contents/Resources/resources/embedded-runtime/staged/neurologg-q4_k_m.gguf`
6. rerun packaged feasibility
7. rerun repeat success after clean reset
8. rerun second-environment validation

## llama.cpp dylibs

The Apple Silicon `llama-server` binary used here is dynamically linked.

Release pipeline expectation:

1. `npm run prepare:release-assets` stages the required `llama.cpp` dylibs under `src-tauri/resources/embedded-runtime/staged/lib/`
2. `npm run tauri:build` copies those dylibs into the app bundle under `Contents/lib`

If the packaged `llama-server` sidecar fails with `Library not loaded: @rpath/...`, first verify the expected files exist under `Contents/lib`.

## runtime contract changes

If you change runtime readiness or command error behavior:

1. update `docs/runtime-contract.md`
2. update the frontend types in `src/types.ts`
3. update the Rust serialized types in `src-tauri/src/models.rs`
4. rerun frontend and backend verification

## adding a new command error code

Only add a new code if the UI needs distinct behavior.

Required updates:

1. Rust `CommandErrorCode`
2. TypeScript `CommandErrorCode`
3. UI branching/tests
4. `docs/runtime-contract.md`

## post-v1 deferred items

- second bundled model
- universal or Intel build support
- real workflow modules
- broader browser-preview parity
- richer preview diffing
