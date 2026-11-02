import type { Answer } from './app.contract.d.ts';

// The suite writes the id `exact new` gave the app.
export const appId = '__APP_ID__';
export const grants = 'device.age-range';

export const answer: Answer = (source) => { throw new Error(`unknown source ${source}`); };
