import type { Answer, Sources } from './app.contract.d.ts';

// The drive writes the id `exact new` gave the app. The fixture has no data sources: every call is placed from an
// action.
export const appId = '__APP_ID__';
export const grants = 'device.microphone purpose.microphone\ndevice.calls purpose.calls';

const sources: Sources = {};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
