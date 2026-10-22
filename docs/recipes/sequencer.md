# Recipe: a step sequencer

Lanes are `Track`s in a `derive`; one `sequence` plays them on the runner's clock, with
no action per step. Each loops at its own length (polymeter). MIDI and the rest: ["Sound, MIDI and media"](../contract-for-agents.md#sound-midi-and-media).

```contract
sound "assets/kits/808/kick.wav"
sound "assets/kits/808/hat.wav"
sound "assets/kits/909/kick.wav"
sound "assets/kits/909/hat.wav"

shape Step
  beat: number
  src: string
  gain: number
  group: string
  note: number
  velocity: number
  channel: number
  duration: number
  haptic: string

shape Track
  beats: number
  loops: number
  offset: number
  output: string
  steps: Step[]

shape Transport
  bpm: number
  anchorAt: number
  anchorBeat: number

// `n` steps of `every` beats, sounding at `hits`.
fn lane(kit: string, slot: string, n: number, every: number, hits: number[]): Track = Track(beats=n * every, loops=0, offset=0, output="", steps=hits.map(i => Step(beat=i * every, src=`assets/kits/${kit}/${slot}.wav`, gain=1, group=slot, note=-1, velocity=100, channel=9, duration=every, haptic="")))

component Drums
  state playing = false
  state at = 0
  state kit = "808"
  resource transport = exactTransport() as shape Transport
  derive tracks = [lane(kit, "kick", 16, 0.25, [0, 4, 8, 12]), lane(kit, "hat", 12, 0.25, [0, 3, 6, 9])]
  sequence groove when playing
    play(tracks, bpm=120, from=at)
  action start
    playing = true
  action pause
    at = transport.anchorBeat + (performanceNow() - transport.anchorAt) * transport.bpm / 60000
    playing = false
  action setKit(name: string)
    kit = name
  view
    row gap=8
      if playing
        button "Pause" press=pause testId="pause"
      else
        button "Play" press=start testId="play"
      button "909" press=setKit("909") aria-pressed=(kit == "909") testId="kit-909"
```

```contract-test
test "lanes loop at their own lengths; an edit plays at once"
  tap "play"
  clock +2000
  expect sound has "assets/kits/808/hat.wav" at 1500
  tap "kit-909"
  clock +1000
  expect sound has "assets/kits/909/kick.wav" at 2500
```

- **Declare every file a built path can name**, a `sound` line each: a step whose `src`
  none declares is silent (`logs`: `… is not a declared sound`).
- **The shapes are positional:** keep their fields in this order.
- **Resolution is beats a step:** `0.25` is a sixteenth, `1/3` a triplet eighth.
- **Edits are live:** a changed step, lane, kit or `bpm` plays from its commit, in phase.
  `pause` keeps the beat in `at` for the next start. A new `key=` starts again at `from`:
  one sequence plays at a time, so a mode is a key, not a second `sequence`.
- **A playhead** is an animation: the guide's "Following the transport".
- **MIDI:** a track's `output` is an output's `id`, its `offset` a delay in ms;
  `clock=` an input's `id` follows that device (`waiting` until Start).
- **iOS stops it when hidden** unless the app plays in the background (LLP 1125 D4); a
  task on `transport.reason == "background"` (add `reason: string` to `Transport`) sets
  `playing` false.
