# Lokal Skriveflate v1 Execution Brief

## v1 scope

Lokal Skriveflate v1 is an Apple Silicon-only, local-only macOS writing assistant.

It targets:

- one bundled Codex sidecar
- one bundled `llama-server` sidecar
- one blessed bundled model
- preview-before-apply editing
- safe local storage and recovery
- export to HTML, TXT, and PDF

## out of scope for v1

- workflow modules for report, log, or project flows
- browser-preview parity with the desktop app
- a second bundled model
- Intel or universal macOS builds
- remote services, web search, or hosted AI calls

## hard release rules

- packaged feasibility is the first release gate
- the runtime/packaging contract does not freeze after one success
- release behavior cannot depend on host-installed Codex
- release behavior cannot depend on `LOKAL_AI_*` overrides
- assistant edits must always return as preview, never silent mutation

## early owners

- Release Owner: packaging, signing, notarization, artifacts, release checklist assembly
- Runtime Owner: sidecars, model asset handling, prepare/start/repair/shutdown, diagnostics
- Frontend Owner: degraded-state send, preview retention, workflow-surface removal, v1 UX integrity
- QA Owner: clean-environment validation, second-environment validation, evidence capture, go/no-go table

## clean environment definition

A clean environment means:

- no host-installed Codex
- no `LOKAL_AI_*` environment overrides
- fresh app data
- no previously prepared runtime directory
- no cached sidecar/model state from prior runs

Preferred second environment:

- a second physical Apple Silicon Mac

Fallback only if needed:

- a second clean macOS user account with isolated state
