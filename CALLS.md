# CALLS.md – Was der Renningenieur sagt und beantwortet

Katalog aller Funkmeldungen, Fragen und Sprachbefehle. **Jede Meldungsart ist in den Settings einzeln einstellbar** (siehe „Einstellbarkeit“ unten). Texte stehen nicht im Code, sondern in `resources/calls/de.json` und `en.json` mit mehreren Varianten pro Meldung.

Spalte **Daten** = woher die Info kommt (ACC). „abgeleitet“ = RaceEngineerAI berechnet es selbst. Für spätere Spiele über `games.json`-Fähigkeiten prüfen, ob eine Meldung verfügbar ist; nicht verfügbare Meldungen werden im UI ausgegraut mit Hinweis.

---

## Einstellbarkeit (gilt für jede Meldung)

Jede Meldungsart (`call_id`) hat in den Settings:
- **An/Aus**
- **Priorität** (Spotter / Kritisch / Hoch / Normal / Info) – Standardwert vorgegeben, änderbar außer Spotter
- **Häufigkeit**: jedes Mal / jede N-te Runde / nur auf Nachfrage
- **Cooldown** in Sekunden und **Ablaufzeit** (danach verworfen)
- **Schwellenwerte**, wo sinnvoll (in den Tabellen unten als *Schwelle* markiert)
- **Stimme**: Renningenieur oder Spotter
- **Session-Typen**: Training / Qualifying / Rennen (z. B. Lob nur im Rennen)
- **Vorhören**-Button in den Settings, der die Meldung mit Beispieldaten abspielt

Dazu gibt es **Presets**: „Nur Spotter“, „Minimal“, „Standard“, „Voller Renningenieur“, „Endurance“, „Qualifying“. Eigene Presets speicherbar, pro Spiel ein Standard-Preset wählbar.

Die Settings-Seite zeigt den Katalog gruppiert nach den Abschnitten unten, mit Suche und „Alle in Gruppe an/aus“.

---

## 0. Session-Start & Funkcheck
| call_id | Beispiel | Daten | Einstellbar |
|---|---|---|---|
| `session.radio_check` | „Radio Check, Kuba. Hier ist Max, hörst du mich?“ | Session-Beginn (Spiel erkannt + Session live) | an/aus, wann (jede Session / nur erste des Tages / nur Rennen), Antwort abwarten (Fahrer sagt „Laut und klar“ → „Copy, viel Erfolg“) |
| `session.greeting` | „Willkommen in Spa. Training, 60 Minuten, trocken, 24 Grad Strecke.“ | Session-Info + Wetter | an/aus, Umfang (kurz / mit Wetter / mit Briefing) |
| `session.spotter_intro` | „Spotter ist da, ich pass auf die Seiten auf.“ | Session-Beginn | an/aus |

**Funkstille beim Rennstart:** Der Ingenieur spricht bis kurz vor dem Start (einstellbar: Anteil der Formationsrunde) und schweigt dann bis nach der ersten Rennrunde (einstellbar). Der Spotter, kritische Meldungen und Antworten auf Fragen des Fahrers bleiben aktiv.

## 1. Spotter
| call_id | Beispiel | Daten | Einstellbar |
|---|---|---|---|
| `spotter.left` / `right` / `three_wide` / `clear` | „Auto links“ – „Drei breit“ – „Frei“ | abgeleitet aus Koordinaten | Hysterese, Seitenabstand, „Frei“ an/aus |
| `spotter.closing_fast` | „Kommt schnell von hinten“ | abgeleitet (Annäherung) | *Schwelle* km/h Differenz |
| `spotter.still_there` | „Immer noch da“ (bei langem Seite-an-Seite) | abgeleitet | Intervall |

**Spotter-Einstellungen (eigener Bereich in den Settings):** Name des Spotters, eigene Stimme, Lautstärke, Tempo, eigener Funk-Effekt + Stärke, Satzlänge (kurz „Links!“ / normal „Auto links“ / doppelt „Auto links, links“), Seitenabstand quer (min/max m), Längsbereich (Autolänge), Hysterese (ms), „Frei“ an/aus, „Immer noch da“-Intervall, „Kommt schnell“-Schwelle, in Box/Stand aus, andere Meldungen beim Spotter absenken (Ducking-Stärke), Spotter auch im Training/Quali an/aus.

