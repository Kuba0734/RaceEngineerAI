# iRacing – Schnittstellen

Umsetzung in `src-tauri/src/games/iracing/`. Layouts und Namen stammen aus den Quellen unten, nichts davon ist geraten.
**iRacing ist auf dem Entwicklungsrechner nicht installiert** – alles mit „live prüfen“ muss beim ersten Start mit
`cargo test --test iracing_live -- --ignored --nocapture` bestätigt werden.

## Quellen

| Was | Quelle |
|---|---|
| Header-Layout, Konstanten, Enums | `irsdk_defines.h` aus dem offiziellen iRacing-SDK (© 2013 iRacing.com Motorsport Simulations, BSD-Lizenz). Unveränderte Kopie im npm-Paket `iracing-sdk-js@1.5.0`, Pfad `src/cpp/irsdk/irsdk_defines.h` (<https://unpkg.com/iracing-sdk-js@1.5.0/src/cpp/irsdk/irsdk_defines.h>). Das Original liegt im iRacing-Mitgliederforum (Login nötig). |
| Namen, Einheiten der Telemetrie-Variablen | Live-Dump der Variablen-Header des Sims im Projekt pyirsdk: <https://github.com/kutu/pyirsdk/blob/master/vars.txt> (Stand 2026-06-09) |
| Schlüssel der Session-YAML | <https://sajax.github.io/irsdkdocs/yaml/> (WeekendInfo, SessionInfo, DriverInfo, SplitTimeInfo) |
| Werteformate der YAML (`3.98 km`, `300.00 sec`, `unlimited`) | Session-Dump in <https://gist.github.com/brookslyrette/8ae5207984318bab1f18> |
| Prozessname `iRacingSim64DX11.exe` | iRacing-Support: <https://support.iracing.com/support/solutions/articles/31000162005-failed-createprocessasuser-error>; CrewChief nutzt denselben Namen |

## Shared Memory

- Mapping `Local\IRSDKMemMapFileName` (Event `Local\IRSDKDataValidEvent` wird nicht benutzt – wir pollen `tickCount`, die Telemetrie-Schleife läuft ohnehin).
- `irsdk_header` (112 Byte, natürliche C-Ausrichtung, alles `int`):
  `ver, status, tickRate, sessionInfoUpdate, sessionInfoLen, sessionInfoOffset, numVars, varHeaderOffset, numBuf, bufLen, pad1[2], varBuf[4]`.
  `irsdk_varBuf` = `tickCount, bufOffset, pad[2]` (16 Byte).
- `irsdk_varHeader` (144 Byte): `type, offset, count, bool countAsTime, pad[3], name[32], desc[64], unit[32]`.
- `IRSDK_VER = 2`, `status & 1` = verbunden. Typen: `char=0, bool=1, int=2, bitField=3, float=4, double=8 Byte (5)`.
- Lesen wie im SDK (`irsdk_getNewData`): Puffer mit höchstem `tickCount` kopieren, danach prüfen, dass `tickCount` unverändert ist, sonst nochmal.
- Variablen werden **per Name** gesucht (die Liste hängt vom Auto ab). Fehlt eine Variable, bleibt der Wert leer – kein Absturz.
- Session-YAML: `sessionInfoOffset`/`sessionInfoLen`, neu lesen wenn `sessionInfoUpdate` sich ändert. Kodierung ISO-8859-1 (byteweise → `char`).
- Der Sim schreibt mit 60 Hz (`tickRate`); das reicht für den Spotter.

## Benutzte Variablen

`Speed` (m/s), `RPM`, `Gear` (-1 R, 0 N), `Throttle`, `Brake`, `SteeringWheelAngle` (rad), `Yaw` (rad), `YawRate` (rad/s),
`FuelLevel` (l), `Lap`, `LapCompleted`, `LapDistPct` (0–1), `LapCurrentLapTime`, `LapLastLapTime`, `LapBestLapTime` (s),
`LapDeltaToBestLap` (s) + `LapDeltaToBestLap_OK`, `PlayerCarIdx`, `PlayerCarPosition`, `OnPitRoad`, `PlayerCarInPitStall`,
`PlayerTrackSurface` (irsdk_TrkLoc), `IsOnTrack`, `IsReplayPlaying`, `SessionNum`, `SessionState`, `SessionFlags` (irsdk_Flags),
`SessionTimeRemain` (s), `SessionTimeOfDay` (s), `CarLeftRight` (irsdk_CarLeftRight), `BrakeABSactive`, `AirTemp`, `TrackTempCrew`,
`TrackWetness`, `Precipitation` (0–1), `WindVel`, `PlayerCarMyIncidentCount`,
`CarIdxLapDistPct`, `CarIdxLap`, `CarIdxLapCompleted`, `CarIdxPosition`, `CarIdxClassPosition`, `CarIdxOnPitRoad`, `CarIdxTrackSurface`,
`CarIdxLastLapTime`, `CarIdxBestLapTime`, `CarIdxEstTime`, `CarIdxSessionFlags`.
Wagenabhängig, nur wenn vorhanden: `dcTractionControl`, `dcABS`, `[LF|RF|LR|RR]tempCM`, `[..]wearM`, `[..]coldPressure` (kPa).

## Abbildung auf das spielunabhängige Modell

| Modell | iRacing | Hinweis |
|---|---|---|
| Spotter | `CarLeftRight` (nativ) | `LRCarLeft`/`LR2CarsLeft` → links, `LRCarRight`/`LR2CarsRight` → rechts, `LRCarLeftRight` → drei breit, `LROff` → Spotter aus. Fähigkeit `hasNativeSpotter`. Radar: Längsabstand aus `CarIdxLapDistPct` × Streckenlänge, Seite aus `CarLeftRight`. |
| Position auf der Karte | – | Live gibt es **keine Weltkoordinaten** (`Lat`/`Lon` nur in .ibt-Dateien). Die Strecke wird aus der eigenen Runde gekoppelt (`Speed` × Richtung aus `Yaw`, 60 Hz) und der Schließfehler über die Runde verteilt. Gegner werden über `CarIdxLapDistPct` auf diese Linie gesetzt. **Live prüfen:** Vorzeichen von `Yaw` (falsches Vorzeichen = gespiegelte Karte). |
| Flaggen | `SessionFlags` | Rangfolge: checkered > black/disqualify > repair (Meatball → `orange`, wie ACC) > blue > yellow/yellowWaving/caution/cautionWaving > white > green. `caution`/`cautionWaving` setzen zusätzlich `global_yellow` (Full Course Yellow). |
| Session-Typ | YAML `SessionType` | `Practice`, `Open Qualify`, `Lone Qualify`, `Race`, `Warmup`, `Offline Testing` |
| Phase | `SessionState` | GetInCar/Warmup → PreSession, ParadeLaps → Formation, Racing → Session, Checkered → SessionOver, CoolDown → PostSession |
| Strecke | YAML `TrackName` (Schlüssel, wie bei ACC), `TrackLength` (`"5.51 km"`) | |
| Sektoren | YAML `SplitTimeInfo.Sectors[].SectorStartPct` | iRacing hat meist mehr als 3 Sektoren → auf 3 gleich große Gruppen zusammengefasst. |
| Sprit max. | YAML `DriverCarFuelMaxLtr × DriverCarMaxFuelPct` | |
| Grip | `TrackWetness` | Dry→Optimum, MostlyDry→Greasy, VeryLightly/LightlyWet→Damp, Moderately/VeryWet→Wet, ExtremelyWet→Flooded |
| Regen jetzt | `Precipitation` | 0 → keiner, <0,1 Niesel, <0,3 leicht, <0,6 mittel, sonst stark. **Keine Vorhersage** (`hasWeatherForecast=false`). |
| Rangliste/Abstände | `CarIdx*`-Arrays + YAML `DriverInfo.Drivers` | Kein zweiter Kanal nötig → `hasBroadcastTiming=true`. Offizieller Abstand zum Führenden aus `CarIdxF2Time` (nur im Rennen). |
| Sektorzeiten aller Autos | – (selbst gemessen) | Der Provider misst für jedes Auto die drei Drittel der Runde aus `CarIdxLapDistPct` + `SessionTime` (60 Hz, ±16 ms). Runden über die Boxengasse zählen nicht als Bestzeit. |
| Karte | gekoppelt + gespeichert | Siehe oben; die fertige Linie wird pro Strecke unter `%LOCALAPPDATA%\RaceEngineerAI\maps\iracing\` gespeichert und beim nächsten Mal sofort geladen. |
| Track Limits | `PlayerTrackSurface` | Vier Räder neben der Strecke (`irsdk_OffTrack`) = Track Limit, die Runde gilt als ungültig. `hasTrackLimits=true`. |
| Incidents | `PlayerCarMyIncidentCount` (Team: `PlayerCarTeamIncidentCount`), YAML `WeekendOptions.IncidentLimit` | Meldung `penalty.incidents`. |
| Reifen | `tempCL/CM/CR`, `wearL/M/R`, `coldPressure` | iRacing gibt **keine Live-Reifenwerte** heraus; die Werte ändern sich nur in der Box. Der Provider hält sie 1 s nach dem Anhalten in der Box fest (`TyreCheck`, innen/mitte/außen – bei linken Rädern ist „L“ außen). Overlay zeigt sie als „Stand letzter Boxenstopp“, Meldung `tyre.after_stop`. Live-Drücke: nicht vorhanden. |
| Heißdrücke | YAML `CarSetup.*.{LeftFront,…}.LastHotPressure` | Werden gesucht, egal wie das Setup des Autos verschachtelt ist (kPa, psi oder bar). Eine neue Messung wird an den Reifencheck des Stopps gehängt → Meldung `tyre.pressure_next_stint` (Abweichung vom Zielfenster aus `cars.json`), Overlay zeigt die Heißdrücke. |
| Bremsen | – | iRacing gibt weder Bremstemperatur noch Verschleiß heraus (nur `brakeLinePress`). |
| Schaden | `PitRepairLeft`, `PitOptRepairLeft`, `FastRepairAvailable`, `EngineWarnings` (mandRepNeeded) | Keine Schadenszonen, aber die echte Reparaturzeit → Meldung `damage.repair` und Abwägung in `damage.report`. |
| Motor | `EngineWarnings`, `WaterTemp`, `OilTemp` | Meldung `car.engine`. |
| Boxenmenü | `PitSvFlags`, `PitSvFuel` | Meldung `pit.menu_check` bei Boxeneinfahrt, auch im KI-Werkzeug `get_fuel_strategy`. |
| Safety Car | `SessionFlags` (caution, oneLapToGreen), `CarIdxPaceFlags` | FCY wie bei ACC, dazu `flag.pace`. |
| Regenreifen | `PlayerTireCompound`, `CarIdxTireCompound` + YAML `DriverInfo.DriverTires[].TireCompoundType` | **Live prüfen:** `DriverTires` ist nicht dokumentiert; ohne den Schlüssel bleibt „Regenreifen montiert“ unbekannt. `WeatherDeclaredWet` sperrt die Regenreifen-Empfehlung, bis nass freigegeben ist. |
| Regenprognose | – | iRacing gibt keine Vorhersage heraus, nur Regen und Nässe jetzt. |
| Licht, Wischer, Stint-Restzeit | – | kein Zustand vorhanden → `-1` = unbekannt, „Licht an“/„Wischer an“ bleiben still; iRacing kennt keine Stint-Grenze. |
| Strafen | `SessionFlags` black/disqualify | Die Art (Durchfahrt, Stop-and-Go) gibt iRacing nicht heraus, nur „schwarze Flagge“. |
| Boxenfenster, Pflichtstopp | – | Nicht im SDK. Pro Spiel in den Einstellungen (Spiele-Seite) eintragbar; die App zählt die eigenen Stopps. |

## Sektoren

iRacing teilt Strecken in viele Zeitsektoren (`SplitTimeInfo`). Die Logik arbeitet mit drei Sektoren; der Provider misst daher Drittel der Runde (`LapDistPct`) selbst mit `LapCurrentLapTime`. Gelbe Flaggen werden ebenfalls dem Drittel zugeordnet, in dem ein langsames oder neben der Strecke stehendes Auto liegt.

## Live-Prüfung

`cargo test --test iracing_live -- --ignored --nocapture` (im Auto sitzen) gibt Header, alle Variablennamen, die Sessions aus der YAML, Rohwerte und den fertigen Frame aus. Mit `IRACING_LIVE_SECONDS=240` läuft er länger; nach einer vollen sauberen Runde meldet er die Fahrtrichtung der gekoppelten Karte.

### Live verifiziert (2026-11-02, Nürburgring GP, Porsche 911 GT3 R (992), Offline-Test)

| Punkt | Ergebnis |
|---|---|
| Header | `ver=2`, `status=1`, `tickRate=60`, `numBuf=3`, 335 Variablen – Layout passt |
| Session-YAML | Strecke `nurburgring gp`, 5068,5 m, Session `Practice` unbegrenzt, Spielername, Startnummer, Auto korrekt |
| Unbegrenzte Zeit/Runden | `SessionTimeRemain=604800`, `SessionLapsRemainEx=32767` → als „unbegrenzt“ erkannt |
| `CarLeftRight` | `1` (Clear) im Auto |
| `dcTractionControl`, `dcABS` | vorhanden (GT3) |
| `SteeringWheelAngleMax` | 9,89 rad |
| `WeekendInfo.SubSessionID` offline | → `online=false` |
| `CarClassShortName` | **offline leer** → Klasse wird jetzt zusätzlich aus Autoname/-pfad abgeleitet |
| `LapCompleted` | **-1** bis zur ersten Überfahrt der Linie aus der Box → wird auf 0 begrenzt |
| `SessionFlags` in der Box | `0x10040000` = startHidden + servicible → keine Flagge |

### Zweiter Live-Test (2026-11-02, gleiche Session, Auto in der Box)

| Punkt | Ergebnis |
|---|---|
| `DriverInfo.DriverTires` | vorhanden: `TireIndex 0 = Hard`, `1 = Wet` → Regenreifen-Erkennung bestätigt |
| `WeekendOptions.IncidentLimit` | `unlimited` → kein Limit |
| `FastRepairAvailable` | `0`, obwohl `FastRepairsLimit: unlimited` → Limit aus der YAML hat Vorrang |
| `EngineWarnings` | `14` mit stehendem Motor (Öldruck + Spritdruck + abgewürgt) → Druckwarnungen nur bei laufendem Motor |
| `SessionFlags` | `0x10040200`: „one lap to green“ auch im Test ohne Pace Car → nur zusammen mit `PaceMode` ≠ 4 (NotPacing) |
| Reifenwerte in der Box | `tempCL/CM/CR` und `wearL/M/R` plausibel (z. B. LF 86/81/71 °C, 94–97 % Profil) |
| `IsReplayPlaying` | `1` außerhalb des Autos → Status „Replay“, keine Ansagen |
| `CarSetup.TiresAero.*.LastHotPressure` | vorhanden (`159 kPa`), aber nach 4 Runden **noch gleich dem Startdruck** und `LastTempsOMI` noch `40C` – iRacing hat sie nicht sofort aktualisiert. Genutzt wird deshalb nur eine **Änderung** gegenüber dem ersten gesehenen Wert. **Live prüfen:** Wann aktualisiert iRacing die Werte (normale Boxeneinfahrt vs. Zurücksetzen)? |

### Noch offen

1. Vorzeichen von `Yaw` (Karte gespiegelt?) – mit `IRACING_LIVE_SECONDS` eine Runde auf einer bekannten Strecke fahren. Nürburgring GP wird im Uhrzeigersinn gefahren.
2. Richtung von `SteeringWheelAngle` (Vorzeichen links/rechts).
3. `SessionLapsRemainEx` in einem Rennen nach Runden: zählt es die laufende Runde mit? (`laps_left = Rest − LapDistPct`)
4. `online` in einer gehosteten/offiziellen Session.
5. Spotter, Rangliste und Abstände mit Gegnern (KI-Rennen).
