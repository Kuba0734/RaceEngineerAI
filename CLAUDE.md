# CLAUDE.md – RaceEngineerAI

RaceEngineerAI ist eine Windows-Desktop-App, die wie eine echte Pit Crew über Funk mit dem Fahrer spricht. Sie liest live die Telemetrie von Sim-Racing-Spielen, gibt Strategie-, Spotter- und Fahrhinweise per Stimme aus, beantwortet gesprochene Fragen über eine KI (Groq) und zeigt ein Overlay über dem Spiel.

**Zielplattform: nur Windows 10/11 x64.** Ausgeliefert wird ein Setup (NSIS) mit automatischen Updates.
**Oberste Prinzipien: Performance zuerst, Spotter-Latenz vor allem anderen, keine FPS-Einbußen im Spiel.**

---

## Tech-Stack (festgelegt, nicht ohne Rückfrage ändern)

| Bereich | Technologie |
|---|---|
| App-Shell | Tauri 2 (Rust-Backend, WebView2-Frontend) |
| Frontend | React 19 + TypeScript (strict) + Vite |
| Styling | Tailwind CSS v4 + CSS-Variablen (Design-Tokens) |
| Animationen | `motion` (Framer Motion), nur `transform`/`opacity` animieren |
| State (Frontend) | Zustand mit Selektoren; Hochfrequenz-Daten über Refs, nicht über React-State |
| Live-Karte | `<canvas>` 2D mit `requestAnimationFrame`, kein SVG/DOM pro Auto |
| Audio I/O | `cpal` (Mikrofon + Ausgabe), eigener Mixer mit Prioritäts-Queue |
| VAD, Codewort, TTS | `sherpa-onnx` (Rust-Bindings): Silero-VAD, Keyword-Spotting, Piper-VITS-Stimmen |
| Spracherkennung | Groq Whisper API (`whisper-large-v3-turbo`, Modell in Settings wählbar) |
| KI-Antworten | Groq Chat API (OpenAI-kompatibel), Modellliste live von `/openai/v1/models` laden |
| HTTP | `reqwest` (rustls), `tokio` |
| Speicher | SQLite über `rusqlite` (Sessions, Runden, Referenzrunden) |
| Settings | JSON in `%APPDATA%\RaceEngineerAI\settings.json`, versioniert + Migrationen |
| API-Key | Windows Credential Manager über `keyring`-Crate – **niemals** im Klartext speichern/loggen |
| Lenkrad-Tasten | `Windows.Gaming.Input` RawGameController (`windows`-Crate), `gilrs` als Fallback |
| Prozess-Erkennung | `sysinfo`, Polling alle 2 s (nur Prozessnamen, kein Full-Refresh) |
| Installer/Updates | Tauri-Bundler NSIS + `tauri-plugin-updater` über GitHub Releases (signiert) |
| Logging | `tracing` + rotierende Logdatei in `%LOCALAPPDATA%\RaceEngineerAI\logs` |

---

## Architektur

```
┌──────────────── Rust-Backend (src-tauri) ─────────────────┐
│ GameDetector ──► TelemetryProvider (Trait, pro Spiel)     │
│                     │  normalisiert zu TelemetryFrame     │
│                     ▼                                     │
│                 RaceState (Single Source of Truth)        │
│      ┌──────────┬───────┴─────┬────────────┬────────────┐ │
│   Spotter   Strategy     Gaps/Rivals   LapCompare  Weather│
│   (60 Hz)   (Fuel/Pit)   (Undercut)   (Kurven)    (Regen) │
│      └──────────┴──────┬──────┴────────────┴────────────┘ │
│                   CallEngine (Regeln, Prioritäten, Cooldown)
│                        ▼                                  │
│   VoiceIn (VAD/PTT/Codewort) ─► Groq STT ─► Router ─► LLM+Tools │
│                        ▼                                  │
│                 AudioOut (TTS, Funk-Effekt, Mixer)        │
└─────────────── Tauri Channels (gedrosselt) ───────────────┘
        ▼                                ▼
  Hauptfenster (React)        Overlay-Fenster (React, transparent)
```