## 2. Flaggen & Rennleitung
| call_id | Beispiel | Daten | Einstellbar |
|---|---|---|---|
| `flag.blue_leader` | „Führender kommt, 2 Sekunden hinter dir“ | Broadcasting (Position/Runden) + abgeleitet | *Schwelle* Sekunden, Empfehlung „lass ihn in Kurve X vorbei“ an/aus |
| `flag.yellow_ahead` | „Gelb in Sektor 2, Auto steht rechts nach Kurve 7“ | `graphics.flag`, `globalYellow1..3` + stehende Autos (Geschwindigkeit ≈ 0 auf Strecke) | Vorwarnzeit |
| `flag.fcy` | „FCY! Box diese Runde, Stopp ist jetzt billig“ | Flaggen/Rennleitung | Box-Empfehlung an/aus |
| `flag.green` | „Grün, grün, grün!“ | Flagge/Session-Phase | – |
| `flag.white` / `flag.checkered` | „Letzte Runde“ / „Zielflagge, P4, gut gemacht“ | Session | – |
| `penalty.track_limits` | „Track Limit Nummer 3, beim nächsten gibt's Strafe“ | abgeleitet aus `isValidLap`/Penalty-Feldern | Zähler-Warnung ab N |
| `penalty.received` | „Drive-Through, innerhalb von 3 Runden absitzen“ | `graphics.penalty` | – |
| `penalty.reminder` | „Strafe noch offen, diese Runde rein“ | abgeleitet | Erinnerungsrunde |

## 3. Rennzeit & Abstände
| call_id | Beispiel | Daten | Einstellbar |
|---|---|---|---|
| `race.time_left` | „Noch 10 Minuten, etwa 5 Runden“ | `sessionTimeLeft` + Rundenschnitt | Zeitpunkte (z. B. 30/10/5 min) |
| `gap.ahead` / `gap.behind` | „Abstand nach vorne 1,4, du holst 2 Zehntel“ | positionsbasiertes Timing | Häufigkeit, nur bei Änderung > *Schwelle* |
| `gap.trend_rival` | „P3 holt auf, 4 Zehntel pro Runde“ / „P5 fällt zurück“ | abgeleitet | *Schwelle* Zehntel/Runde, Anzahl Autos vor/hinter |
| `gap.catch_prediction` | „Bei 1:48,5 bist du in 6 Runden dran“ | abgeleitet | an/aus, Mindest-Restrunden |
| `rival.sector_strength` | „Der hinter dir ist in Sektor 2 schneller, verteidige dort“ | Sektorzeiten Broadcasting | an/aus, *Schwelle* |
| `rival.attack_hint` | „In Sektor 3 bist du schneller, greif in der Schikane an“ | Sektorzeiten | an/aus |
| `rival.info` | Reifenalter, letzter Stopp, offene Strafe des Gegners | Broadcasting + abgeleitet | welche Infos |

## 4. Sprit & Boxenstopp
| call_id | Beispiel | Daten | Einstellbar |
|---|---|---|---|
| `fuel.status` | „Sprit reicht für 12 Runden, brauchst 11“ | `fuel`, Verbrauch abgeleitet | Häufigkeit |
| `fuel.critical` | „Sprit wird knapp, 2 Runden übrig“ | abgeleitet | *Schwelle* Runden |
| `fuel.save_target` | „Spar 0,2 Liter pro Runde, dann ohne Stopp. Lift and Coast vor Kurve 1 und 8“ | abgeleitet + Kurvendaten | an/aus, max. Sparziel |
| `pit.window` | „Boxenfenster offen“ / „Pflichtstopp noch nicht erledigt“ | `pitWindowStart/End`, `mandatoryPitDone` | – |
| `pit.fuel_amount` | „Nachtanken: 38 Liter“ | abgeleitet | Reserve (Runden/Liter) |
| `pit.exit_position` | „Nach dem Stopp kommst du als P7 raus, hinter der 88“ | Box-Verlustzeit + Abstände | an/aus |
| `pit.rival_pitting` | „P3 fährt an die Box“ | Broadcasting `CarLocation` | nur Autos innerhalb N Positionen |
| `pit.undercut` | „Undercut-Gefahr: der hinter dir ist rein“ | abgeleitet | *Schwelle* Sekunden |
| `pit.plan` | „Plan: Box in Runde 18“ (vom Fahrer gesetzt) | Sprachbefehl | Erinnerung N Runden vorher |
| `prerace.start_fuel` | „Startsprit fürs nächste Rennen: 64 Liter“ | abgeleitet | Formationsrunde, Reserve |

