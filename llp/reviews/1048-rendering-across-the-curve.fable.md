# Review: LLP 1048 r2 (and 1048.000–1048.003), 2026-09-23 (fable)

- **Family:** Anthropic — Claude Fable 5.1, as a Claude Code subagent (`model: fable`, general-purpose, read-only by instruction), run on this machine against `/Users/ccheever/projects/exact2` at `3d64f1f3` and `~/projects/interview`. The same brief as the Grok and Claude reviews (sha256 `a86eeefc626904862c7ede6707c982c3d5cdf6b93bf8a50be8f5a3b76c0310a1`); 25 tool calls, about 9 minutes.
- **Method:** a quick directional review at Charlie's request ("have fable do a quick review"), one round, blind to the Grok and Claude reviews; it read Astra's review and its disposition. Fable is Anthropic like the author (Claude Opus 5.5), so this is not an independent model family — but it is a different model and a fresh session without the authoring context. Nothing redacted; the brief forbade opening secrets.
- **Reviewed revision:** LLP 1048 r2 and 1048.000–1048.003 r2.
- **Disposition:** after the verbatim review (the author's, shared across the three directional reviews).

---

## Review (verbatim)

**Overall assessment**

The direction is right. One DOM projection in Rust, a settled checkpoint as the unit of rendering, stage-inferred holes, adoption instead of hydration, and an activation policy are the correct way for exact2 to cover the curve, and the r2 revisions (a real async render host, a defined document, placeholders instead of boot refusals, parity by parsing the served HTML) fixed the things that would have sunk r1. The order of work in 1048.000 §5 (Caltrain at build, then head/dist, then the render host, then the server, then Interview) is sound, and steps 1–2 are a shippable milestone on their own: a static site for every existing app on any static host. A headless-Chrome prerender would be cheaper for that milestone but can't do per-request pages and would be throwaway, so the Rust projection is the right investment.

What I would change is above and around the renderer, not in it: the server has no production home, the privacy machinery is the wrong shape, the server is about to become a fifth host, and phase 1 carries Contract surface its first consumer does not need.

**Major issues**

1. **HIGH — The website has no production topology, and the acceptance case has no environment.** Interview's backend is a Node `createServer` wrapping a spawned `snapback4 dev` owner ("loopback-only", `scripts/backend.mjs:1–15`); `src/config.ts` bakes `backendURL = "http://127.0.0.1:3211"`; Interview's `rules/NOT-DOING.md` excludes "Production hosting … this is local development" and "public access", and §9.5 is still open. The plan specifies the render server's HTTP contract in detail (1048.000 D10–D11) but never says what sits in front of it, where Interview's owner and adapter run, who serves assets, or how TLS and the domain work. Phase 1 can land completely in exact2 and there is still no website. *Change:* decide §9.5 and the deployable topology before kickoff, and put Interview's §10 list in an Interview LLP with an implementer — it is the larger half of phase 1.

2. **HIGH — Confidentiality labels, runtime capability-read tracking and tracked `Vary` are speculative apparatus for a guarantee the plan already has by construction.** 1048.000 D9 states the invariant that matters: the server renders with no request context, no cookies, an empty store. Phase 3 (1048.002 §1, §3, §5, D5–D7) then gives the server cookies and request headers and buys back safety with static label propagation, per-continuation read recording through Hermes (`js/src/lib.rs` ~743), whole-payload inspection and refusal tests — a proof obligation that never ends, in a module the repo calls "trusted app code, NOT a security sandbox" (`host/web/module-glue.js:2`). No consumer needs it: Interview's phase-3 column ("the viewer's parts are private holes") is identical if private means client hole, filled from the device's own replica after adoption. *Change:* make the anonymous server permanent. Keep `stage`; drop `confidentiality`. Request context becomes a declared allowlist of public inputs (client hints, `Accept-Language`) so `Vary` is declared, not tracked. Cookies never reach a render. This deletes most of 1048.002 §1/§3 and half of parent D3.

3. **MEDIUM — The render server should be a mode of the Linux host, fed by delivery, not a fifth host.** Everything it needs exists there: the executor core shared with Apple by `#[path]` (`host/linux/src/executor.rs:9`), grant-scoped outbound HTTP (the SSRF policy D10 asks for), Hermes loading, the delivery store (`host/linux/src/delivery.rs`), and the agent server — `tree`/`state`/`logs` against a live render is exactly the debugging you will want. A separate crate duplicates that wiring and drifts. Delivery matters operationally: a Contract change becomes a bundle the running server picks up at its next check, not a redeploy. Relatedly, for Interview the render should read a **local Snapback4 replica of a public partition** (the Rust device in `snapback4/src/lib.rs` already links into Interview via `native.rs`), not per-request HTTP to the Node adapter — whose anonymous `/state` branch returns no content anyway (`backend.mjs` ~165). Otherwise "no Node on the render path" is nominal: every render awaits Node.

4. **MEDIUM — The seeded boot names the wrong vehicle, and freshness is unstated.** Kept answers are store-readers only, capped at 8 KB ("never a feed"), and re-asked at `data_ready` (`runner/src/runner/kept.rs:17, 96–124`). A question page's answers fail all three. `boot_carrying` already seeds `ResourceState` with arguments (`runner/src/runner.rs` ~590); seed that way, and say explicitly whether the client re-asks after the module loads. If it does, every `request` page costs two backend reads and can flicker — under two engines, since the browser realm runs `app.js` while the server runs `app.hbc` under Hermes.

5. **MEDIUM — The inferred `eager` default keeps ~420 KB (brotli) on every Interview page.** LLP 1047 §1: 1.1 MB raw wasm for a 35-line app, 2.9× growth in 25 days with no budget, plus 41 KB of glue. Every Interview route has handlers, so `never` never applies and `eager` competes with images and fonts on the 6 Mbit/s profile the plan measures against. *Change:* `idle` is the default for rendered routes; land a byte budget (1047) before phase 2's `interaction`.

6. **MEDIUM — Phase 1's Contract surface is larger than its consumer needs and crosses a NOT-DOING number.** `contract/lower/src/tags.rs:62–150` has 22 tags; 1048.003 D2 adds about 20, landing at the "not 40" NOT-DOING names. The `SemanticTag` prop and `role="heading" aria-level=` already exist (Interview uses them), and §6 already lists "`h1`–`h6` from heading levels" as pending — headings are being specified twice. Width variants were pulled into phase 1 for the rail/tabs switch, but the rail renders only `when data.authenticated and wide` (`app.contract` ~413, ~637): anonymous readers, the whole of phase 1, never see it. *Change:* part A is `head`, `scroll document` and the route fields; elements come from roles; semantic tags and variants wait for a consumer that hand-authors a document.

7. **LOW — Realities the plan doesn't yet meet.** A Q&A site earns rich results from JSON-LD (`QAPage`), which `head` cannot express; `request`/`cached` routes have no sitemap unless `pages=` also feeds one; `robots.txt` needs an AI-crawler policy in 2026; the `Accept`-negotiated envelope with `Vary: Accept` on the HTML URL (`serve.mjs` `readWebRequest`) fragments CDN caches once pages are `cached` — serve it by `<link rel=alternate>` only. Security: the embedded checkpoint must escape `</script` and `<!--`; with no inline app JS a strict CSP is free and belongs in D11. Accessibility: don't mark controls disabled while activation is pending (replay already covers them; screen readers announce disabled), and a stacked presentation (`each e in stack(nav)`, absolutely positioned) will put the feed under every question page unless covered entries are hidden in the document.

8. **LOW — Rules.** 1048.001 is a Spec with "implementer assigned at phase 2 kickoff"; RULES says a spec without an implementer isn't written — keep it an RFC until then. Progressive forms are refused, yet `form submit=` opens 1048.001 §4.3; rule it: a form does nothing without the runtime.

**Top three changes**

1. Anonymous server by construction, forever: drop confidentiality and runtime read-tracking, keep stage, declare public request inputs, never forward cookies.
2. The server is the Linux host in a serve mode, updated by delivery, reading Interview from a local public replica; decide the production topology and §9.5 before kickoff.
3. Shrink phase 1 to what anonymous Interview readers need: `head`, `scroll document`, route fields, `idle` activation; semantic tags and width variants move behind a consumer.

VERDICT: DIRECTIONALLY RIGHT WITH MAJOR CHANGES

---

## Disposition (Claude, Opus 5.5 — the author), 2026-09-23

Three reviewers ran on the same brief, each blind to the others:
- Grok 4.7 (xAI);
- Fable 5.1;
- a fresh Opus 5.5 session.

Fable and Opus are Anthropic, the author's family. All three answered DIRECTIONALLY RIGHT WITH MAJOR CHANGES, and they agree on the end state:
- one document projection shared with the live host;
- a render policy per route;
- adoption from a document checkpoint that isn't reload state.

They converge on five major changes:

1. **Seeding.** Kept answers are the wrong way to carry the server's answers (all three). They seed only resources that read the store, they cap an answer at 8 KiB, and they are asked again at `data_ready` (`runner/src/runner/kept.rs:18`, `runner/src/runner.rs` ~573–593). A question page fails all three. Checkpoint answers become their own runner input:
   - kept out of the store;
   - keyed by source, arguments and logic identity;
   - sized for a page;
   - not asked again until their freshness lapses;
   - kept when a refresh fails.
2. **The first slice.** Phase 1 aimed the most expensive point on the curve at a consumer whose own rules exclude public access and production hosting (all three; `interview/rules/NOT-DOING.md` ~16–18). That point is a per-request server with no cache, a wasm boot that starts at once, and a DOM replacement.
3. **Caching.** A render that sees no cookies and an empty store depends only on its URL and public data. It is cached from the start (Claude, Grok), not served `no-store`.
4. **An anonymous server.** The server renders the public document, and the device renders the person (Fable, Claude, and Grok's "no credentials on the public render"). With no credentials and no private data on the render path, confidentiality labels, runtime read tracking and an inferred `Vary` protect nothing. Grok would keep the confidentiality label, but on such a server "private" can only mean "rendered on the device", and stage already says that.
5. **The Contract surface.** Phase 1's is too large (all three):
   - Tags with HTML defaults are kernel work on four hosts: `em` lengths, `list-item`, and classes at runtime.
   - Anonymous readers never see Interview's rail (`app.contract:413`, `when data.authenticated and wide`).
   - NOT-DOING says "roughly 15 built-in tags, not 40".

This reverses three r2 acceptances of Astra's review: rows 5 (confidentiality labels and a tracked `Vary`), 7 (tags with HTML defaults in phase 1) and 8 (width variants in phase 1). The problems those rows found still stand. What changes is the answer: nothing on the server can leak, and the tags and variants wait for a consumer.

The code claims I spot-checked hold:
- the kept path;
- `app.ts` ~132 and ~186;
- the rail's guard;
- the Linux executor sharing `executor_core.rs` by `#[path]` (`host/linux/src/executor.rs:9`);
- `renderMarkup`'s spans and `<br>` (`host/web/navigation.js:1420`);
- `readWebRequest` negotiating on `Accept` (`host/web/serve.mjs:591`);
- Interview's NOT-DOING.

One claim is misplaced. The global `h1`…`h6` reset is the web lane's commit, now on main as `44f8643c`; it isn't a `core/fixes` commit.

**What r3 will say.** This is a proposal. Charlie rules on the items marked ★.

- **1a: documents for the in-repo apps.**
  - They are rendered at build into `dist/` and served by whatever serves `dist/` today.
  - Covers the safe projection and its parity check, the head, `scroll document`, explicit route fields, the sitemap and the 404.
  - No server, no Hermes and no preload: the runtime loads when the page is idle or at the first interaction.
  - The render lane's steps 1–3 are this.
- **1b: Interview.**
  - **Gated on:**
    - ★ which content is public, including content written while Interview was invitation-only (§9.5);
    - ★ Interview's NOT-DOING lifting its public-access and production-hosting lines;
    - ★ a hosting target.
  - **Proposed topology:**
    - the Linux host in a serve mode, a native Rust binary (ruling 4), updated through delivery;
    - it holds a read-only replica of Interview's public partition, so the render path has no Node, no credentials and no private data;
    - it renders on request behind an ordinary CDN (`s-maxage`, `stale-while-revalidate`), purged by surrogate keys taken from the records a render read.
  - **Why render on request:** Grok would instead write static files when content changes. But a new question's URL has to answer at once, and the CDN keeps the index from page to record that rewriting on change would have to keep itself.
  - A render that doesn't settle answers 503 with `Retry-After`; an unknown item answers 404.
  - The server counts and budgets its startup: the module graph each render loads, the source calls, and time to first byte.
- **One boot path.**
  - The runtime adopts the document when the checkpoint digest matches, and renders fresh when it doesn't.
  - There is no replace-then-adopt migration.
  - The parts of 1048.001 that 1a uses (the checkpoint, the digest and adoption) fold into 1048.000.
- **Anonymous forever.**
  - Stage stays.
  - Confidentiality labels, runtime read tracking and an inferred `Vary` go, and so do credentialed holes and the edge.
  - Request inputs are a declared, closed list of public facts, and each one is part of the cache key.
  - Streaming waits for a measurement.
  - ★ This takes server-rendered signed-in content off the curve. A signed-in reader's parts are holes the device fills after adoption.
- **The Contract surface.**
  - Phase 1 adds `head`, `scroll document` and route fields.
  - Meaning comes from roles on plain nodes, lowered to HTML elements that keep a bare div's box, as `44f8643c` does for headings.
  - Authored tags with HTML defaults, and width and media variants, get their own kernel spec when a consumer needs them.
- **The working set.** 1048.001 and 1048.002 leave `llp/current/` until their kickoffs.

**This review's findings.** "Change N" refers to the five changes above.

| # | Finding | Disposition | Where it goes |
|---|---|---|---|
| 1 | No production topology; the acceptance case has no environment | **Accepted.**<br>• ★ Charlie: the hosting target and §9.5.<br>• Once he rules, Interview's §10 becomes an Interview LLP with an implementer. | 1048 §9–§10 (r3); Interview |
| 2 | Confidentiality labels, runtime read tracking and a tracked `Vary` are speculative | **Accepted** (change 4).<br>• Stage stays and confidentiality goes.<br>• Request inputs are declared public facts, and cookies never reach a render.<br>• Most of 1048.002 §1, §3 and §5 goes with them. | 1048 D3; 1048.002 (r3) |
| 3 | The server should be a serve mode of the Linux host, fed by delivery, reading a local public replica | **Accepted** as the proposed 1b topology (★ Charlie).<br>To confirm before r3 is final:<br>• a serve mode keeps the painter off the server's startup path;<br>• Snapback4 can hold a public partition. | 1048.000 D9–D10 (r3) |
| 4 | Seeding uses the wrong mechanism, and freshness is unstated | **Accepted** (change 1).<br>• The checkpoint seeds `ResourceState` the way `boot_carrying` does.<br>• The client doesn't ask again until freshness lapses, so a page costs one backend read.<br>• The two engines (`app.js` in the browser, `app.hbc` on the server) never race for the same answer. | 1048.000 D6 (r3) |
| 5 | An inferred `eager` keeps about 420 KB of runtime on every page | **Accepted.**<br>• By default the runtime loads when the page is idle or at the first interaction, with no preload.<br>• A byte budget (LLP 1047) comes before the `interaction` policy. | 1048.000 D3; 1048.001 (r3) |
| 6 | Phase 1's Contract surface crosses a NOT-DOING number | **Accepted** (change 5). | 1048.003 (r3) |
| 7 | JSON-LD, sitemaps, robots, `Vary: Accept`, escaping, CSP, disabled controls, stacked entries | **Accepted for 1b:**<br>• JSON-LD `QAPage` from the head;<br>• a sitemap from the public replica;<br>• the data envelope at its own URL, never negotiated on the HTML URL;<br>• checkpoint escaping;<br>• a strict CSP in D11;<br>• controls never disabled while activation is pending;<br>• covered stack entries left out of the document.<br>★ Charlie decides the robots policy for AI crawlers when 1b starts. | 1048.000, 1048.003 (r3) |
| 8 | 1048.001 is a Spec with no implementer; forms are unruled | **Accepted.**<br>• 1048.001 leaves the working set.<br>• Phase 1 has no `form`, because the new tags are cut.<br>• Whether a GET form navigates without the runtime is decided against NOT-DOING's refusal of progressive forms when someone proposes one. | 1048 (r3) |

**Proposed next step:**
- Charlie rules on the ★ items.
- r3 revises 1048, 1048.000 and 1048.003, and moves 1048.001 and 1048.002 out of the working set.
- The render lane continues 1a, with a policy declared on each route and no preload.
- The Interview lane continues behind its switch, which stays off by default.

No further review round is needed to settle the direction. Status stays Draft; Charlie decides.

**Update, 2026-09-23: after Charlie's rulings (r3).** Each item marked ★ is now resolved:
- **Public content:** "questions, answers, posts, profiles — but all at the preference of the user". Each author chooses, and the choice is off by default (LLP 1048 §9.5).
- **Hosting:** "for now just hosted locally, eventually on a CDN/hosting provider" (§9.6). Interview's NOT-DOING records public reading at the author's choice, and still keeps production hosting out.
- **The server's shape:** Charlie wasn't sure and asked the author to decide. It renders on request from a serve mode of the Linux host, on loopback, with headers ready for a cache (§9.7).
- **Pages for signed-in readers:** Charlie asked "why not server render the personal stuff?" They are now rendered on the server in a simple form: the whole page for that reader, `private, no-store`, never shared (§9.8, LLP 1048.002).
  - This reverses the disposition's "anonymous forever".
  - The reviews' objection still holds and that form stays out: personal parts mixed into cached pages, guarded by labels and read tracking.
- **The robots policy for AI crawlers** waits for real hosting.

r3 is in `llp/1048-rendering-across-the-curve.rfc.md` and its sub-LLPs. LLP 1048.001 became an RFC, and it and 1048.002 left the working set until their kickoffs.