### Kernregeln der Architektur
1. **Spiel-Abstraktion:** Jedes Spiel implementiert `trait TelemetryProvider` und liefert ein spielunabhängiges `TelemetryFrame` + `SessionInfo` + `Vec<CarState>`. Die gesamte Logik darüber kennt kein einziges spielspezifisches Feld. Neue Spiele = neuer Provider, sonst nichts.
2. **Spiele sind datengetrieben:** `src-tauri/resources/games.json` enthält pro Spiel: id, Name, Prozessnamen, Provider, Fähigkeiten (z. B. `hasNativeSpotter`, `hasWeatherForecast`), Setup-Hinweise. Die Spieleübersicht im UI wird daraus generiert.
3. **Deterministisch rechnen, KI versteht:** Spotter, Box-Calls, Sprit, Abstände, Undercut, Regen und Kurvenvergleich werden **regelbasiert in Rust** berechnet. Die KI (Groq) versteht die Fragen und Befehle des Fahrers in jeder Formulierung und holt sich die Antworten über Werkzeuge aus dieser Logik; außerdem optional für natürlichere Formulierung nicht-zeitkritischer Meldungen und Zusammenfassungen. Fällt Groq aus, funktioniert alles außer dem freien Gespräch weiter.
4. **Spotter ist heilig:** Spotter-Meldungen („Auto links“, „Auto rechts“, „Drei breit“, „Frei“) laufen nie über Netzwerk oder KI. Ihre Audio-Clips werden beim Start pro Stimme/Sprache vorgerendert und im RAM gecacht. Ziel-Latenz von Telemetrie-Ereignis bis Ton: **< 100 ms**.
5. **Audio-Prioritäten:** `Spotter > Kritisch (Sprit/Schaden/Box jetzt) > Antwort auf Frage > Strategie > Info`. Höhere Priorität unterbricht oder senkt niedrigere ab (Ducking). Jede Meldungsart hat Cooldown und Ablaufzeit (veraltete Meldungen werden verworfen, nicht verspätet gesagt).
6. **Echo-Schutz:** Während die Crew spricht, wird das Mikrofon für VAD/Codewort stummgeschaltet (Halbduplex), außer „Unterbrechen erlauben“ ist in den Settings an.
7. **Ein Overlay-Fenster pro Monitor:** Ein einziges transparentes, rahmenloses, `always_on_top`, `skip_taskbar`, durchklickbares (`set_ignore_cursor_events(true)`) Fenster. Alle Widgets (Karte, Rangliste, Spotter-Radar, Sprit/Strategie, Delta, Funk-Anzeige, Dash, Pedale/Lenkung, letzte Runden, Reifen, Flaggen-Banner) liegen darin und sind im **Bearbeitungsmodus** (globaler Hotkey) frei verschieb- und skalierbar. Layout und Overlay-Einstellungen werden pro Spiel gespeichert. Keine separate WebView pro Widget (zu viel RAM).
8. **Telemetrie-Raten:** Intern lesen so schnell wie das Spiel liefert (Spotter mind. 60 Hz). Ans Frontend gedrosselt: Overlay 20 Hz (in Settings 10–30 Hz), Hauptfenster 5 Hz. Nur geänderte Felder senden, kompakte Structs, keine großen JSON-Blobs pro Frame.
9. **Kein Blockieren:** Keine blockierenden Aufrufe im Telemetrie- oder Audio-Thread. Netzwerk, SQLite und TTS-Synthese laufen in eigenen Tasks/Threads.

---

## Spiele

| Spiel | Status | Prozess (prüfen!) | Datenquelle |
|---|---|---|---|
| Assetto Corsa Competizione | **zuerst, jetzt** | `AC2-Win64-Shipping.exe` | Shared Memory + Broadcasting-UDP-API |
| Le Mans Ultimate | später | `Le Mans Ultimate.exe` | eingebaute Shared Memory (recherchieren) |
| iRacing | umgesetzt, live teilweise geprüft | `iRacingSim64DX11.exe` | iRacing SDK Memory-Mapped File, Details `docs/iracing.md` |
| Assetto Corsa EVO | später | recherchieren | recherchieren |

