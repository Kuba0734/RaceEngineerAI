# Assetto Corsa Competizione – Schnittstellen

Alle Struct-Layouts und Protokolle in `src-tauri/src/games/acc/` sind aus den folgenden Quellen nachgebaut. Nichts davon ist geraten.

## Verifiziert auf diesem Rechner (2026-11-01)

| Was | Wert | Quelle |
|---|---|---|
| Prozessname | `AC2-Win64-Shipping.exe` | `D:\SteamLibrary\steamapps\common\Assetto Corsa Competizione\AC2\Binaries\Win64\AC2-Win64-Shipping.exe` (Launcher: `acc.exe`) |
| Spielversion | 1.10.4 | `changelog.txt` im Spielordner |
| Broadcasting-SDK | Protokollversion 4 | `Assetto Corsa Competizione Dedicated Server\sdk\broadcasting\Sources\ksBroadcastingNetwork\BroadcastingNetworkProtocol.cs` (offiziell von Kunos, seit 1.5.7 im Steam-Tool „ACC Dedicated Server“) |

## Shared Memory

**Quelle:** *ACC Shared Memory Documentation v1.8.12* (Kunos Simulazioni), offizielles Forum
<https://www.assettocorsa.net/forum/index.php?threads/acc-shared-memory-documentation.59965/>,
Spiegel der PDF: <https://github.com/rrennoir/PyAccSharedMemory/blob/main/ACCSharedMemoryDocumentationV1.8.12.pdf>.

- Mappings: `Local\acpmf_physics` (`SPageFilePhysics`), `Local\acpmf_graphics` (`SPageFileGraphic`), `Local\acpmf_static` (`SPageFileStatic`).
- Packing: 4 Byte (`#pragma pack(4)` im originalen AC-Header `SharedFileOut.h`) → `#[repr(C, packed(4))]`.
- Strings: `wchar_t[N]` = UTF-16 → `[u16; N]`.
- Enums (`ACC_STATUS`, `ACC_SESSION_TYPE`, `ACC_FLAG_TYPE`, `ACC_PENALTY_TYPE`, `ACC_TRACK_GRIP_STATUS`, `ACC_RAIN_INTENSITY`) sind `int` (4 Byte).
- Reihenfolge der Felder 1:1 wie in der PDF. Die Offsets werden in `shm.rs` per Test festgeschrieben.

### Bekannte Eigenheiten / Abweichungen von der PDF

| Feld | PDF | Umsetzung | Begründung |
|---|---|---|---|
| `SPageFileStatic.trackConfiguration` | `wchar_t trackConfiguration` (ohne Länge) | `[u16; 33]` | Im AC-Header `SharedFileOut.h` ist das Feld `wchar_t trackConfiguration[33]`; die PDF lässt die Länge weg. Alle nachfolgenden Felder (Boxenfenster) hängen davon ab. **Live mit ACC prüfen** (Boxenfenster-Werte plausibel?). |
| `SPageFilePhysics.fuel` | „in kg“ | als Liter behandelt | `fuelXLap` ist laut PDF in Litern, ACC zeigt Liter; Kunos-Forum bestätigt Liter. **Live prüfen.** |
| `SPageFileGraphic.carCoordinates[60][3]` | x, y, z | x, y, z (y = Höhe) | Für Spotter/Karte werden x und z genutzt. |
| `SPageFilePhysics.heading` | Gierwinkel | Radiant | Für die Drehung ins Spieler-Koordinatensystem. |

Gegner-Fahrtrichtung liefert Shared Memory nicht – der Spotter nutzt nur die Spieler-Ausrichtung (`heading`) und die Positionen. Für Gegner-Ausrichtung auf der Karte gibt die Broadcasting-API `Yaw`.

## Broadcasting-UDP-API

**Quelle:** offizielles SDK `ksBroadcastingNetwork` (C#), Dateien `BroadcastingNetworkProtocol.cs`, `BroadcastingEnums.cs`, `Structs/*.cs`, `ACCUdpRemoteClient.cs`.

- Transport: UDP, ein Datagramm = eine Nachricht, erstes Byte = Nachrichtentyp. Little Endian (`BinaryReader`).
- Strings: `u16`-Länge + UTF-8-Bytes.
- Ausgehend: `REGISTER_COMMAND_APPLICATION=1` (Protokollversion 4, displayName, connectionPassword, `i32` msRealtimeUpdateInterval, commandPassword), `UNREGISTER=9`, `REQUEST_ENTRY_LIST=10`, `REQUEST_TRACK_DATA=11` (jeweils + `i32` connectionId).
- Eingehend: `REGISTRATION_RESULT=1`, `REALTIME_UPDATE=2`, `REALTIME_CAR_UPDATE=3`, `ENTRY_LIST=4`, `TRACK_DATA=5`, `ENTRY_LIST_CAR=6`, `BROADCASTING_EVENT=7`. Feldreihenfolge exakt wie in `ProcessMessage`.
- Lap-Struktur: `i32` laptime, `u16` carIndex, `u16` driverIndex, `u8` splitCount + `i32`×n, 4×`u8` (invalid, validForBest, outlap, inlap). `i32::MAX` = kein Wert.
- `REALTIME_CAR_UPDATE` für unbekannte Autos → Entry-List neu anfordern (max. 1×/s), wie im SDK.
- Achtung: In `REGISTRATION_RESULT` ist `isReadonly = (byte == 0)` – so im SDK.

### `broadcasting.json`

Pfad: `Dokumente\Assetto Corsa Competizione\Config\broadcasting.json`, **UTF-16 LE** kodiert. Felder: `updListenerPort`, `connectionPassword`, `commandPassword`. Quelle: `sdk\broadcasting\Testclient\readme.txt` im Dedicated-Server-Ordner. RaceEngineerAI legt vorher ein Backup (`broadcasting.json.bak-<zeit>`) an, fragt den Nutzer und weist auf den ACC-Neustart hin.

Hinweis: Auf diesem Rechner existiert der Ordner `Dokumente\Assetto Corsa Competizione` noch nicht (wird von ACC beim ersten Start angelegt).

## Offene Punkte (live mit ACC prüfen)

- Update-Rate von `carCoordinates` (Grafik-Schritt, also an die FPS gekoppelt?) – entscheidend für die Spotter-Latenz.
- `trackConfiguration`-Länge (siehe oben).
- Ob `gapAhead`/`gapBehind` in allen Session-Typen gefüllt sind.
