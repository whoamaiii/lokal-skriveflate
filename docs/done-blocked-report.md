# Lokal Skriveflate v1 Done / Blocked Report

## recommendation

- Status: `NO-GO`
- Confidence: `high`

The repository now reflects the approved v1 direction much more closely, and the release evidence is materially stronger than before: real Apple Silicon assets are staged, the packaged `.app` builds, the packaged `.app` launches on the primary machine, frontend verification is deterministic, and the bundled runtime now completes a headless packaged assistant turn on the primary machine. This is still not a truthful `GO` because signing/notarization is unavailable on this machine and a second clean environment has not been validated yet.

## shipped / implemented in repo

- fake workflow/module surface removed from the primary frontend UX
- runtime readiness fields added to the shared contract
- structured command errors added to the Tauri command layer
- degraded-state send path enabled in the chat UI
- per-document preview retention implemented in the app shell
- single-model release default applied in browser-preview and staged manifest
- minimal Rust tracing added for startup/runtime/bridge/shutdown paths
- explicit runtime shutdown cleanup wired into app exit
- packaged-first Codex path resolution implemented
- CI Node version pinned to `20.19.0`
- deterministic frontend verification restored
- real Apple Silicon Codex sidecar staged locally
- real Apple Silicon `llama-server` sidecar staged locally
- real `neurologg-q4_k_m` model staged locally
- release asset prep now stages required `llama.cpp` dylibs
- release build now syncs staged dylibs into `.app/Contents/lib`
- packaged `.app` build succeeds
- packaged `.app` startup on the primary machine succeeds and logs the bundled runtime resource root
- lean release docs updated to reflect the current evidence

## status items

### packaged feasibility proof

- Owner: `Runtime Owner`
- Failure class: `resolved`
- Impact: `cleared on primary-machine smoke`
- Next action: keep the packaged smoke path repeatable and use it as the primary bundled-runtime regression check

### signing and notarization validation

- Owner: `Release Owner`
- Failure class: `signing_notarization_unavailable`
- Impact: `release blocker`
- Next action: supply valid codesigning identity and notarization access, then rerun the packaged feasibility sequence

### second clean environment validation

- Owner: `QA Owner`
- Failure class: `missing_second_environment_evidence`
- Impact: `release blocker`
- Next action: run the packaged smoke on a second physical Apple Silicon Mac, or use a second clean macOS user account as fallback evidence

### frontend verification

- Owner: `Frontend Owner`
- Failure class: `resolved`
- Impact: `cleared`
- Next action: keep Vitest scoped to `src/**/*.{test,spec}.{ts,tsx}` and continue using exact Node `20.19.0`

## evidence

- Real Apple Silicon sidecars staged: `PASS`
- Real blessed model staged: `PASS`
- Packaged `.app` build: `PASS`
- Packaged `.app` primary-machine launch: `PASS`
- Bundled runtime resource root detected from inside the app: `PASS`
- Packaged `llama-server` sidecar launch from bundle: `PASS`
- Packaged bare Codex initialize from bundle: `PASS`
- Packaged local turn: `PASS`
- Repeat clean reset run: `PASS`
- Second clean environment: `FAIL`
- No host-installed Codex used in packaged build/startup proof: `PASS`
- No env override reliance in packaged build/startup proof: `PASS`
- Frontend verify: `PASS`
- Degraded-state send: `IMPLEMENTED`
- Per-document preview retention: `IMPLEMENTED`
- No orphan helper processes after exit: `IMPLEMENTED, NOT FULLY PACKAGED-VERIFIED`
- `cargo check`: `PASS`
- `cargo test`: `PASS`

## key artifacts

- v1 execution brief: `/Users/quentinthiessen/Documents/localdocumentasjon/docs/v1-execution-brief.md`
- runtime contract: `/Users/quentinthiessen/Documents/localdocumentasjon/docs/runtime-contract.md`
- feasibility go/no-go: `/Users/quentinthiessen/Documents/localdocumentasjon/docs/feasibility-go-no-go.md`
- release checklist: `/Users/quentinthiessen/Documents/localdocumentasjon/docs/release-checklist.md`
- troubleshooting/setup note: `/Users/quentinthiessen/Documents/localdocumentasjon/docs/troubleshooting-setup.md`
- maintenance note: `/Users/quentinthiessen/Documents/localdocumentasjon/docs/maintenance-note.md`

## immediate next action

Finish the last release blockers in this order:

1. sign/notarize the packaged build with a real identity
2. rerun the packaged smoke against the signed/notarized bundle
3. rerun second-environment validation, including the GUI walkthrough and preview-before-apply checks