**Prozessnamen und Schnittstellen der späteren Spiele vor der Umsetzung recherchieren und verifizieren, nicht raten.**

### ACC-Spezifika
- Shared Memory: `Local\acpmf_physics`, `Local\acpmf_graphics`, `Local\acpmf_static`. Structs exakt nach offizieller ACC-Shared-Memory-Doku nachbauen, `#[repr(C, packed(4))]`, Strings sind `[u16; N]` (wchar). Lesen per `OpenFileMappingW` + `MapViewOfFile`, `packetId` prüfen, um ungeänderte Frames zu überspringen.
- Gegner-Positionen für Spotter/Karte: `graphics.carCoordinates` + `carID`-Array + `playerCarID`.
- Rangliste, Sektorzeiten, Box-Status anderer Autos, Rundenzeiten: **Broadcasting-UDP-API**. RaceEngineerAI kann `Dokumente\Assetto Corsa Competizione\Config\broadcasting.json` automatisch einrichten (Port, Passwort) – Datei ist **UTF-16 LE** kodiert, vorher Backup anlegen, Nutzer bestätigen lassen, Hinweis: ACC neu starten.
- Wetter: `rainIntensity`, `rainIntensityIn10min`, `rainIntensityIn30min`, `trackGripStatus`.
- Sprit: `fuel`, `fuelXLap`, `usedFuel`, `fuelEstimatedLaps`; Boxenfenster: `pitWindowStart`/`pitWindowEnd` aus `static`.
- ACC liefert keinen fertigen „Auto links/rechts“-Wert → Spotter selbst berechnen (siehe unten).

---

## Fachlogik (Kurzfassung)

