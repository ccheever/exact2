import type { Answer, Sources } from './app.contract.d.ts';

export const appId = 'com.example.starter';
export const grants = '';

const sources: Sources = {
  dailyGoal: () => ({ target: 8, unit: 'glasses' }),
};
export const answer: Answer = (source, args, store, storage, native) =>
  sources[source](args, store, storage, native);
