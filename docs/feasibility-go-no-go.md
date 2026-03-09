# Feasibility Go / No-Go

## decision

`NO-GO`

## reason

The repository now contains the v1 runtime contract, explicit readiness fields, shutdown cleanup, one-model packaging, real staged Apple Silicon assets, a passing packaged `.app` build, and a repeated primary-machine packaged-turn smoke proof using bundled assets only.

This is still not a truthful release `GO` because the remaining release-critical evidence is external and incomplete:

- signed packaged app proof
- notarized packaged app proof
- second-environment success

Current blockers:

- no valid local codesigning identity is available on this machine
- no second clean Apple Silicon environment has been used yet
- the repeated packaged-turn proof is currently headless smoke evidence, not a full GUI walkthrough on a second environment

## reviewers

- Release Owner: packaged `.app` build works, but signing/notarization evidence is still pending
- Runtime Owner: real asset staging, dylib sync, local Responses proxy translation, and repeated packaged-turn smoke are implemented
- QA Owner: blocked from final proof until a second clean environment and signed/notarized validation are executed
- Frontend Owner: verification is green

## investigated failure classes

- sidecar discovery failure: release bundle now contains packaged `codex` and `llama-server` sidecars
- sidecar execution/permission failure: packaged `llama-server` initially failed because required dylibs were missing; release build now syncs those dylibs into `Contents/lib`
- model path/resource failure: real staged `neurologg-q4_k_m.gguf` is bundled into the app and visible in the packaged manifest
- runtime health/start failure: packaged `llama-server` now starts successfully from the bundle after dylib sync and with the larger local context window
- Codex bridge launch/protocol failure: packaged `codex app-server` now completes turns against the local provider after the Responses-proxy translation and timeout hardening
- signing/notarization behavior difference: still blocked by lack of valid signing identity on this machine

## packaged success status

- packaged app build: `PASS`
- packaged app primary-machine launch: `PASS`
- packaged local runtime sidecar launch from bundle: `PASS`
- packaged Codex initialize from bundle: `PASS`
- packaged local assistant turn: `PASS`
- repeat success after clean reset: `PASS`
- second clean environment: `FAIL`

## frontend/runtime hardening completed in repo

- hidden workflow/module release surface
- explicit runtime readiness fields
- structured command errors
- degraded-state send support
- per-document preview retention
- minimal local tracing
- explicit runtime shutdown cleanup path
- one-model staged manifest default
- deterministic frontend verification
- packaged dylib sync for `llama.cpp`

## chosen next action

Finish the remaining proof sequence in this order:

1. sign/notarize with a real identity
2. rerun the packaged smoke against the signed/notarized bundle
3. rerun on a second clean Apple Silicon environment
