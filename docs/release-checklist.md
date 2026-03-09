# Release Checklist

## prerequisites

- [x] Apple Silicon target confirmed
- [x] real Codex sidecar available
- [x] real `llama-server` sidecar available
- [x] real model asset available
- [ ] signing credentials available
- [ ] notarization access available
- [ ] second clean environment available

## packaged feasibility

- [x] build packaged `.app`
- [ ] sign packaged `.app`
- [ ] notarize packaged `.app`
- [ ] launch packaged app on a clean environment
- [x] prepare local AI without host-installed tooling
- [x] complete one local assistant turn using bundled assets only
- [ ] verify the reply arrives as preview, not silent document mutation
- [ ] verify logs show bundled runtime/binary paths

Current primary-machine evidence:

- packaged `.app` launches locally
- startup logs prove the app resolves the bundled runtime resource root from inside the `.app`
- packaged `llama-server` now starts from the bundle after copying staged `llama.cpp` dylibs into `Contents/lib`
- packaged `codex` responds to bare `initialize`
- full packaged assistant turn now passes in headless smoke using bundled assets only

## repeatability

- [x] wipe local app data and prepared runtime state
- [x] repeat the same packaged success
- [ ] validate the same flow on a second clean environment

## product-critical UX

- [ ] workflow/module placeholders are not part of the v1 release UX
- [ ] degraded-state send works with warm-up feedback
- [ ] per-document previews survive navigation
- [ ] stale previews are visible but cannot be applied

## reliability

- [x] `cargo check` passes
- [x] `cargo test` passes
- [x] frontend verify is green
- [ ] no orphan helper processes remain after app exit
- [ ] export works for HTML, TXT, and PDF
- [ ] corrupt storage recovery still behaves safely

## security and privacy

- [x] no release reliance on `LOKAL_AI_*` overrides
- [x] no host-installed Codex dependency in release behavior
- [ ] export sanitization still blocks script execution in HTML output
- [ ] logs do not contain raw document contents by default