- **Spotter:** Gegnerposition ins lokale Koordinatensystem des Spielers drehen (Heading). Überlappung, wenn längs innerhalb ±(Autolänge, ca. 4,8 m) und quer 1,0–6 m. Zustände: frei / links / rechts / drei breit. Hysterese (ca. 300 ms) gegen Flackern, „Frei“ nur nach vorheriger Meldung. In der Box und im Stand aus.
- **Abstände:** Positionsbasiertes Timing: Strecke in ~100 Mini-Sektoren teilen, Durchfahrtszeit jedes Autos pro Mini-Sektor speichern, Abstand = Zeitdifferenz am selben Punkt. Trends über die letzten 3–5 Runden → „holt auf“ / „fällt zurück“ (Zehntel pro Runde).
- **Sprit:** Rollierender Durchschnitt über saubere Runden (Out-Lap, In-Lap, Gelb und Safety Car verwerfen). Rest-Runden, Nachtank-Liter = (Restrunden × Verbrauch) + Sicherheitsreserve (Settings, Standard 1 Runde) − aktueller Sprit. Startsprit fürs nächste Rennen = Renndistanz × Verbrauch + Formationsrunde + Reserve.
- **Boxenstopp:** Box-Verlustzeit pro Strecke messen (sonst Standardwert aus `tracks.json`), Rauskomm-Position = Vergleich mit Abständen der Autos dahinter. Undercut-Warnung: Auto in Reichweite (Settings, Standard < 3 s) fährt an die Box.
- **Kurvenvergleich:** Kurven automatisch aus Geschwindigkeitsminima der Referenzrunde erkennen (oder aus `tracks.json`). Pro Kurve: Bremspunkt, Scheitelgeschwindigkeit, Gasannahme vergleichen mit bester Runde. Nach der Runde höchstens die **1–2 größten** Zeitverluste nennen („Kurve 3: zu früh gebremst, etwa 8 Meter“).
- **Regen & Reifen:** Regenvorhersage ansagen, Wechsel auf Regenreifen empfehlen, wenn Prognose/Gripstatus es nahelegen. Reifendruck-Empfehlung für den nächsten Stint aus gemessenen Heißdrücken vs. Zielfenster (Zielfenster pro Fahrzeugklasse in `cars.json`, editierbar).
- **Flaggen & Rennleitung:** Überrundung (blaue Flagge), Gelb voraus inkl. stehender Autos, FCY mit Box-Empfehlung, Track Limits zählen und vor Strafe warnen, Strafen + Erinnerung, Restzeit/Restrunden, letzte Runde, Zielflagge.
- **Auto & Reifen:** kalte/überhitzte Reifen, Live-Drücke, Bremsverschleiß und -temperatur, Schaden mit Abwägung Reparatur vs. Zeitverlust, TC-/ABS-Eingriffe und Blockieren mit Setup-Tipp, Licht, Wischer, Stint-Fahrzeit.
- **Pace & Taktik:** Ziel-Pace zum Einholen, Spritspar-Ziel mit Lift-and-Coast-Punkten, Gegner-Stärken pro Sektor (verteidigen/angreifen), Gegner-Reifenalter/letzter Stopp/Strafe, Lob (lila Sektor, Bestzeit, Konstanz), Grip- und Streckentemperatur-Trend.
- **Qualifying & Training:** Position zur Pole, Verkehrslücke, „jetzt pushen“, reicht die Zeit für noch eine Runde, Fahrverhalten (Unter-/Übersteuern aus Lenkwinkel vs. Gierrate).
- **Vor/nach dem Rennen:** Briefing (Startsprit, Wetter, Drücke), Formationsrunde, Stint- und Rennzusammenfassung (auch als Seite „Rennbericht“).
- **Sprachbefehle:** „Ruhe für 2 Runden“, „Nur noch Spotter“, „Sag mir jede Runde …“, „Erinner mich in 5 Runden an die Box“, „Plan: Box in Runde 18“ (Plan fließt in alle Berechnungen), „Wiederhol das“, „Lauter/Leiser“.
- **Persönlichkeit:** Stil (ruhig, motivierend, trocken-britisch, eigener Freitext), Namen von Ingenieur/Spotter, Anrede, Lob-Häufigkeit, Verhalten nach Fehlern, Zahlen-Genauigkeit, Einheiten.
- **Vollständiger Katalog aller Meldungen, Fragen und Befehle mit `call_id`, Datenquelle und Einstellmöglichkeiten: `CALLS.md`.** Jede Meldung ist eine `call_id` mit Definition in `resources/calls/catalog.json` (Priorität, Cooldown, Ablaufzeit, Schwellen, Stimme, Session-Typen, benötigte Spiel-Fähigkeit). Neue Meldungen = Eintrag im Katalog + Trigger-Funktion + Templates DE/EN + Test.
- **Gesprochene Fragen – freies Gespräch, keine feste Fragenliste:** Der Fahrer kann **jede Frage in jeder Formulierung** stellen, auch eigene, nicht vorgesehene Fragen und Folgefragen. Details siehe Abschnitt „Freies Gespräch mit dem Renningenieur“.

---

## Freies Gespräch mit dem Renningenieur

Ziel: Es soll sich anfühlen wie Funk mit einem echten Ingenieur, nicht wie ein Sprachmenü. „Reicht der Sprit?“, „Komm ich ohne Stopp durch?“, „Muss ich noch tanken?“ und „Wie sieht's mit Benzin aus?“ müssen alle funktionieren – genauso wie „Warum ist der Typ hinter mir in Sektor 2 so schnell?“ oder „Was würde passieren, wenn ich zwei Runden später reinkomme?“.

