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
| Rangliste/Abstände | `CarIdx*`-Arrays + YAML `DriverInfo.Drivers` | Kein zweiter Kanal nötig → `hasBroadcastTiming=true`. Sektorzeiten anderer Autos gibt es nicht. |
| Track Limits | – | iRacing hat keinen „Runde ungültig“-Wert, nur Incident-Punkte. `hasTrackLimits=false`. Neue Incidents → Ereignis `Accident`. |
| Reifen | `tempCM`, `wearM` | iRacing aktualisiert diese Werte nur in der Box. Live-Drücke gibt es nicht → `hasTyreCoreTemps=false`, Druck bleibt 0 (Druck-Meldungen sind dadurch still). |
| Bremsen, Schaden | – | nicht vorhanden → 0 (`hasBrakeWear=false`). |
| Licht, Wischer | – | kein Zustand vorhanden → `-1` = unbekannt, die Meldungen „Licht an“/„Wischer an“ bleiben still. |
| Regenreifen montiert | – | `PlayerTireCompound` ist ein Index ohne dokumentierte Bedeutung → `false`. |
| Boxenfenster, Pflichtstopp | – | nicht vorhanden. |

## Sektoren

iRacing teilt Strecken in viele Zeitsektoren (`SplitTimeInfo`). Die Logik arbeitet mit drei Sektoren; der Provider misst daher Drittel der Runde (`LapDistPct`) selbst mit `LapCurrentLapTime`. Gelbe Flaggen werden ebenfalls dem Drittel zugeordnet, in dem ein langsames oder neben der Strecke stehendes Auto liegt.

## Live-Prüfung

`cargo test --test iracing_live -- --ignored --nocapture` (im Auto sitzen) gibt Header, alle Variablennamen, die Sessions aus der YAML, Rohwerte und den fertigen Frame aus. Offen, weil iRacing hier nicht installiert ist:

1. Vorzeichen von `Yaw` (Karte gespiegelt?) und Richtung von `SteeringWheelAngle`.
2. `SessionLapsRemainEx` zählt die laufende Runde mit? (`laps_left = Rest − LapDistPct`)
3. `WeekendInfo.SubSessionID` ist offline 0 (→ `online`).
4. Namen der wagenabhängigen Variablen `dcTractionControl` / `dcABS` bei GT3-Autos.
