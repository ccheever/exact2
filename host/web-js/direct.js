// A module the app's data code imports and its Contract never names (LLP
// 1133.006 D7, D8): no row in the plan, so modules.js has none of it; data
// code's calls come here (ts-data.js `__exact_module`), and go to the bound
// web implementation, loaded on first use (`ext-<name>.js`), past the
// runner's admission as the native runner gives it (runner/src/runner/
// modules/admitted.rs):
//
// - a name the interface declares `"origin": "person"` is refused
//   `NotAllowedError` (person-required) when the commit's origin is not a
//   person's (`Origin`, modules.js);
// - each `{"$path": "app:/…"}` is read from the page's store under the
//   source's `fs.read` (ts-body-from.js), and reaches the implementation as
//   `{"$file": Blob}`; `{"$bytes"}` stays base64;
// - under the agent, only an implementation that is its own substitute
//   (`"substitute": "implementation"`, D8 item 4) answers, told it is
//   substituting (`agent`); any other name is `NotSupportedError`, since no
//   declared substitute of it is in the plan;
// - an answer's `$state` is kept for `state.extensions`, and each call
//   counted, as the runner's `extensions_json` writes them.
import { clock, say, Origin } from './rt.js';

// `table`: extension → [a loader of its implementation, its admission {person, substitute}].
export function direct(table) {
  const loaded = new Map(), calls = {}, reported = {};
  const implementation = extension => {
    if (!loaded.has(extension)) loaded.set(extension, table[extension][0]().then(m => m.implementation?.({
      agent: clock.agent, say, changed: () => {},
    }) ?? null));
    return loaded.get(extension);
  };
  const refused = (key, name, why) => { say(`${key}: refused: ${name}${why ? `, ${why}` : ''}`); return { error: name, message: `${key}: ${why || name}` }; };
  /** `args` with each admitted path's file in place of its path. */
  const files = async (key, value) => {
    if (Array.isArray(value)) return Promise.all(value.map(v => files(key, v)));
    if (!value || typeof value !== 'object') return value;
    const keys = Object.keys(value);
    if (keys.length === 1 && typeof value.$path === 'string') {
      const [{ readBodyFile }, { tsGrantSet }] = await Promise.all([import('./ts-body-from.js'), import('./admission-data.js')]);
      return { $file: await readBodyFile(value.$path, tsGrantSet) };
    }
    return Object.fromEntries(await Promise.all(keys.map(async k => [k, await files(key, value[k])])));
  };
  (globalThis.exact ??= {}).direct = {
    has: extension => Object.hasOwn(table, extension),
    async call(body) {
      const extension = body.extension, name = body.send ?? body.source, key = `${extension}.${name}`;
      const [, admission] = table[extension];
      ((calls[extension] ??= {})[name] ??= 0);
      calls[extension][name]++;
      if (clock.agent && !admission.substitute) return refused(key, 'NotSupportedError', 'no substitute of it under the agent');
      if (admission.person?.includes(name) && Origin.now() !== 'person') return refused(key, 'NotAllowedError', 'person-required');
      let args;
      try { args = await files(key, body.args ?? {}); }
      catch (e) { return refused(key, e?.code === 'denied' ? 'NotAllowedError' : 'DataError', e?.message ?? String(e)); }
      const impl = await implementation(extension);
      if (!impl?.[name]) return { error: 'NotSupportedError' };
      const answer = await impl[name](args);
      if (answer && typeof answer === 'object' && '$state' in answer) {
        reported[extension] = answer.$state;
        const { $state, ...rest } = answer;
        return rest;
      }
      return answer;
    },
    // What `state.extensions` shows of each, beside the plan's modules.
    state() {
      const out = {};
      for (const extension of Object.keys(table)) {
        if (!calls[extension] && !reported[extension]) continue;
        out[extension] = { bound: !clock.agent || !!table[extension][1].substitute, preferred: {}, readers: {}, held: [], calls: calls[extension] ?? {}, ...(reported[extension] ?? {}) };
      }
      return out;
    },
  };
}