**Ablauf pro Funkspruch:**
1. **Spracherkennung:** Groq Whisper mit `prompt`-Parameter, der Rennvokabular plus Namen aus der aktuellen Session (Strecke, Kurvennamen, Fahrzeuge, Fahrernamen, Startnummern) enthält, damit diese richtig erkannt werden.
2. **Schneller Weg (optional):** Nur wenn der Satz mit **hoher Sicherheit** einer bekannten Frage entspricht (normalisiert, unscharfer Abgleich, Schwelle einstellbar), sofort deterministisch antworten – ohne Netzwerk. Im Zweifel **immer** zur KI, nie eine falsche feste Antwort geben. Abschaltbar („Immer KI benutzen“).
3. **KI mit Werkzeugen (Standardweg):** Groq-Chat mit **Tool Calling** (OpenAI-kompatibel). Der Systemprompt enthält Persönlichkeit, Sprache, Einheiten und einen kompakten RaceState-Snapshot (Position, Abstände, Sprit, Reifen, Wetter, Restzeit, Flaggen, Strategie-Plan; < 2 KB). Für alles Weitere ruft die KI Werkzeuge auf, die in Rust die echten Daten liefern bzw. rechnen:
   - **Abfragen:** `get_standings`, `get_car` (nach Position, Startnummer oder Name; „der hinter mir“, „der Porsche“), `get_gaps`, `get_lap_history`, `get_sector_comparison`, `get_corner_analysis`, `get_fuel_strategy`, `simulate_pit_stop` (Runde, Liter, Reifen → Rauskomm-Position), `get_tyres`, `get_brakes`, `get_damage`, `get_weather`, `get_session_info`, `get_penalties`, `get_session_summary`.
   - **Aktionen** (für frei formulierte Befehle): `set_quiet`, `set_preset`, `add_recurring_call`, `add_reminder`, `set_strategy_plan`, `repeat_last`, `set_volume`, `toggle_call`, `set_pit_menu` (Spiele mit `hasPitControl`, z. B. iRacing).
   - Werkzeuge und ihre Beschreibungen werden aus einer Registry generiert; neue Fachlogik → neues Werkzeug.
4. **Rechnen nur in Rust:** Die KI rechnet keine Zahlen selbst aus, sondern nutzt die Werkzeuge. Max. 3 Werkzeug-Runden pro Frage.
5. **Gesprächsgedächtnis:** Die letzten Funksprüche der Session (Anzahl einstellbar) werden mitgeschickt, damit Folgefragen funktionieren („Und der dahinter?“, „Und wenn ich eine Runde später komme?“).
6. **Antwort:** Streaming; der erste fertige Satz geht sofort an die Sprachausgabe. Kurz und funkgerecht (Standard max. 2 Sätze, einstellbar), gerundete Zahlen gemäß Persönlichkeit. Dauert es länger als ~1,2 s, kurzes „Moment …“ / „Copy, check ich“ (abschaltbar).
7. **Ehrlichkeit:** Fehlen Daten oder das Spiel liefert sie nicht, sagt der Ingenieur das klar („Dazu hab ich keine Daten“). Nichts erfinden. Bei unklarer Frage kurze Rückfrage („Meinst du P3 oder die Nummer 3?“).
8. **Themenbereich einstellbar:** „Nur Rennen & Auto“ (Standard) oder „Auch allgemeine Fragen“ (z. B. Regeln, Strecke, Fahrtechnik).
9. **Ohne KI:** Bei fehlendem Key, Rate-Limit oder Offline bleibt der schnelle Weg aktiv, und die Crew sagt einmal kurz, dass freie Fragen gerade nicht gehen.
10. **Tests:** Satz-Sammlung mit vielen Umformulierungen pro Thema (DE/EN, Umgangssprache, Tippfehler-artige Erkennungsfehler) in `tests/questions/` – prüft Routing und Werkzeugwahl gegen Replay-Daten.

---

## Design

Modern, dunkel, hochwertig, „Motorsport-Kommandostand“. Keine generische Admin-Oberfläche.

