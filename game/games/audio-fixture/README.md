# audio-fixture

Sampled sounds beside a synthesized one, with no model support (`game.audio: true`
only). [`art/`](art) holds four generated files the bake turns into `.sound` assets:

| file | encoding | content |
|---|---|---|
| `blip.wav` | 16-bit PCM, mono, 22,050 Hz, 0.15 s | 880 Hz sine, decaying |
| `chord.wav` | 24-bit PCM, stereo, 32,000 Hz, 0.12 s | 523.25 Hz left, 659.25 Hz right |
| `whoosh.wav` | 32-bit float, mono, 48,000 Hz, 0.1 s | seeded noise under a half-sine |
| `drone.ogg` | Vorbis (libvorbis), stereo, 22,050 Hz, 1 s | 110 Hz left, 165 Hz right, whole periods |

[The game](logic/src/lib.rs) loops the drone as a faded-in voice and as a source on
a speaker to the listener's right, plays panned and pitched blips, a float whoosh at
the speaker and a band-limited synth tick; Space plays the chord part-way in, F fades
the voice out. Tick and save hashes live in [pins.json](pins.json).

```sh
bun game/prove.mjs audio-fixture --hosts linux,web
EXACT_AUDIO_PROBE=1 bun game/games/audio-fixture/proof.mjs web
```

The second runs the live WebAudio probe: a trusted gesture, a running context, the
Vorbis drone as a 22,050 Hz stereo buffer, and non-zero analyser RMS.
