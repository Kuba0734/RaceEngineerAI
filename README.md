# RaceEngineerAI

Windows-App (Tauri 2 + Rust + React), die beim Sim-Racing wie eine echte Pit Crew über Funk spricht: Spotter, Renningenieur mit Strategie, freies Gespräch über Groq (Whisper + LLM mit Tool Calling), lokale Piper-Stimmen, Overlay.

Regeln, Architektur und Design: [`CLAUDE.md`](CLAUDE.md) · Alle Meldungen, Fragen, Befehle: [`CALLS.md`](CALLS.md) · Spiel-Schnittstellen: [`docs/`](docs)

## Entwicklung

```bash
npm install
npm run tauri dev
npm run lint && npm run typecheck
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
```

Ohne Spiel testen: im Dashboard „Demo starten“ (Mock-Rennen) oder aufgenommene Replays abspielen (Settings → Allgemein).

Manuelle Live-Tests:

```bash
cargo test --test acc_live -- --ignored --nocapture        # ACC muss laufen
cargo test --test questions -- --ignored --nocapture       # GROQ_API_KEY setzen
```

## Release

Tag `vX.Y.Z` pushen → GitHub Actions baut das signierte NSIS-Setup und einen Draft-Release mit `latest.json` für den Auto-Updater. Secrets im Repo: `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Der private Schlüssel liegt **nur** unter `%USERPROFILE%\.tauri\raceengineerai.key` – sichern, sonst können keine Updates mehr ausgeliefert werden.