- **Farben (Tokens in `src/styles/tokens.css`):** Hintergrund `#0A0B0E`, Flächen `#12141A` / `#1A1D25`, Rand `#262A35`, Text `#F2F4F8` / `#8A90A0`, Akzent Pit-Orange `#FF5A1F`, Daten-Cyan `#22D3EE`, OK `#22C55E`, Warnung `#F59E0B`, Gefahr `#EF4444`. Sektorfarben: Lila `#A855F7` (Bestzeit gesamt), Grün (persönliche Bestzeit), Gelb (langsamer).
- **Schrift:** Inter (UI), JetBrains Mono mit `tabular-nums` für alle Zahlen und Zeiten. Schriften lokal einbinden, kein CDN.
- **Animationen (viele, aber billig):** Seitenwechsel mit Slide/Fade, gestaffelte Listen, Hover-Glow auf Karten, animierte Zahlen-Ticker, pulsierender „Funk aktiv“-Orb mit Live-Waveform, Spiel-erkannt-Animation, Positionswechsel in der Rangliste mit Layout-Animation. **Nur `transform` und `opacity`**, keine animierten `box-shadow`/`filter`/`width`.
- **Performance-Modus:** Sobald ein Spiel läuft, werden im Hauptfenster dekorative Animationen reduziert und das Fenster drosselt Updates, wenn es minimiert/verdeckt ist. Overlay-Animationen sind dezent und abschaltbar. `prefers-reduced-motion` respektieren.
- Alle UI-Texte über i18n (`src/i18n/de.json`, `en.json`), keine hartcodierten Strings. Sprachausgabe-Texte ebenfalls in Template-Dateien pro Sprache (`resources/calls/de.json`, `en.json`) mit Varianten, damit die Crew nicht immer gleich klingt.

---

## Settings (alles einstellbar)

Groq API-Key (mit „Testen“-Button und Anzeige der Rate-Limit-Header), STT- und LLM-Modell, Sprache (DE/EN), Mikrofon + Pegel-Anzeige + Rauschschwelle, Zuhör-Modus (**VAD dauerhaft / Codewort / Push-to-Talk**, kombinierbar), Codewort (lokal per sherpa-onnx-KWS, englisch klingende Wörter: „Radio“, „Hey Crew“, „Box“, eigene), Funkcheck/Begrüßung zu Session-Beginn, Spotter-Bereich (siehe `CALLS.md`), Stimmen + Funk-Effekt mit Stärke getrennt für Ingenieur/Spotter, Fahrername + Aussprache, Gegner-Bezeichnung (Position/Nummer/Fahrzeug/Name), Funktaste (Lenkrad/Tastatur, „Taste drücken zum Belegen“), Schnellbefehle auf Lenkrad-Tasten, Stimmen (Renningenieur und Spotter getrennt, Lautstärke, Tempo, Funk-Effekt an/aus), Gesprächigkeit über **Presets** („Nur Spotter“, „Minimal“, „Standard“, „Voller Renningenieur“, „Endurance“, „Qualifying“, eigene speicherbar, Standard-Preset pro Spiel), **jede Meldung einzeln** (an/aus, Priorität, Häufigkeit, Cooldown, Schwellen, Stimme, Session-Typen, Vorhören-Button; gruppiert, mit Suche), freies Gespräch (schneller Weg an/aus + Sicherheits-Schwelle, „Immer KI benutzen“, Antwortlänge, Gesprächsgedächtnis, „Moment …“-Füller, Themenbereich, eigene Vokabeln für die Spracherkennung), Sprachbefehle einzeln an/aus, Persönlichkeit, Einheiten, Sprit-Reserve, Undercut-Schwelle, Overlay (Widgets, Deckkraft, Update-Rate, Bearbeiten-Hotkey), Sessions speichern an/aus + Speicherdauer, Autostart mit Windows, Updates (automatisch/manuell).

---

## Projektstruktur

