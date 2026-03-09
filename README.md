# Lokal Skriveflate

En Apple Silicon-only, offline-first macOS skriveflate for lokal AI-assistanse med preview-before-apply som hovedregel.

## Hva som er med

- Tauri 2 desktop-app for Apple Silicon macOS
- React + TipTap editor med dokument og chat side om side
- Lokal filbasert lagring av dokumenter, meldinger, settings og snapshots
- Codex `app-server`-bro via `stdio`
- Bundlet lokal runtime-arkitektur for `llama.cpp` med førsteoppstarts-klargjøring
- Én velsignet modellbane for `Neurologg Q4_K_M`
- AI-kontrakt der alle dokumentendringer returneres som preview før innsetting
- Eksport til HTML, TXT og enkel PDF

## Kjør lokalt

```bash
npm install
./node_modules/.bin/tauri dev
```

## Verifiser lokalt

```bash
npm run verify:frontend
npm run verify:backend
```

## Bygg app

```bash
LOKAL_RELEASE_CODEX_SOURCE="/full/path/to/codex-aarch64-apple-darwin" \
LOKAL_RELEASE_LLAMA_SOURCE="/full/path/to/llama-server" \
LOKAL_RELEASE_MODEL_SOURCE="/full/path/to/neurologg-q4_k_m.gguf" \
npm run prepare:release-assets

npm run tauri:build -- --bundles app
```

Den ferdige `.app`-pakken havner under:

```text
src-tauri/target/release/bundle/macos/Lokal Skriveflate.app
```

## Lokal AI-stack

- bundlet Codex-sidecar
- bundlet `llama-server`-sidecar
- én bundlet modell
- ingen host-installert Codex i release-behavior
- `llama.cpp`-dylibs blir staged under `src-tauri/resources/embedded-runtime/staged/lib/`
- `npm run tauri:build` synkroniserer disse bibliotekene inn i `.app`-pakken under `Contents/lib`

Se også:

- [v1 execution brief](./docs/v1-execution-brief.md)
- [runtime contract](./docs/runtime-contract.md)
- [feasibility go/no-go](./docs/feasibility-go-no-go.md)
- [release checklist](./docs/release-checklist.md)

For lokal staging med ekte filer:

```bash
export LOKAL_RELEASE_CODEX_SOURCE="/full/path/to/codex-aarch64-apple-darwin"
export LOKAL_RELEASE_LLAMA_SOURCE="/full/path/to/llama-server"
export LOKAL_RELEASE_MODEL_SOURCE="/full/path/to/neurologg-q4_k_m.gguf"
npm run prepare:release-assets
```

I debug-bygg kan du fortsatt bruke utvikler-overstyringer:

```bash
export LOKAL_AI_CODEX_PATH="/full/path/to/codex"
export LOKAL_AI_BINARY_PATH="/full/path/to/llama-server"
export LOKAL_AI_MODEL_PATH="/full/path/to/neurologg-q4_k_m.gguf"
```

I release-behavior skal appen bruke de bundlete sidecarene og den bundlete modellen.

## Gjenoppretting fra korrupt lokal lagring

Hvis appen finner korrupt JSON i dokumentmappen eller i `app-state.json`, flyttes fila automatisk til:

```text
~/Library/Application Support/no.quentin.lokalskriveflate/corrupt/
```

Appen fortsetter oppstarten med de friske dokumentene som finnes og viser en gjenopprettingsmelding i grensesnittet etter oppstart.

## Manuell validering av `.app`-pakken

Bruk den faktiske `.app`-pakken under `src-tauri/target/release/bundle/macos/Lokal Skriveflate.app`, ikke bare `tauri dev`, når du går gjennom denne sjekklista.

For scenarier som avhenger av lagringsstatus, start med en ren app-data-mappe:

```bash
rm -rf ~/Library/Application\ Support/no.quentin.lokalskriveflate
```

Sjekkliste for close-out:

1. Skriv tekst og trykk send umiddelbart. AI-svaret skal bruke siste revisjon uten å miste tekst.
2. Skriv tekst og bytt dokument umiddelbart. Endringen skal være lagret når det andre dokumentet åpnes.
3. Skriv tekst og eksporter umiddelbart til HTML, PDF og TXT. Eksporten skal inneholde siste tekst.
4. Lag et AI-preview, rediger dokumentet videre og prøv å bruke previewet. Previewet skal markeres som utdatert og ikke kunne brukes.
5. Ødelegg én dokument-JSON manuelt og start appen på nytt. Appen skal åpne, flytte fila til `corrupt/` og vise gjenopprettingsmelding.
6. Ødelegg `app-state.json` manuelt og start appen på nytt. Appen skal gjenopprette standardtilstand, velge et friskt dokument og vise gjenopprettingsmelding.
7. Kjør lokal AI-oppsett med bare standardmodellen tilgjengelig. Oppsettet skal lykkes uten å forvente ekstra modellpakker.
8. Tving gjentatte start- eller reparasjonsfeil i lokal runtime. Appen skal kunne prøve igjen uten hengende hjelpeprosesser.
9. Eksporter HTML fra et dokument som inneholder `<script>` i tittel eller innhold. Den eksporterte fila skal ikke kjøre skript.
10. Eksporter et tomt dokument til PDF. Resultatet skal være en gyldig PDF med én tom side.

## Begrensninger akkurat nå

- Packaged `.app` build, primær oppstart og headless packaged assistant-turn smoke er verifisert på primærmaskinen
- Gjentatt packaged smoke etter clean reset er dokumentert, men full GUI-walkthrough på en andre clean environment er fortsatt ikke kjørt
- Denne maskinen mangler gyldig codesigning-identitet og notariseringssteg
- Andre clean-environment / second-machine-bevis er fortsatt ikke kjørt
- Ekte release-artefakter er staged lokalt, men er ikke ment å ligge versjonert i repoen
- DOCX import/eksport er ikke implementert
- Rapport/logg/prosjekt er ikke del av funksjonell v1-scope
- Intel/universal builds er ikke del av v1
