# Lokal Skriveflate

En offline-first Mac-prototype for dokumentskriving med lokal AI, inspirert av Codex-opplevelsen men bygget som egen Tauri-app.

## Hva som er med

- Tauri 2 desktop-app for macOS
- React + TipTap editor med dokument og chat side om side
- Lokal filbasert lagring av dokumenter, meldinger, settings og snapshots
- Codex `app-server`-bro via `stdio`
- Bundlet lokal runtime-arkitektur for `llama.cpp` med førsteoppstarts-klargjøring
- Modellkatalog med `Qwen3-4B-Instruct Q4_K_M` som standard og `Qwen3-8B-Instruct Q4_K_M` som valgfri kvalitets-pakke
- AI-kontrakt der alle dokumentendringer returneres som preview før innsetting
- Eksport til HTML, TXT og enkel PDF
- Planlagte workflow-plassholdere for rapport, logg og prosjekt

## Kjør lokalt

```bash
npm install
./node_modules/.bin/tauri dev
```

## Bygg app

```bash
./node_modules/.bin/vite build
./node_modules/.bin/tauri build --debug --bundles app
```

Den ferdige `.app`-pakken havner under:

```text
src-tauri/target/debug/bundle/macos/Lokal Skriveflate.app
```

## Lokal AI-stack

Appen forventer:

- `codex` tilgjengelig på maskinen
- en lokal OpenAI-kompatibel `llama.cpp`-serverbinar pakket som appressurs eller pekt inn via miljøvariabel
- en lokal GGUF-modell for standardsporet `qwen3-4b-instruct-q4_k_m`

Denne repoen inneholder:

- et bundlet runtime-manifest i `src-tauri/resources/embedded-runtime/manifest.json`
- placeholder-filer som lar appen bygge og pakke rent
- støtte for å erstatte placeholderne med ekte artefakter uten kodeendringer

For lokal test med ekte filer kan du sette:

```bash
export LOKAL_AI_BINARY_PATH="/full/path/to/llama-server"
export LOKAL_AI_STANDARD_MODEL_PATH="/full/path/to/qwen3-4b-instruct-q4_k_m.gguf"
export LOKAL_AI_QUALITY_MODEL_PATH="/full/path/to/qwen3-8b-instruct-q4_k_m.gguf"
```

Når appen startes og du trykker `Klargjør lokal AI`, kopieres disse filene inn i appens lokale runtime-mappe under `~/Library/Application Support/no.quentin.lokalskriveflate/runtime/<versjon>/`.

## Begrensninger i denne prototypen

- DMG-bundling er ikke ferdigstabilisert; `.app`-bundle er verifisert
- Repoen ships med placeholder-artefakter for runtime/modell fordi ekte Qwen3/llama.cpp-filer er for store til å ligge i kildekoden
- DOCX import/eksport er ikke implementert
- Rapport/logg/prosjekt er planlagte moduler, ikke ferdige workflows
- Typecheck-scriptet er beholdt separat fordi standardbuilden er optimalisert for faktisk app-pakking i dette miljøet
