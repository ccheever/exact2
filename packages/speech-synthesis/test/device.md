# Speech synthesis on a phone, by a person (LLP 1133.012 §10)

The drives a phone needs, done with the Markdown reader (`apps/markdown`)
built as in production: no agent, the package bound. Open a document with
a few paragraphs, such as `apps/markdown/assets/listen-fixture.md`.

- iPhone: `bun host/apple/build.mjs --ios --device markdown-apple --run`

The reader has no Android build. On an Android phone, steps 3 and 4 are
for whichever app first speaks there.

Each step says what to do and what should happen.

1. **Listen.** Tap Listen. The document is read aloud from the top, one
   paragraph after another, and the word being spoken is marked. Pause
   holds the mark and the voice, and Resume goes on from there. When the
   last paragraph ends, "Finished" shows.
2. **The screen locked** (iPhone). Tap Listen and lock the phone. Reading
   goes on. Unlock: the mark is at the word being spoken. Lock-screen
   controls are not required: Now Playing for a speech `playback` queue
   remains deferred (`rules/DEFERRED.md`, Speech).
3. **A call.** While reading, call the phone from another one. Reading
   pauses while the call rings. End the call: reading resumes, or Resume
   resumes it.
4. **Another app's audio.** Play a podcast, then open the reader and tap
   Listen. The podcast stops while the reader speaks. Tap Stop: the
   podcast resumes (iOS's `.notifyOthersOnDeactivation`; Android's audio
   focus given back).
5. **A silent switch** (iPhone). With the ring switch on silent, Listen
   still speaks (`playback`, not `ambient`).
6. **Leaving.** Tap Listen, then open another document. Reading stops.
