#!/usr/bin/env bun
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { proof } from '../../proof.mjs';
import { audioProof } from '../../bench/probes/audio.mjs';

const SOUNDS = ['blip.sound', 'chord.sound', 'whoosh.sound', 'drone.sound'];

if (import.meta.main) await proof(import.meta, async ({pin, pinSave, open, check, equal, out, host, say}) => {
  const node = (tree, id) => tree?.nodes?.find(n => n.props?.testId === id);
  const world = async s => {
    const state = await s.state();
    if (!state.world) throw new Error(`no world surface; logs: ${JSON.stringify(await s.logs()).slice(-600)}`);
    return state.world[0];
  };
  const start = async saved => { const s = await open(saved ? {world:saved} : {}); await s.tap('play'); return s; };

  let s = await open();
  check('title offers Play', node(await s.tree(), 'play')?.accessibleName === 'Play');
  await s.tap('play');
  const first = await world(s);
  check('every baked sound is declared and delivered before tick 0', first.loading.length === 0
    && SOUNDS.every(name => first.assets.some(a => a.name === name && a.state === 'Loaded')), first.assets);
  check('setup starts the looping drone voice, faded in from half a second in',
    first.audio.voices.some(v => v.sound === 'drone' && v.ends === null && v.offset === 0.5 && v.fade === 0), first.audio);
  check('the speaker carries the looping drone source', first.audio.sources.some(src =>
    src.sound === 'drone' && src.entity === 'speaker' && src.playing), first.audio);
  pin(0, first);

  const w = s.world('world');
  await w.run(1500);
  const ninety = await world(s);
  check('1500 ms is 90 ticks', ninety.tick === 90, ninety.tick);
  pin(90, ninety);
  const lines = (await s.logs())?.world?.flatMap(entry => entry.lines) ?? [];
  for (const line of ['sfx drone at ui gain 1.00 offset 0.50 fade-in 1.00 loop', 'loop drone on gain 0.50',
    'sfx blip at ui gain 1.00 pan -0.60', 'sfx blip at ui gain 1.00 pan 0.60',
    'sfx whoosh at speaker gain 1.00', 'sfx tick at ui gain 1.00']) {
    check(`journal carries "${line}"`, lines.some(l => l.endsWith(line)), lines.slice(-12));
  }
  check('the drone has faded in', ninety.audio.voices.some(v => v.sound === 'drone' && !('fade' in v)), ninety.audio);
  const hud = node(await s.tree(), 'hud')?.props?.text;
  check('the HUD counts voices and both loops from the world', /^Voices \d+ · loops 2$/.test(hud ?? ''), hud);

  await w.tap('Space');
  await w.run(50);
  const chord = (await world(s)).audio;
  check('Space plays the 24-bit stereo chord 20 ms in', chord.voices.some(v => v.sound === 'chord' && v.offset === 0.02), chord);
  await w.tap('KeyF');
  await w.run(100);
  const fading = (await world(s)).audio;
  check('F fades the looping drone and gives it an end', fading.voices.some(v => v.sound === 'drone'
    && v.fade > 0 && v.fade < 1 && Number.isInteger(v.ends)), fading);
  const midFile = resolve(out, 'mid-sound.world');
  await w.save(midFile);
  const mid = await w.snapshot();
  await w.run(1000);
  const end = await w.snapshot();
  const ended = (await world(s)).audio;
  check('the faded voice ended while the source loops on', !ended.voices.some(v => v.sound === 'drone')
    && ended.sources.some(src => src.sound === 'drone' && src.playing), ended);
  const fadeLines = (await s.logs())?.world?.flatMap(entry => entry.lines) ?? [];
  check('journal carries the fade', fadeLines.some(l => l.endsWith('sfx drone fade 0.50')), fadeLines.slice(-12));
  check('no host errors', !(await s.logs())?.host?.some(line => /^(exception:|console\.error:|error:)/.test(line)));
  const endFile = resolve(out, 'end.world');
  await w.save(endFile);
  pinSave('continuation', endFile);
  await s.close(); s = null;

  // A fresh process restores mid-sound (fade and all) and reaches the same bytes.
  const restored = await start(midFile), rw = restored.world('world');
  check('fresh process restores the mid-sound snapshot', equal(await rw.snapshot(), mid));
  await rw.run(1000);
  check('continuation matches the uninterrupted run', equal(await rw.snapshot(), end));
  const restoredFile = resolve(out, 'end-restored.world');
  await rw.save(restoredFile);
  check('continued save bytes match', readFileSync(endFile).equals(readFileSync(restoredFile)));
  await restored.close();

  if (host === 'web' && process.env.EXACT_AUDIO_PROBE === '1') {
    const {context} = await audioProof({game: 'audio-fixture', loop: 'drone', out, check, say});
    check('the Vorbis drone plays as a 22,050 Hz stereo AudioBuffer', context.sources.some(src =>
      src.looping && src.channels === 2 && src.sampleRate === 22050), context.sources);
    check('the 16-bit blip plays at its own 22,050 Hz, mono', context.sources.some(src =>
      !src.looping && src.channels === 1 && src.sampleRate === 22050), context.sources);
  } else if (host === 'web') say('SKIP live WebAudio probe: set EXACT_AUDIO_PROBE=1');
});
