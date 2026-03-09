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

## Verifiser lokalt

```bash
npm run typecheck
npm test
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
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

## Gjenoppretting fra korrupt lokal lagring

Hvis appen finner korrupt JSON i dokumentmappen eller i `app-state.json`, flyttes fila automatisk til:

```text
~/Library/Application Support/no.quentin.lokalskriveflate/corrupt/
```

Appen fortsetter oppstarten med de friske dokumentene som finnes og viser en gjenopprettingsmelding i grensesnittet etter oppstart.

## Manuell validering av `.app`-pakken

Bruk den faktiske `.app`-pakken under `src-tauri/target/debug/bundle/macos/Lokal Skriveflate.app`, ikke bare `tauri dev`, når du går gjennom denne sjekklista.

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
7. Kjør lokal AI-oppsett med bare standardmodellen tilgjengelig. Oppsettet skal lykkes uten at kvalitetsmodellen finnes.
8. Tving gjentatte start- eller reparasjonsfeil i lokal runtime. Appen skal kunne prøve igjen uten hengende hjelpeprosesser.
9. Eksporter HTML fra et dokument som inneholder `<script>` i tittel eller innhold. Den eksporterte fila skal ikke kjøre skript.
10. Eksporter et tomt dokument til PDF. Resultatet skal være en gyldig PDF med én tom side.

## Begrensninger i denne prototypen

- DMG-bundling er ikke ferdigstabilisert; `.app`-bundle bygger rent, men skal fortsatt kjøres gjennom sjekklista over før release
- Repoen ships med placeholder-artefakter for runtime/modell fordi ekte Qwen3/llama.cpp-filer er for store til å ligge i kildekoden
- DOCX import/eksport er ikke implementert
- Rapport/logg/prosjekt er planlagte moduler, ikke ferdige workflows
- macOS er det validerte målmiljøet i denne repoen; andre plattformer er ikke en del av den nåværende release-støtten
