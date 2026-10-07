import type { Answer, Result, Sources } from './app.contract.d.ts';

export const appId = 'com.example.anchor-probe';
export const grants = '';

const sources: Sources = {
  rows: ([n]): Result<'rows'> => Array.from({ length: n }, (_, i) => ({ id: String(i) })),
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
