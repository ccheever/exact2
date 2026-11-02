// Variable Type asks for nothing: the poster is the view alone.
import type { Answer, Sources } from './app.contract.d.ts';

export const appId = 'com.exact.variabletype';
export const grants = '';

const sources: Sources = {};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
