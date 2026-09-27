// The recorder's source (LLP 1067.000): every answer asks the app's one
// native object, the page module on the web and the Swift module on Apple.
// Long calls go through `native.later`, which is the only call the web can
// answer; `native.watch` re-asks an answer when the recorder announces.
import type { Answer, NativeModule } from './app.contract.d.ts';

export const appId = 'com.exact.recorder';
export const grants = '';

type Native = NativeModule | null | undefined;
const unavailable = { available: false, recording: false, message: 'Recording is not available here.' };

// `available` is the device's fact: asking it keeps the bake from compiling
// an answer that depends on the recorder (LLP 1067 D4).
function recorder(native: Native): NativeModule | null {
  return native && native.available ? native : null;
}

const sources: Record<string, (native: Native) => unknown> = {
  async recorderStatus(native) {
    const r = recorder(native);
    if (!r) return unavailable;
    r.watch('status');
    return await r.later({ op: 'status' });
  },
  async takes(native) {
    const r = recorder(native);
    if (!r) return { takes: [] };
    r.watch('takes');
    return await r.later({ op: 'takes' });
  },
  async startRecording(native) {
    const r = recorder(native);
    if (!r) throw { kind: 'Unavailable', message: unavailable.message };
    return await r.later({ op: 'start' });
  },
  async stopRecording(native) {
    const r = recorder(native);
    if (!r) throw { kind: 'Unavailable', message: unavailable.message };
    return await r.later({ op: 'stop' });
  },
};

export const answer: Answer = ((source: string, _args: unknown[], _store: unknown, _storage: unknown, native: Native) => {
  const run = sources[source];
  if (!run) throw { kind: 'UnknownSource', message: source };
  return run(native);
}) as unknown as Answer;
