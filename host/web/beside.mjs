// The names a web build at `<dist>` uses beside it, `<dist>.<name>` (serve.mjs `replaceBuild`, generations.mjs):
// the builds it replaced, kept by generation (`gen`), those reaped (`gen.reaped`), drives' leases on them (`leases`),
// a swap's lock (`lock`), the tree a swap puts aside (`previous`), and the directory each build is made in
// (`stage-…`). None is a source: each scan of an app skips them beside its `dist` (host/web-js/build.mjs and dev.mjs,
// scripts/agent-launch.mjs, scripts/deploy.mjs), and js/bake/src/beside.rs holds the same names for the native bake.
export const BESIDE = ['gen', 'gen.reaped', 'leases', 'lock', 'previous'];
export const STAGE = 'stage-';

/** Whether `name` is one a build named `base` uses beside it. */
export function beside(name, base = 'dist') {
  if (!name.startsWith(`${base}.`)) return false;
  const rest = name.slice(base.length + 1);
  return BESIDE.includes(rest) || (rest.startsWith(STAGE) && rest.length > STAGE.length);
}

/** The same, as a regular expression's source for a path segment (`dist.gen`, `dist.stage-a1b2c3`). */
export const besidePattern = (base = 'dist') => `${base.replaceAll('.', '\\.')}\\.(?:${BESIDE.map((n) => n.replaceAll('.', '\\.')).join('|')}|${STAGE}[^/]+)`;
