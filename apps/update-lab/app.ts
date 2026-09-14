import type { Answer } from './app.contract.d.ts';

export const appId = 'com.exact.updatelab';
export const grants = '';

// Edit these two values for the TypeScript update experiment.
const VERSION = 'TS v9';
const MULTIPLIER = 4;

export const answer: Answer = (source, args) => {
  if (source === 'typescriptProbe') {
    const input = args[0];
    if (typeof input !== 'number') throw new Error('typescriptProbe expects one number');
    return { label: VERSION, input, output: input * MULTIPLIER };
  }
  // The app's Rust DataSource owns rustProbe and rustExecutor on every platform.
  // Neither probe runs at bake: Contract invokes them after first pixel.
  throw new Error(`Source is not implemented in TypeScript: ${source}`);
};
