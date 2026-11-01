# Audio, Stimmen, Spracherkennung – Entscheidungen & Quellen

## sherpa-onnx
- Crate `sherpa-onnx` **=1.13.8** (offiziell, k2-fsa). Lädt beim Build fertige statische Windows-Bibliotheken (inkl. onnxruntime) – kein CMake nötig, ein einziges EXE.
- **Nicht 2.x:** Ab 2.0.0 ist die eingebaute espeak-ng-Phonemisierung entfernt (`vits data_dir` wird ignoriert, Log: „--vits-data-dir is deprecated and ignored in sherpa-onnx >= v2.0.0“). Piper-Stimmen bräuchten dann einen externen Phonemizer. Lokal verifiziert am 2026-11-01.
- Piper-Stimmen: **fp32**-Archive. Die `-fp16`-Archive laden nicht („Type (tensor(float16)) … does not match expected type (tensor(float))“).
- Messung (Release, Thorsten medium, 2 Threads): 2,7 s Sprache in 165 ms gerendert.

## Modelle (Download beim ersten Start, SHA-256-geprüft)
Katalog: `src-tauri/resources/models.json`. Quelle: GitHub-Releases `k2-fsa/sherpa-onnx` (Tags `tts-models`, `asr-models`, `kws-models`).

| Modell | Lizenz |
|---|---|
| de_DE-thorsten-medium, thorsten_emotional, kerstin-low | CC0 |
| en_GB-northern_english_male-medium | CC-BY-SA 4.0 (OpenSLR 83) – Namensnennung in „Über“ |
| en_GB-cori-medium, en_US-john-medium | Public Domain (LibriVox) |
| en_US-joe-medium | CC0 |
| silero_vad.onnx | MIT |
| sherpa-onnx-kws-zipformer-gigaspeech-3.3M-2024-01-01 | Apache-2.0 |

Bewusst **nicht** aufgenommen: `en_US-ryan` (CC BY-NC-SA, nicht kommerziell), `en_GB-alan` (Lizenz unklar).

## Codewort
- Englisches Zipformer-KWS-Modell (GigaSpeech). Keywords müssen als BPE-Tokens übergeben werden.
- Vorschläge („RADIO“, „HEY CREW“, „BOX“, …) sind mit dem SentencePiece-Modell des Pakets vortokenisiert: `resources/wakewords.json`.
- Eigene Codewörter: Greedy-Longest-Match über `tokens.txt`. Für RADIO / HEY CREW / BOX identisch mit SentencePiece, für manche Wörter abweichend (z. B. ENGINEER) – funktioniert, kann aber schlechter erkannt werden.
