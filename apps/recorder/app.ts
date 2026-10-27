// The recorder's data (LLP 1133.009 §11, LLP 1133.012 §10): the takes are the
// files @exact/recording wrote to app:/data/takes; each one's length and
// level, which the Contract passes when it files a take, are kept beside them
// in index.json. A take's transcript is the directory
// @exact/speech-recognition wrote beside it (`<take>.transcript`, JSON Lines
// files of final results), and transcripts.json keeps whether each one is
// complete: its recognition ended by the app's stop, not aborted or failed.
import type { Answer, Storage } from './app.contract.d.ts';

export const appId = 'com.exact.recorder';
const DIR = 'app:/data/takes';
const INDEX = `${DIR}/index.json`;
const TRANSCRIPTS = `${DIR}/transcripts.json`;
export const grants = `device.microphone purpose.microphone\ndevice.microphone-background purpose.recordingInBackground\ndevice.speech-recognition purpose.transcribe\nfs.read ${DIR}\nfs.write ${DIR}`;

type Kept = { name: string; seconds: number; level: number };
type Info = { id: string; transcript: string | null; finalCount: number; files: number; error: string | null };

async function json<T>(storage: Storage, path: string, none: T): Promise<T> {
  try {
    return JSON.parse(new TextDecoder().decode(await storage.fs.readFile(path))) as T;
  } catch {
    return none;
  }
}

const nameOf = (path: string) => path.slice(path.lastIndexOf('/') + 1).replace(/\.[^.]*$/, '');
const label = (k: Kept) => `${k.name.replace('take-', 'Take ')} · ${k.seconds.toFixed(1)} s · level ${k.level}%`;
const isTake = (f: string) => /^take-\d+\.[a-z0-9]+$/.test(f) && !f.endsWith('.transcript') && !f.endsWith('.json');

/** The first final result of a take's transcript, or `""`: its first file's first line. */
async function firstLine(storage: Storage, name: string): Promise<string> {
  try {
    const text = new TextDecoder().decode(await storage.fs.readFile(`${DIR}/${name}.transcript/00000.jsonl`));
    const line = text.slice(0, text.indexOf('\n') < 0 ? undefined : text.indexOf('\n'));
    return line ? (JSON.parse(line).alternatives?.[0]?.transcript ?? '') : '';
  } catch {
    return '';
  }
}

// Typed here rather than by `Sources`: the smoke builds this file against fixture plans that read neither source.
const sources: Record<string, (args: any, store: unknown, storage: Storage) => Promise<unknown>> = {
  takes: async (_args, _store, storage) => {
    let files: string[] = [];
    try {
      files = await storage.fs.readdir(DIR);
    } catch {
      // No takes yet: the directory is made by the first.
    }
    const kept = await json<Kept[]>(storage, INDEX, []);
    const complete = await json<Record<string, boolean>>(storage, TRANSCRIPTS, {});
    const takes = await Promise.all(files.filter(isTake).map(async (f) => {
      const name = nameOf(f), k = kept.find((t) => t.name === name);
      return {
        name, label: k ? label(k) : name.replace('take-', 'Take '), path: `${DIR}/${f}`,
        transcript: await firstLine(storage, name), complete: complete[name] === true,
      };
    }));
    takes.sort((a, b) => Number(a.name.slice(5)) - Number(b.name.slice(5)));
    return { takes, next: takes.reduce((n, t) => Math.max(n, Number(t.name.slice(5)) || 0), 0) + 1 };
  },
  // The take's level is its RMS over the time metered (WebRTC statistics' `totalAudioEnergy` over `totalSamplesDuration`).
  keepTake: async ([path, duration, energy, seconds], _store, storage) => {
    const level = seconds > 0 ? Math.sqrt(energy / seconds) : 0;
    const take: Kept = { name: nameOf(path), seconds: Math.round(duration / 100) / 10, level: Math.round(level * 100) };
    const kept = (await json<Kept[]>(storage, INDEX, [])).filter((t) => t.name !== take.name);
    await storage.fs.mkdir(DIR);
    await storage.fs.writeFile(INDEX, new TextEncoder().encode(JSON.stringify([...kept, take])));
    return { name: take.name, message: `Saved ${label(take)}` };
  },
  // A recognition that ended: its take's transcript is complete when it ended with no error.
  keepTranscript: async ([info]: [Info], _store, storage) => {
    if (!info.transcript) return { name: '', message: '' };
    const name = nameOf(info.transcript);
    const complete = await json<Record<string, boolean>>(storage, TRANSCRIPTS, {});
    complete[name] = info.error == null;
    await storage.fs.mkdir(DIR);
    await storage.fs.writeFile(TRANSCRIPTS, new TextEncoder().encode(JSON.stringify(complete)));
    const lines = `${info.finalCount} ${info.finalCount === 1 ? 'line' : 'lines'}`;
    return { name, message: info.error == null ? `Transcribed ${name.replace('take-', 'Take ')} · ${lines}` : `${name.replace('take-', 'Take ')}'s transcript stopped early (${info.error})` };
  },
};

export const answer = ((source: string, args: unknown, store: unknown, storage: Storage) => sources[source](args, store, storage)) as unknown as Answer;
