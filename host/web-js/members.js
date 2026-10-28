// Members on the JS target (LLP 1133.014 D10 item 1), as the runner's
// `modules/members.rs`: an option another package declares on a ceremony's
// name (`@exact/sign-in`'s `identity` on `@exact/passkeys`' `get`). A row
// whose substitute names `memberOf` is the member's; the owner's call
// carries its option. Admission refuses an option no bound package declares,
// and holds an identity member to FedCM's rules and the providers' grants;
// the owner's `credential` ticket names it; its answer, cancel and choices
// are the member's where the call asks nothing of the owner's own ceremony.

const PLATFORMS = ["apple", "android", "web", "windows", "linux"];
const FIELDS = ["name", "email", "picture"];

/** `rows`: every module row (`{key, sub}`); `valueOf(row)`, a source's value now; `grants()`, the app's lines. */
export function members(rows, valueOf, grants) {
  const extensionOf = row => row.key.slice(0, row.key.indexOf("."));
  const of = owner => rows().filter(r => r.sub?.memberOf === owner.key).map(r => [r.sub.member, r]);
  const inArgs = (owner, args) => of(owner).filter(([o]) => args?.[o] != null);
  const ownsCeremony = (owner, args) => !owner.sub.ceremony?.options || args?.[owner.sub.ceremony.options] != null;
  const status = (member) => {
    const name = member.sub.status, row = name && rows().find(r => r.key === `${extensionOf(member)}.${name}`);
    return (row && valueOf(row)) ?? {};
  };
  const lines = () => (grants() ?? "").split("\n").map(l => l.trim().split(/\s+/));
  const granted = (line, id) => lines().some(w => w.length === 2 && w[0] === line && w[1] === id);
  const fetches = url => lines().some(([l, t]) => {
    if (l !== "net.fetch" || !t) return false;
    t = t.replace(/\/+$/, "");
    return url === t || url.startsWith(`${t}/`);
  });
  const fail = (error, message) => ({ error, message });

  function identity(owner, member, option, args) {
    const request = args[option] ?? {}, mode = request.mode ?? "passive", mediation = args.mediation ?? "optional";
    const withKey = !!owner.sub.ceremony && ownsCeremony(owner, args);
    const providers = Array.isArray(request.providers) ? request.providers : [];
    if (!providers.length) return fail("TypeError", `${option}.providers names at least one provider`);
    if (mode !== "active" && mode !== "passive") return fail("TypeError", `${option}.mode is "active" or "passive", not ${JSON.stringify(mode)}`);
    if (mediation === "conditional") return fail("NotSupportedError", `mediation "conditional" beside ${option}: no target can keep it`);
    if (mode === "active" && providers.length !== 1) return fail("NotSupportedError", `an active ${option} request names one provider`);
    if (mode === "active" && withKey) return fail("NotSupportedError", `an active ${option} request is a press's own, never beside publicKey`);
    const can = status(member);
    if (can.available === false) return fail("NotSupportedError", `no ${option} provider is offered on web`);
    let kept = 0;
    for (const [i, p] of providers.entries()) {
      const known = member.sub.providers?.[p?.configURL];
      if (!known) return fail("NotSupportedError", `${option}.providers[${i}].configURL ${JSON.stringify(p?.configURL ?? null)} is no provider here`);
      if (!granted(known.line, p.clientId)) return fail("SecurityError", `grant-missing (${known.name}'s clientId=${p.clientId} needs \`${known.line} ${p.clientId}\`)`);
      const back = p.web?.redirectURI;
      if (typeof back !== "string") return fail("NotSupportedError", `${known.name} on the web needs web.redirectURI, the app server's return route`);
      if (!fetches(back)) return fail("SecurityError", `grant-missing (web.redirectURI=${back} is outside every \`net.fetch\` line)`);
      const caps = can.providers?.[known.name] ?? {};
      // Beside a passkey, a provider counts only where one sheet holds both (D1).
      if (caps[mode] === true && (!withKey || caps.withPublicKey === true)) kept++;
      else if (mode === "active") return fail("NotSupportedError", `${known.name} offers no active request on web`);
    }
    if (!kept && withKey) return fail("NotSupportedError", `no provider of this ${option} request shares one sheet with a passkey on web`);
    if (!kept) return fail("NetworkError", `no provider of this passive ${option} request is offered on web`);
    return null;
  }

  return {
    ownsCeremony,
    /** Why the owner's call is refused by its members' rules, or null. */
    refused(owner, args) {
      const takes = owner.sub.takes;
      if (!Array.isArray(takes)) return null;
      if (args?.mediation === "silent") return fail("NotSupportedError", 'mediation "silent": no target can promise no UI (LLP 1133.014 D1)');
      const bound = of(owner);
      for (const key of Object.keys(args ?? {})) {
        if (args[key] == null || takes.includes(key) || PLATFORMS.includes(key)) continue;
        const found = bound.find(([o]) => o === key);
        if (!found) return fail("NotSupportedError", `\`${key}\` is no option of ${owner.key}, and no package bound here declares it as a member`);
        if (found[1].sub.providers) {
          const why = identity(owner, found[1], key, args);
          if (why) return why;
        }
      }
      return null;
    },
    /** The members the call carries, as the owner's ticket names them. */
    args(owner, args) {
      const out = {};
      for (const [option, m] of inArgs(owner, args)) {
        const request = args[option];
        if (!m.sub.providers) { out[option] = request; continue; }
        const mode = request.mode ?? "passive", can = status(m), relay = (m.sub.relay ?? []).includes("web");
        const withKey = !!owner.sub.ceremony && ownsCeremony(owner, args);
        const offered = p => { const c = can.providers?.[m.sub.providers[p.configURL]?.name] ?? {}; return mode === "active" || (c.passive === true && (!withKey || c.withPublicKey === true)); };
        const providers = (request.providers ?? []).filter(offered).map(p => {
          const known = m.sub.providers[p.configURL] ?? {}, path = known.audience?.web;
          return { provider: known.name ?? null, configURL: p.configURL, clientId: p.clientId ?? null,
            audience: path === "app" ? null : path ? path.split(".").reduce((x, k) => x?.[k], p) ?? null : null,
            nonce: p.params?.nonce ?? null, fields: p.fields ?? FIELDS, loginHint: p.loginHint ?? null, domainHint: p.domainHint ?? null,
            token: relay ? "relay" : "native", redirectURI: relay ? p.web?.redirectURI ?? null : null };
        });
        out[option] = { mode, context: request.context ?? "signin", providers };
      }
      return out;
    },
    /** The member whose `answer` fields `v` has: [its row] or null. */
    answering(owner, v) {
      if (!v || typeof v !== "object") return null;
      return of(owner).map(([, m]) => m).find(m => m.sub.answer && Object.entries(m.sub.answer).every(([k, x]) => v[k] === x)) ?? null;
    },
    /** A typed answer of a member's type: null when no member's, else why it is refused ("" for none). */
    typed(owner, args, v) {
      const m = this.answering(owner, v);
      if (!m) return null;
      const named = (args?.[m.sub.member]?.providers ?? []).map(p => p.configURL);
      return named.includes(v.configURL) ? "" : `${owner.key}: the answer's configURL is ${JSON.stringify(v.configURL)}, and the request named ${JSON.stringify(named)}`;
    },
    /** A tap's answer where the call asks nothing of the owner's ceremony: the first member's cancel or choice. */
    tapped(owner, args, choice) {
      if (ownsCeremony(owner, args)) return null;
      const [, m] = inArgs(owner, args)[0] ?? [];
      return (m && (choice === "cancel" ? m.sub.cancel : m.sub.choices?.[choice])) ?? null;
    },
    choices(owner) { return [...new Set(of(owner).flatMap(([, m]) => Object.keys(m.sub.choices ?? {})))]; },
    /** For the list `state.extensions` shows: each member's mode and providers, never a client or nonce. */
    summary(owner, args) {
      const out = {};
      for (const [option, d] of Object.entries(this.args(owner, args))) if (d?.providers) out[option] = { mode: d.mode, providers: d.providers.map(p => p.provider) };
      return out;
    },
  };
}
