import { availableParallelism } from 'node:os';

export function conformArgs(argv) {
  const defaults = {
    '--wasm-root': '/tmp/e3-wasm', '--out': '/tmp/exact-web-js-conform',
    '--steps': 10, '--browser': null, '--only': null, '--label': 'urls',
    '--jobs': Math.min(4, availableParallelism()), '--js-from': null,
  };
  const options = Object.fromEntries(Object.entries(defaults).map(([flag, fallback]) => {
    const i = argv.indexOf(flag);
    return [flag, i < 0 ? fallback : argv[i + 1]];
  }));
  const named = argv.includes('--urls') ? [] : argv.filter((a, i) => !a.startsWith('--') && !Object.hasOwn(options, argv[i - 1]));
  return { options, named };
}