## 5. Reifen, Bremsen, Auto
| call_id | Beispiel | Daten | Einstellbar |
|---|---|---|---|
| `tyre.cold` | „Reifen noch kalt, vorsichtig in Kurve 1“ | `tyreCoreTemperature` | *Schwelle* °C pro Klasse |
| `tyre.overheat` | „Vorne links überhitzt, schon die Reifen“ | Temperaturen | *Schwelle* |
| `tyre.pressure_live` | „Drücke vorne zu hoch, 28,1“ | `wheelsPressure` | Zielfenster (`cars.json`) |
| `tyre.pressure_next_stint` | „Nächster Stint: vorne links −0,3“ | abgeleitet | an/aus |
| `tyre.wet_switch` | „Regen kommt in 10 Minuten, Regenreifen beim nächsten Stopp“ | Regenprognose + `trackGripStatus` | Aggressivität (früh/normal/spät) |
| `brake.wear` | „Bremsbeläge reichen noch 2 Stints“ | `padLife`, `discLife` | *Schwelle* |
| `brake.temp` | „Bremsen vorne sehr heiß“ | `brakeTemp` | *Schwelle* |
| `damage.report` | „Schaden kostet ~4 Zehntel/Runde, Reparatur 25 s – lohnt sich nicht“ | `carDamage` + Pace-Vergleich | an/aus |
| `assist.tc` | „TC greift viel in Kurve 6, probier TC eins runter“ | TC-Eingriffe + Kurven | *Schwelle*, nur Training/Rennen |
| `assist.lockup` | „Du blockierst vorne beim Anbremsen, Balance einen Klick nach hinten“ | `wheelSlip`, Bremse | *Schwelle* |
| `car.lights` / `car.wipers` | „Wird dunkel, Licht an“ / „Regen, Scheibenwischer“ | Tageszeit, Lichtstatus, Regen | – |
| `car.stint_time` | „Noch 10 Minuten Fahrzeit in diesem Stint“ | `driverStintTimeLeft` | Zeitpunkte |

## 6. Strecke & Wetter
| call_id | Beispiel | Daten | Einstellbar |
|---|---|---|---|
| `weather.forecast` | „Leichter Regen in 10 Minuten“ | `rainIntensityIn10min/30min` | Vorwarnzeit |
| `track.grip` | „Strecke wird schneller“ / „Strecke trocknet ab“ | `trackGripStatus` | – |
| `track.temp` | „Streckentemperatur fällt, Drücke im Auge behalten“ | `roadTemp` | *Schwelle* °C |

## 7. Performance & Fahrstil
| call_id | Beispiel | Daten | Einstellbar |
|---|---|---|---|
| `lap.corner_feedback` | „Kurve 3: zu früh gebremst, etwa 8 Meter“ | Kurvenvergleich | max. Punkte pro Runde (1–3), *Schwelle* Zeitverlust |
| `lap.biggest_loss` | „Meiste Zeit in Kurve 11, 3 Zehntel zur Bestzeit“ | Kurvenvergleich | – |
| `lap.praise_sector` | „Lila im zweiten Sektor, stark!“ | Sektorzeiten | an/aus |
| `lap.personal_best` | „Persönliche Bestzeit, 1:47,9“ | Rundenzeit | – |
| `lap.consistency` | „Letzte 5 Runden innerhalb 2 Zehntel“ | abgeleitet | Fenster, *Schwelle* |
| `setup.handling` | „Untersteuern im Kurveneingang, besonders langsame Kurven“ | Lenkwinkel vs. Gierrate | nur Training/Quali |
| `driver.after_mistake` | „Abhaken, die Pace ist gut“ | Dreher/Ausritt erkannt | Stil (siehe Persönlichkeit) |

