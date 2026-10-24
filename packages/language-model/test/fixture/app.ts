import type { Answer, Sources, Store, Storage } from './app.contract.d.ts';
import { createLanguageModel, prompt, destroyLanguageModel } from '@exact/language-model';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = '';

const NOTES = [
  { title: 'Borrowing', excerpt: 'A borrow may not outlive what it borrows; lifetimes say how long.' },
  { title: 'Groceries', excerpt: 'Eggs, rice, lemons.' },
];
const code = (e: unknown) => (e as { code?: string }).code ?? String(e);
const NOTHING = { type: 'object', properties: {}, required: [] };

// The session `prepare` made, and what its handlers met (`reenter`).
let own = '';
let met: string[] = [];

// The model's tools (LLP 1133.016 D9): a handler per name a session lists, given the model's arguments.
export const tools = {
  findNotes: async ({ query }: { query: string }, _store: Store, _storage: Storage) =>
    NOTES.filter((n) => `${n.title} ${n.excerpt}`.toLowerCase().includes(String(query).toLowerCase())),
  // Reentry (D9): a handler prompting the session whose response called it waits on itself, so it fails at once.
  promptOwn: async () => {
    try { await prompt({ session: own, input: 'Again.' }); met.push('promptOwn answered'); } catch (e) { met.push(`promptOwn ${code(e)}`); }
    return 'done';
  },
  // A model's call is never more than `tool` (D10): it may not start a download.
  startDownload: async () => {
    try { await createLanguageModel({}); met.push('startDownload created'); } catch (e) { met.push(`startDownload ${code(e)}`); }
    return 'done';
  },
};

const sources: Sources = {
  // Data code's own session: a structured prompt whose schema is an object, answered as JSON text.
  ask: async () => {
    try {
      const { id } = await createLanguageModel({});
      const { text } = await prompt({ session: id, input: 'A title, please.', responseConstraint: { type: 'object', properties: { title: { type: 'string' } }, required: ['title'] } });
      await destroyLanguageModel({ session: id });
      return { text };
    } catch (e) {
      return { text: `error ${code(e)}` };
    }
  },
  prepare: async () => {
    try {
      const made = await createLanguageModel({ tools: [
        { name: 'promptOwn', description: 'Prompts this session again', inputSchema: NOTHING },
        { name: 'startDownload', description: 'Creates a session', inputSchema: NOTHING },
      ] });
      own = made.id;
      return { text: own };
    } catch (e) {
      return { text: `error ${code(e)}` };
    }
  },
  // A prompt whose script calls both handlers; what they met, in order.
  reenter: async () => {
    met = [];
    try { await prompt({ session: own, input: 'Call your tools.' }); } catch (e) { met.push(`prompt ${code(e)}`); }
    return { text: met.join('; ') };
  },
};

export const answer: Answer = (source, args, store, storage) => sources[source](args, store, storage);
