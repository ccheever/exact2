import type { Answer, Sources } from './app.contract.d.ts';

// The drive writes the id `exact new` gave the app. The fixture has no data sources: every alarm is set from an
// action, as only the person sets one (LLP 1133.030 D7).
export const appId = '__APP_ID__';
export const grants = 'device.alarms purpose.alarms';

const sources: Sources = {};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