## 8. Qualifying & Training
| call_id | Beispiel | Daten | Einstellbar |
|---|---|---|---|
| `quali.position` | „P5, Pole ist 4 Zehntel schneller“ | Broadcasting | – |
| `quali.traffic_gap` | „Verkehr vorne, lass dir 8 Sekunden Platz“ | abgeleitet | Wunschabstand |
| `quali.push_now` | „Lücke frei, Reifen auf Temperatur, jetzt pushen“ | Abstände + Reifentemp | – |
| `quali.time_for_lap` | „Noch 4 Minuten, reicht für genau eine Runde“ | Restzeit + Rundenzeit | – |

## 9. Vor & nach dem Rennen
| call_id | Beispiel | Daten | Einstellbar |
|---|---|---|---|
| `prerace.briefing` | Startsprit, Wetter für Renndauer, Druck-Empfehlung | abgeleitet | Umfang |
| `prerace.formation` | „Formationsrunde, Reifen anwärmen“ | Session-Phase | – |
| `stint.summary` | „Stint-Schnitt 1:48,6, Verbrauch 2,9, Reifenabbau gering“ | abgeleitet | an/aus |
| `race.summary` | „P4, 3 Plätze gewonnen, beste Runde 1:47,9, meiste Zeit in Kurve 11“ | abgeleitet | an/aus; zusätzlich als Seite „Rennbericht“ in der App |

---

## Fragen des Fahrers – freies Gespräch

**Es gibt keine feste Fragenliste.** Der Fahrer kann alles fragen, in jeder Formulierung, auch Folgefragen und eigene Fragen. Die KI versteht den Satz und holt sich die Antwort über Werkzeuge aus der Rennlogik (Ablauf siehe `CLAUDE.md`, Abschnitt „Freies Gespräch mit dem Renningenieur“).

Die folgenden Beispiele sind nur **Testfälle und Ideen**, keine Grenze. Jedes Thema muss in vielen Umformulierungen funktionieren:

| Thema | Beispiele für unterschiedliche Formulierungen |
|---|---|
| Sprit | „Reicht der Sprit?“ · „Komm ich ohne Stopp durch?“ · „Muss ich noch tanken?“ · „Wie sieht's mit Benzin aus?“ · „Wie viel Liter brauch ich noch?“ |
| Abstände | „Abstand nach vorne?“ · „Wie weit ist der vor mir weg?“ · „Komm ich an den ran?“ · „Kann ich ihn bis zum Ende noch kriegen?“ |
| Gegner | „Wann war P3 in der Box?“ · „Wie alt sind die Reifen vom Porsche?“ · „Warum ist der hinter mir so schnell?“ · „Wer ist gerade am schnellsten?“ |
| Boxenstopp | „Wo komm ich nach dem Stopp raus?“ · „Was, wenn ich zwei Runden später reinkomme?“ · „Lohnt sich ein Undercut?“ · „Soll ich jetzt reinkommen?“ |
| Reifen & Auto | „Wie sind die Reifen?“ · „Sind die Drücke okay?“ · „Wie schlimm ist der Schaden?“ · „Halten die Bremsen bis zum Ende?“ |
| Fahren | „Was hab ich in der letzten Runde falsch gemacht?“ · „Wo verlier ich am meisten?“ · „Wie fahr ich Kurve 7 besser?“ · „War die Runde gut?“ |
| Wetter & Strecke | „Kommt Regen?“ · „Sollen wir Regenreifen nehmen?“ · „Wird die Strecke schneller?“ · „Wie warm ist der Asphalt?“ |
| Session | „Wie lange noch?“ · „Wie viele Track Limits hab ich?“ · „Hab ich eine Strafe?“ · „Was ist meine beste Runde?“ |
| Folgefragen | „Und der dahinter?“ · „Und wenn ich eine Runde später komme?“ · „Wie viel war das nochmal?“ |
| Allgemein (wenn erlaubt) | „Wie funktioniert die FCY-Regel?“ · „Wie fährt man in Spa Eau Rouge?“ |

