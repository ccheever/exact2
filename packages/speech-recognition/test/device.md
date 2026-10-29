# Speech recognition on a device, by a person (LLP 1133.012 §10, drive 7)

The drives that need a real microphone, done with the recorder
(`apps/recorder`) built as in production: no agent, the packages bound.
The iPhone 16 Pro results are in LLP 1133.020 §8. This Mac's microphone
does not answer.

- iPhone (iOS 26): `bun host/apple/build.mjs --ios --device recorder-apple --run`
- Mac: `bun host/apple/build.mjs recorder-apple --run`, on a Mac whose
  microphone answers
- Android: `bun scripts/agent-android.mjs build recorder --apk`, then
  `adb install -r host/android/app/build/recorder/outputs/apk/release/app-release.apk`
- Chrome: `bun host/web/build.mjs recorder`, then serve `host/web/dist`
  (`bun host/web/dev.mjs --app recorder`) and open it in Google Chrome

Each step says what to do and what should happen.

1. **A live transcript.** Tap Record. Allow the microphone. The legacy
   `SFSpeechRecognizer` also asks for speech recognition; the
   `SpeechAnalyzer` engines on iOS and macOS 26 or later need no separate
   authorization. Speak for
   thirty seconds. The line under the bars shows what you say, the words
   still being heard following the finished ones. Tap Stop. The take's row
   shows the transcript's first line, and it reads as you spoke. Only one
   recording indicator showed throughout.
2. **Speech recognition denied** (`SFSpeechRecognizer` only). In Settings,
   turn speech recognition off for Recorder and record a take. It records
   with no live line, and the line saying transcripts are off shows with
   Open Settings. This step does not apply to a `SpeechAnalyzer` engine;
   Recorder has no separate speech-recognition switch for that engine.
3. **Transcribe.** Record a take, and leave the app while it records. Back
   in the app, the take's row offers Transcribe, and its transcript is
   partial. Tap Transcribe. The transcript is replaced by one of the whole
   take.
4. **A long take** (iOS 17, simulator or device, where the recognizer is
   `SFSpeechRecognizer`). Record for more than a minute with on-device
   recognition. The transcript runs on past the first minute, and the
   journal (`state.extensions`, or the log) says whether a seam came at
   the minute (D9).
5. **Android's service.** Open the app, and read
   `speechRecognitionStatus` (the log, or `state` in a development build):
   it names the inputs the phone's service takes (`track` only for a
   service the app can feed, `microphone` otherwise). Record a take: a
   live line shows only where `track` is listed.
6. **Chrome's microphone and track** (a Mac whose microphone answers):
   `bun packages/speech-recognition/test/web.mjs chrome` hears the spoken
   file on Chrome's fake device and checks it. In the recorder in Chrome,
   step 1 holds, with one prompt (the microphone).
7. **Safari.** The recorder in Safari records with no live line (Safari's
   recognizer only takes its own microphone, which the take holds).