```
RaceEngineerAI/
├─ src/                    # React-Frontend
│  ├─ app/                 # Routing, Layout
│  ├─ pages/               # Dashboard, Games, Session, Analysis, Settings
│  ├─ overlay/             # Overlay-Root + Widgets
│  ├─ components/ui/       # eigene UI-Bausteine
│  ├─ stores/  hooks/  i18n/  styles/  lib/ipc.ts
├─ src-tauri/
│  ├─ src/
│  │  ├─ games/            # detector.rs, provider.rs (Trait), acc/ (shm.rs, broadcast.rs)
│  │  ├─ race/             # state.rs, spotter.rs, gaps.rs, fuel.rs, pit.rs, lapcompare.rs, weather.rs
│  │  ├─ calls/            # engine.rs (Prioritäten, Cooldowns), catalog.rs, templates.rs, triggers/ (flags, gaps, fuel, tyres, quali, …), commands.rs (Sprachbefehle, Erinnerungen, Plan)
│  │  ├─ voice/            # capture.rs, vad.rs, wakeword.rs, ptt.rs, stt_groq.rs, fastpath.rs
│  │  ├─ ai/               # groq.rs, prompts.rs, router.rs, tools/ (Registry + Abfrage-/Aktionswerkzeuge), memory.rs
│  │  ├─ audio/            # tts.rs, mixer.rs, radio_fx.rs
│  │  ├─ input/            # wheel.rs, hotkeys.rs
│  │  ├─ storage/          # db.rs, migrations/, settings.rs, secrets.rs
│  │  ├─ replay/           # recorder.rs, player.rs
│  │  └─ commands.rs  main.rs
│  └─ resources/           # games.json, tracks.json, cars.json, calls/ (catalog.json, de.json, en.json), fastpath/ (Beispielsätze schneller Weg), presets/, sounds/
├─ docs/                   # Recherche-Notizen pro Spiel
├─ tests/questions/        # Umformulierungs-Tests für das freie Gespräch
├─ CLAUDE.md  CALLS.md
```

---

## Befehle

```bash
npm install
npm run tauri dev          # Entwicklung
npm run tauri build        # NSIS-Setup unter src-tauri/target/release/bundle/nsis
npm run lint && npm run typecheck
cd src-tauri && cargo clippy -- -D warnings && cargo test
```

---

## Arbeitsregeln für Claude

- **Alles einstellbar:** Jede Meldung, Schwelle, Frage, jeder Befehl und jede Persönlichkeitsoption kommt in die Settings. Keine fest verdrahteten Zahlen in der Logik – Standardwerte stehen in `catalog.json`/`presets/`.
- **Vor größeren Änderungen kurz planen**, dann in kleinen, lauffähigen Schritten bauen. Nach jedem Schritt `cargo clippy`, `cargo test`, `npm run typecheck` grün halten.
- **Ohne laufendes Spiel testbar machen:** Replay-System (Telemetrie aufzeichnen → Datei → abspielen) und ein Mock-Provider sind Pflicht. Echte Sessions im **Testmodus** (Einstellungen → Über) auf der Seite „Datencheck“ aufnehmen; Aufnahmen in `src-tauri/tests/replays/` werden von `diagnostics::tests` automatisch durch die Rennlogik geprüft. Jede Fachlogik (Spotter, Sprit, Abstände, Kurvenvergleich) bekommt Unit-Tests mit Replay- oder synthetischen Daten.
- **Nichts raten bei Spiel-Schnittstellen.** Struct-Layouts, Offsets, Prozessnamen, UDP-Protokolle aus offizieller Doku/SDK nehmen; Quellen in `docs/<spiel>.md` notieren. Bei Unsicherheit nachfragen.
- **Performance-Budget** (Release-Build, ACC-Rennen läuft): Backend-CPU < 3 % (ein Kern), Gesamt-RAM inkl. WebView2 < 300 MB, Spotter-Latenz < 100 ms, Groq-Antwort auf Frage bis Sprachbeginn < 2 s. Bei neuen Features Auswirkungen prüfen.
- **Groq-Free-Tier schonen:** Nur Sprachabschnitte mit erkannter Sprache senden (min. 0,4 s, max. 15 s), Audio vorher auf 16 kHz Mono, Rate-Limit-Header auswerten, bei 429 freundlich per Funk melden und zurückfallen.
- **Keine Secrets in Logs**, keine Telemetrie an Dritte, kein Tracking.
- Rust: kein `unwrap()` außerhalb von Tests, Fehler mit `thiserror`/`anyhow`. Unsafe-Code (Shared Memory, WinAPI) kapseln und kommentieren.
- TypeScript: `strict`, keine `any`. IPC-Typen zentral in `src/lib/ipc.ts`, Rust-Typen mit `specta`/`ts-rs` generieren statt doppelt pflegen.
- UI-Sprache und Code-Kommentare: Code und Bezeichner auf Englisch, Nutzertexte über i18n (DE + EN).
- Neue Abhängigkeiten nur mit kurzer Begründung (Größe, Wartung, Lizenz).