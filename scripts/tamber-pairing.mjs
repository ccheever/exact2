// Scopes on the design server's wire (LLP 1121 D13 "Authentication",
// LLP 1122 D6 and D12, stage 10 A; tamber/1.13).
//
// Every request names a method, and every method has one scope here: the
// dispatcher (scripts/tamber-server.mjs) checks it before any handler runs,
// and a method with no entry is refused (fail closed). A new method, or a
// gateway's (stage 10 C's `canvas`), adds its entry here.
//
// - `read`: what reads the design, its history and the running builds'
//   reports, and the change stream.
// - `propose`: what only offers a change for someone else to accept (a held
//   proposal: `propose` with `hold`, `import`, `revert`; `drop` of one's own).
// - `write`: what writes the design, or changes what the developer's screens
//   show (`pin`, `preview`, `outline`, `hold`: the lead's ruling 5), or drives
//   the running builds (a scrub).
// - `local`: the local token only, never a paired client's, whatever its
//   scope (stage 10 A2's `clients`, `revoke`, `pair-code`).
//
// `inspect` is `read` (ruling 5): a client below `write` may read the
// selection each build last reported (`refresh`); turning inspect mode on or
// off, which takes the developer's builds' input, is `write`, as is asking
// the builds again (their answer outlines on the developer's screens).

/** The scopes a token may have, lowest first. */
export const SCOPES = ['read', 'propose', 'write'];

/** Each method's scope; the change stream's two paths by their names. */
export const METHOD_SCOPE = Object.freeze({
  hello: 'read',
  describe: 'read',
  resolve: 'read',
  explain: 'read',
  uses: 'read',
  history: 'read',
  export: 'read',
  inspect: 'read',
  subscribe: 'read',
  events: 'read',
  propose: 'propose',
  import: 'propose',
  revert: 'propose',
  drop: 'propose',
  transaction: 'write',
  undo: 'write',
  commit: 'write',
  accept: 'write',
  pin: 'write',
  preview: 'write',
  outline: 'write',
  hold: 'write',
  scrub: 'write',
  'scrub-release': 'write',
  'scrub-cancel': 'write',
});

/** Whether a client of `scope` may do what `needs`: `local` needs the local
 * token (`local` true), whatever its scope. */
export function allows(scope, needs, local = false) {
  if (needs === 'local') return local;
  const have = SCOPES.indexOf(scope), want = SCOPES.indexOf(needs);
  return have >= 0 && want >= 0 && have >= want;
}

/** What `request` needs: its method's entry (an own property only: no
 * inherited name is a method), `drop` of another client's held proposal
 * `write` (theme2's ruling (a)), and `inspect` other than `refresh` `write`
 * (ruling 5). `owner(id)` is a held proposal's client. Null for a method
 * with no entry. */
export function needs(request, owner = () => null) {
  if (!Object.hasOwn(METHOD_SCOPE, String(request.method))) return null;
  const base = METHOD_SCOPE[request.method];
  if (request.method === 'inspect' && request.refresh !== true) return 'write';
  if (request.method === 'drop') {
    const by = owner(request.proposal);
    return by !== null && by !== request.client ? 'write' : 'propose';
  }
  return base;
}

/** A refusal for a client of `scope` asking what `need`s. */
export function refusal(method, scope, need) {
  const why = need === 'local' ? 'the local tool\'s alone' : `\`${need}\` scope`;
  return { status: 403, error: { message: `${method} needs ${why}; this client has \`${scope}\``, scope, needs: need } };
}