Fehlen Daten, sagt der Ingenieur das ehrlich; ist die Frage unklar, fragt er kurz nach.

**Einstellbar:** schneller Weg an/aus + Sicherheits-Schwelle, „Immer KI benutzen“, Antwortlänge, Anzahl gemerkter Funksprüche, „Moment …“-Füller, Themenbereich (nur Rennen & Auto / auch allgemeine Fragen), eigene Vokabeln für die Spracherkennung (z. B. Spitznamen von Gegnern), Groq-Modell.

## Sprachbefehle (Fahrer steuert die Crew)

Auch Befehle funktionieren **frei formuliert** („Halt mal kurz die Klappe“, „Sei zwei Runden still“, „Gib mir ab jetzt jede Runde den Abstand“) – die KI setzt sie über Aktions-Werkzeuge um. Die Tabelle zeigt nur Beispiele.

| Befehl | Wirkung |
|---|---|
| „Ruhe für 2 Runden“ / „Ruhe bis Kurve 1“ | Alle Meldungen außer Spotter und Kritisch pausiert |
| „Nur noch Spotter“ / „Normal weiter“ | Preset wechseln / zurück |
| „Sag mir jede Runde den Abstand nach hinten“ | Wiederkehrende Meldung anlegen |
| „Erinner mich in 5 Runden an die Box“ | Erinnerung |
| „Plan: Box in Runde 18“ | Strategie-Plan setzen, alle Berechnungen nutzen ihn |
| „Wiederhol das“ / „Was war die letzte Meldung?“ | Letzte Meldung erneut abspielen |
| „Lauter“ / „Leiser“ | Lautstärke der Crew |

Einstellbar: Befehle einzeln an/aus, Bestätigungston oder gesprochene Bestätigung („Verstanden“). Befehle auch auf Lenkrad-Schnelltasten legbar.

## Persönlichkeit

| Einstellung | Optionen |
|---|---|
| Stil | Ruhig & professionell · Motivierend · Trocken-britisch · Eigener Stil (Freitext für den LLM-Systemprompt) |
| Name des Ingenieurs / Spotters | frei wählbar, der Ingenieur nennt sich ggf. selbst (z. B. im Funkcheck) |
| Name des Fahrers | Name/Spitzname + optionale Aussprache-Schreibweise für die Sprachausgabe, Du/Sie (DE), wie oft der Name fällt (nie / selten / oft) |
| Gegner-Bezeichnung | Position („P3“) · Startnummer („die 88“) · Fahrzeug („der Porsche“) · Fahrername („Müller“) · Kombination (z. B. „P3, der Porsche“); Reihenfolge wählbar, Fallback falls Daten fehlen; gilt für Meldungen **und** KI-Antworten; KI versteht immer alle Varianten |
| Lob | aus / selten / oft |
| Verhalten nach Fehlern | beruhigen · sachlich · gar nichts sagen |
| Zahlen-Genauigkeit | gerundet („knapp 1,5“) · exakt („1,46“) |
| Einheiten | Liter/Gallonen, °C/°F, psi/bar, km/h/mph |

Der Stil wirkt auf die Auswahl der Template-Varianten **und** auf den LLM-Systemprompt.

## Stimmen & Funk-Effekt

Getrennt für **Renningenieur** und **Spotter**:

| Einstellung | Optionen |
|---|---|
| Stimme | Auswahl aus heruntergeladenen Piper-Stimmen (pro Sprache), Sprecher bei Mehrsprecher-Modellen, Vorhören, Stimmen nachladen/löschen |
| Lautstärke / Tempo | 0–150 % / 0,7–1,5× |
| Funk-Effekt | an/aus + **Stärke 0–100 %** (Bandpass, Verzerrung, Rauschen skalieren mit), Rauschpegel getrennt, Klick/Piep am Anfang und Ende an/aus |
| Ausgabegerät | Standard oder eigenes Gerät |