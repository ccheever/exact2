import type { Answer, Sources } from './app.contract.d.ts';
import { probeUpload } from '@fixture/transfer-probe';

// The suite writes the id `exact new` gave the app, and its server's origin.
export const appId = '__APP_ID__';
const origin = '__ORIGIN__';
export const grants = `net.fetch ${origin}`;

const sources: Sources = {
  // The query is a stand-in for a credential, which the record never keeps.
  startUpload: async ([id]) => {
    const r = await probeUpload({ id: String(id), url: `${origin}/upload?id=${id}&token=secret`, text: `hello ${id}` });
    return { text: r.error || 'started' };
  },
  note: async ([id, status, body, lost]) => {
    const q = new URLSearchParams({ id: String(id), status: String(status), body: String(body), lost: String(lost) });
    await fetch(`${origin}/heard?${q}`).catch(() => null);
    return { text: `${id} ${status} ${body} ${lost}` };
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
