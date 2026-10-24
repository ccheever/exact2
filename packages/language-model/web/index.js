// @exact/language-model on the web (LLP 1133.016): the browser's Prompt API
// (`LanguageModel`, https://github.com/webmachinelearning/prompt-api), and
// under the agent the agent model of §9 on the session clock.
//
// - The transcript is this module's (D6): each session's system message and
//   turns (an input, the tool calls it caused, the reply), from which the
//   browser's session is built and rebuilt. The context window is counted by
//   the browser's own measure where it has one, else by §9's rule.
// - A response is a value the runtime polls (D7): its text so far, read from
//   a cell the browser's stream fills; only a change of `state` is announced.
// - A tool is the app's handler (D9), run through `context.invoke` for the
//   work it serves. The web's loop is open: a reply of `tool-call` items is
//   answered with `tool-response` messages and prompted again, at most 16
//   calls a response.
// - A download starts only from the app's own input in the foreground (D5):
//   the implementation applies the declaration's `download` rule, told the
//   call's origin, since only it knows the model is `downloadable`.
// - Under the agent no browser API is used (§9): replies come from the
//   script `prefer extension language-model.responses=[…]`, a token each 25 ms
//   of the session clock.

import { firstInstance, refusal, violation } from './schema.js';

const TOKEN_MS = 25, WINDOW = 4096, KEPT = 8, TOOL_LIMIT = 16, CUT = 4096;
const RESPONSE = 'language-model.languageModelResponse', SESSION = 'language-model.languageModelSession', AVAILABILITY = 'language-model.languageModelAvailability';
// What a read of no handle (`""`) answers: an app reads before it holds one, and a poll does not ask it again.
const NO_RESPONSE = { id: '', session: '', state: 'complete', text: '', toolCalls: [], overflowed: false };
const NO_SESSION = { id: '', contextWindow: 0, responding: false };

const bytes = s => new TextEncoder().encode(s).length;
/** §9's count: a message costs 4 and a token per 4 bytes; a schema a token per 4 bytes of its JSON. */
const cost = text => 4 + Math.ceil(bytes(text) / 4);
const schemaCost = schema => Math.ceil(bytes(JSON.stringify(schema)) / 4);
/** The first `CUT` bytes of `text`, and whether it was cut (D9). */
function cut(text) {
  if (bytes(text) <= CUT) return { text, truncated: false };
  let out = text.slice(0, CUT);
  while (bytes(out) > CUT) out = out.slice(0, -1);
  return { text: out, truncated: true };
}
/** Tokens as §9 cuts them: at the end of each run of whitespace. */
const tokens = text => text ? text.split(/(?<=\s)(?=\S)/) : [];
const errorOf = (e, fallback = 'OperationError') => ({ error: typeof e?.name === 'string' && /Error$/.test(e.name) ? e.name : fallback });
const isError = v => v && typeof v === 'object' && !Array.isArray(v) && Object.keys(v).length === 1 && typeof v.error === 'string';

export function implementation({ changed, agent, now, foreground, log, invoke }) {
  const say = line => log?.(`language-model: ${line}`);
  const Browser = !agent && typeof globalThis.LanguageModel === 'function' ? globalThis.LanguageModel : null;
  const sessions = new Map(), responses = new Map(), byRequest = new Map();
  let serial = 0;
  // The agent's words (§9) and its preferred availability, laid over the agent model's.
  let overlay = {}, script = [], scriptSeen = '', rate = TOKEN_MS, agentWindow = WINDOW;
  // A download a create started, and the creates waiting on it (D5).
  let downloading = null;
  const waiting = [];
  const announce = key => changed(key);

  // ---------------------------------------------------------------- availability
  const agentAvailability = () => {
    const base = { availability: 'available', contextWindow: agentWindow, leavesDevice: false, structuredOutput: 'enforced', tools: true };
    const a = { ...base, ...overlay };
    if (a.availability === 'downloadable' && downloading) return { ...a, availability: 'downloading', progress: downloading.progress };
    if (a.availability !== 'unavailable') delete a.reason;
    if (a.availability !== 'downloading') delete a.progress;
    return a;
  };
  const browserAvailability = async (options = {}) => {
    const out = { availability: 'unavailable', leavesDevice: false, structuredOutput: 'enforced', tools: false };
    if (!Browser) return { ...out, reason: 'not-supported' };
    let a;
    try { a = await Browser.availability(options); } catch { return { ...out, reason: 'unsupported-options' }; }
    if (downloading && a !== 'available') return { ...out, availability: 'downloading', progress: downloading.progress };
    if (a === 'unavailable') {
      // The browser names no reason; options it refuses are the app's to change.
      let plain = 'unavailable';
      if (options.expectedInputs || options.expectedOutputs) try { plain = await Browser.availability(); } catch {}
      return { ...out, reason: plain === 'unavailable' ? 'device-not-eligible' : 'unsupported-options' };
    }
    let tools = false;
    try { tools = (await Browser.availability({ ...options, expectedOutputs: [...(options.expectedOutputs ?? [{ type: 'text' }]), { type: 'tool-call' }] })) !== 'unavailable'; } catch {}
    return { ...out, availability: a, tools };
  };
  // A page shown again re-reads availability (D5): a model may have arrived or gone while it was hidden.
  if (Browser && typeof document === 'object') document.addEventListener('visibilitychange', () => { if (document.visibilityState === 'visible') announce(AVAILABILITY); });

  // ---------------------------------------------------------------- sessions and turns
  const windowOf = s => agent ? agentWindow : s.window;
  const turnCost = t => (t.input == null ? 0 : cost(t.input)) + (t.reply == null ? 0 : cost(t.reply))
    + t.calls.reduce((n, c) => n + cost(c.arguments + (c.result ?? '')), 0);
  const counted = s => (s.system == null ? 0 : cost(s.system)) + s.toolCost + s.turns.reduce((n, t) => n + turnCost(t), 0);
  const usage = s => s.platform?.contextUsage ?? s.platform?.inputUsage ?? counted(s);
  /** Evict the oldest whole turns until `extra` more fits (D6); false when even then it does not. */
  function fit(s, extra) {
    const had = s.turns.length;
    while (counted(s) + extra > windowOf(s) && s.turns.length) { s.turns.shift(); s.overflowed = true; s.stale = true; }
    if (s.turns.length < had) say(`${s.id}: evicted ${had - s.turns.length} turns, overflowed, ${counted(s)} of ${windowOf(s)} tokens`);
    return counted(s) + extra <= windowOf(s);
  }
  /** `initialPrompts` as the system message and turns: a user message and the assistant messages after it are one. */
  function turnsOf(messages) {
    let system = null;
    const turns = [];
    for (const [i, m] of messages.entries()) {
      if (m.role === 'system') {
        if (i !== 0) throw Object.assign(new Error('a system message comes first'), { name: 'TypeError' });
        system = m.content;
      } else if (m.role === 'user' || !turns.length) turns.push({ input: m.role === 'user' ? m.content : null, calls: [], reply: m.role === 'user' ? null : m.content });
      else { const t = turns.at(-1); t.reply = t.reply == null ? m.content : `${t.reply}\n\n${m.content}`; }
    }
    return { system, turns };
  }
  /** The transcript as the Prompt API's messages, tool exchanges as its `tool-call` and `tool-response` content. */
  function messagesOf(s) {
    const out = s.system == null ? [] : [{ role: 'system', content: s.system }];
    for (const t of s.turns) {
      if (t === s.responding?.turn) continue; // the input being prompted goes as the prompt
      if (t.input != null) out.push({ role: 'user', content: t.input });
      for (const c of t.calls) {
        out.push({ role: 'assistant', content: [{ type: 'tool-call', value: { callID: c.id, name: c.name, arguments: JSON.parse(c.fullArguments ?? c.arguments) } }] });
        out.push({ role: 'user', content: [{ type: 'tool-response', value: { callID: c.id, name: c.name, result: [{ type: 'text', value: c.fullResult ?? c.result ?? '' }] } }] });
      }
      if (t.reply != null) out.push({ role: 'assistant', content: t.reply });
    }
    return out;
  }
  const sessionInfo = s => ({ id: s.id, contextUsage: usage(s), contextWindow: windowOf(s), responding: !!s.responding || s.queue.length > 0 });
  const createdInfo = s => ({ id: s.id, contextUsage: usage(s), contextWindow: windowOf(s), tools: s.tools.map(t => t.name), overflowed: s.overflowed });
  const lookup = id => sessions.get(id) ?? null;

  /** The browser's session for `s`, built from the transcript when it has none or the transcript changed under it. */
  async function platform(s) {
    if (s.platform && !s.stale) return s.platform;
    s.platform?.destroy?.();
    s.platform = await Browser.create({ ...s.options, initialPrompts: messagesOf(s) });
    s.stale = false;
    s.platform.addEventListener?.('contextoverflow', () => { s.overflowed = true; });
    if (s.platform.contextWindow ?? s.platform.inputQuota) s.window = s.platform.contextWindow ?? s.platform.inputQuota;
    return s.platform;
  }

  function makeSession(args, platformSession = null) {
    const { system, turns } = turnsOf(args.initialPrompts ?? []);
    const tools = (args.tools ?? []).map(t => ({ name: t.name, description: t.description, inputSchema: typeof t.inputSchema === 'string' ? JSON.parse(t.inputSchema) : t.inputSchema }));
    const s = {
      id: `session-${++serial}`, system, turns, tools, toolCost: tools.length ? schemaCost(tools) : 0, overflowed: false,
      window: WINDOW, queue: [], responding: null, ended: [], count: 0, platform: platformSession, stale: false,
      options: { expectedInputs: args.expectedInputs, expectedOutputs: tools.length ? [...(args.expectedOutputs ?? [{ type: 'text' }]), { type: 'tool-call' }] : args.expectedOutputs },
    };
    if (platformSession && (platformSession.contextWindow ?? platformSession.inputQuota)) s.window = platformSession.contextWindow ?? platformSession.inputQuota;
    // A create trims as a prompt evicts, and fails only when the system message and the newest turn do not fit (D6).
    if (!fit(s, 0)) return { error: 'QuotaExceededError' };
    sessions.set(s.id, s);
    say(`${s.id}: created, ${s.turns.length} turns, ${counted(s)} of ${windowOf(s)} tokens${s.overflowed ? ', earlier turns evicted' : ''}${tools.length ? `, tools ${tools.map(t => t.name).join(', ')}` : ''}`);
    return s;
  }

  // ---------------------------------------------------------------- responses
  const responseInfo = r => {
    if (agent) advance(r);
    const out = { id: r.id, session: r.session.id, state: r.state, text: r.text, toolCalls: r.calls.map(c => ({ id: c.id, name: c.name, arguments: c.arguments, state: c.state, result: c.result, truncated: c.truncated })), overflowed: r.overflowed };
    if (r.contextUsage != null) out.contextUsage = r.contextUsage;
    if (r.error) Object.assign(out, { error: r.error, reason: r.reason ?? undefined, message: r.message ?? undefined });
    return out;
  };
  /** Under the agent: the tokens the session clock has reached, and the end once every one has landed. */
  function advance(r) {
    if (r.state !== 'generating' || !r.planned) return;
    const all = r.planned.length, reached = rate <= 0 || r.whole ? all : Math.floor((now() - r.start) / rate);
    const shown = Math.min(all - (r.hold ? 1 : 0), reached);
    r.text = r.planned.slice(0, Math.max(0, shown)).join('');
    if (shown >= all) end(r, 'complete', null, rate <= 0 || r.whole ? r.start : r.start + all * rate);
  }
  const advanceAll = () => { for (const r of responses.values()) advance(r); };

  /** The response ends: its turn committed with its text so far (D6), the session's next call begun at `at`. */
  function end(r, state, failure = null, at = now()) {
    if (r.state !== 'generating' && r.state !== 'calling-tool') return;
    r.state = state;
    if (failure) Object.assign(r, failure);
    const s = r.session, begun = s.responding === r;
    // One still waiting its turn never reached the session: nothing of it is committed.
    if (begun) r.turn.reply = r.text;
    else s.queue = s.queue.filter(q => q.r !== r);
    r.contextUsage = usage(s);
    r.controller?.abort();
    if (begun) s.responding = null;
    s.ended.push(r.id);
    // An ended response is kept until its session has ended eight newer (D6).
    while (s.ended.length > KEPT) { const old = s.ended.shift(); responses.delete(old); say(`${old}: retired`); }
    say(`${r.id}: ${state}${failure?.error ? ` (${failure.error}${failure.reason ? `, ${failure.reason}` : ''})` : ''}, ${r.text.length} characters, ${r.calls.length} tool calls`);
    r.settle?.(r);
    announce(RESPONSE);
    announce(SESSION);
    if (begun) nextIn(s, at);
  }
  /** The calls waiting on `s`, begun in order until one generates (D6: one generation a session). */
  function nextIn(s, at) {
    while (!s.responding && s.queue.length) s.queue.shift().go(at);
  }
  /** `work(at)` once the session is free; `r`, the response it begins, holds the session until it ends. */
  function queued(s, work, r = null) {
    return new Promise(resolve => {
      s.queue.push({ r, go: at => { if (r?.state === 'generating') s.responding = r; else if (r) return resolve(); resolve(work(at)); } });
      nextIn(s, now());
    });
  }

  /** The next script entry, in admission order (§9). */
  function entryFor() {
    return script.length ? script.shift() : null;
  }

  /** A prompt's response: made now, generated once its session is free. */
  function respond(s, args, how, streaming) {
    let schema = null, typed = false;
    if (args.responseConstraint != null) {
      typed = typeof args.responseConstraint === 'string';
      try { schema = typed ? JSON.parse(args.responseConstraint) : args.responseConstraint; } catch { return { error: 'SyntaxError' }; }
      const why = refusal(schema);
      if (why) { say(`a constraint outside D8's subset: ${why}`); return { error: 'NotSupportedError' }; }
    }
    if (typeof args.input !== 'string') return { error: 'TypeError' };
    const r = {
      id: `response-${++serial}`, session: s, state: 'generating', text: '', calls: [], overflowed: false, error: null,
      schema, typed, omit: !!args.omitResponseConstraintInput, whole: !streaming, turn: { input: args.input, calls: [], reply: null },
      handle: streaming ? null : s.id, request: how?.request ?? null,
    };
    responses.set(r.id, r);
    if (r.request != null) byRequest.set(r.request, r);
    r.done = new Promise(settle => { r.settle = settle; });
    s.count++;
    const begin = at => (agent ? beginAgent : beginBrowser)(r, at);
    queued(s, begin, r);
    return r;
  }

  /** The input fitted to the window, evicting oldest turns (D6): false, with the response failed, when it cannot. */
  function admitInput(r, extra = cost(r.turn.input) + (r.schema && !r.omit ? schemaCost(r.schema) : 0)) {
    const s = r.session, had = s.turns.length;
    s.responding = r;
    const fits = fit(s, extra);
    // A response reads `overflowed` when its own input evicted a turn.
    r.overflowed = s.turns.length < had;
    s.turns.push(r.turn);
    if (!fits) { end(r, 'failed', { error: 'QuotaExceededError', message: `the input needs ${extra} tokens of ${windowOf(s)}` }); return false; }
    return true;
  }

  // ---------------------------------------------------------------- the agent model (§9)
  function beginAgent(r, at) {
    if (r.state !== 'generating') return;
    if (!admitInput(r)) return;
    const entry = entryFor();
    if (entry?.error) { r.text = ''; end(r, 'failed', { error: entry.error, reason: entry.reason ?? null, message: entry.message ?? null }, at); return; }
    const calls = entry?.toolCalls ?? [];
    if (calls.length) {
      // The handlers run once this answer has reached the runtime, which registers the handle they serve.
      r.state = 'calling-tool';
      setTimeout(() => runCalls(r, calls).then(ok => { if (ok) plan(r, entry, now()); }), 0);
      return;
    }
    plan(r, entry, at);
  }
  /** The reply's text, checked as a constrained platform checks it, cut into tokens from `at`. */
  function plan(r, entry, at) {
    if (r.state !== 'generating' && r.state !== 'calling-tool') return;
    r.state = 'generating';
    let text = entry?.text ?? (entry?.value !== undefined ? JSON.stringify(entry.value) : null);
    if (text == null) text = r.schema ? JSON.stringify(firstInstance(r.schema)) : `Agent model reply ${r.session.count}.`;
    if (r.schema) {
      let v, why;
      try { v = JSON.parse(text); why = violation(r.schema, v); } catch { why = 'the text is not JSON'; }
      // A constrained platform could not produce it (§9): the response fails, and the journal names why.
      if (why) { say(`${r.id}: the script's reply does not satisfy the constraint: ${why}`); end(r, 'failed', { error: 'SyntaxError', message: why }, at); return; }
    }
    r.planned = tokens(text);
    r.hold = !!entry?.hold;
    r.start = at;
    if (r.calls.length) announce(RESPONSE);
    advance(r);
  }
  /** Each tool call through the runtime, in order (D9): false when the response ended meanwhile. */
  async function runCalls(r, calls) {
    const s = r.session;
    for (const [i, call] of calls.entries()) {
      if (r.state !== 'calling-tool') return false;
      if (r.calls.length >= TOOL_LIMIT) { end(r, 'failed', { error: 'OperationError', reason: 'tool-limit' }); return false; }
      const tool = s.tools.find(t => t.name === call.name);
      const why = !tool ? `the session lists no tool \`${call.name}\`` : violation(tool.inputSchema, call.arguments ?? {});
      if (why) { say(`${r.id}: the script's tool call is refused: ${why}`); end(r, 'failed', { error: 'SyntaxError', message: why }); return false; }
      const full = JSON.stringify(call.arguments ?? {}), a = cut(full);
      const c = { id: `${r.id}.call-${i + 1}`, name: call.name, arguments: a.text, fullArguments: full, state: 'running', result: null, truncated: a.truncated };
      r.calls.push(c);
      r.turn.calls.push(c);
      announce(RESPONSE);
      await toolCall(r, c, call.arguments ?? {});
    }
    return r.state === 'calling-tool';
  }
  /** One handler's run, its answer or failure kept as the model saw it. */
  async function toolCall(r, c, args) {
    const answer = await invoke(r.handle ?? r.id, c.name, args);
    if (r.state !== 'calling-tool' && r.state !== 'generating') return answer;
    const full = isError(answer) || answer?.error ? JSON.stringify({ error: answer.error, message: answer.message ?? '' }) : answer.text;
    const result = cut(full);
    Object.assign(c, { state: answer?.error ? 'failed' : 'done', result: result.text, fullResult: full, truncated: c.truncated || result.truncated });
    announce(RESPONSE);
    return answer;
  }

  // ---------------------------------------------------------------- the browser (D1, D9)
  async function beginBrowser(r, at) {
    if (r.state !== 'generating') return;
    const s = r.session;
    let session;
    try { session = await platform(s); } catch (e) { s.turns.push(r.turn); end(r, 'failed', errorOf(e)); return; }
    r.controller = new AbortController();
    const options = { signal: r.controller.signal };
    if (r.schema) Object.assign(options, { responseConstraint: r.schema, omitResponseConstraintInput: r.omit });
    let input = r.turn.input;
    // The browser's own count of the input, where it measures, evicts by D6's rule before the prompt.
    let extra;
    const measure = session.measureContextUsage ?? session.measureInputUsage;
    if (measure) try { extra = await measure.call(session, input, options); } catch {}
    if (!admitInput(r, extra)) return;
    try {
      if (r.overflowed) session = await platform(s);
      for (;;) {
        if (r.state !== 'generating') return;
        // Text pieces land in the cell as they come; a `tool-call` item is collected (the explainer's open loop).
        const stream = session.promptStreaming(input, options), calls = [];
        r.text = '';
        for await (const piece of stream) {
          if (typeof piece === 'string') r.text += piece;
          else calls.push(...toolCallsIn(piece));
        }
        if (!calls.length) break;
        // The open loop: each call through the runtime, answered as `tool-response` messages, then prompted again.
        r.state = 'calling-tool';
        announce(RESPONSE);
        const answers = [];
        for (const call of calls) {
          if (r.calls.length >= TOOL_LIMIT) { end(r, 'failed', { error: 'OperationError', reason: 'tool-limit' }); return; }
          const full = JSON.stringify(call.arguments ?? {}), a = cut(full);
          const c = { id: call.callID ?? `${r.id}.call-${r.calls.length + 1}`, name: call.name, arguments: a.text, fullArguments: full, state: 'running', result: null, truncated: a.truncated };
          r.calls.push(c);
          r.turn.calls.push(c);
          announce(RESPONSE);
          const answer = await toolCall(r, c, call.arguments ?? {});
          answers.push({ type: 'tool-response', value: { callID: c.id, name: c.name, result: answer?.error ? undefined : [{ type: 'text', value: answer.text }], error: answer?.error ? c.result : undefined } });
        }
        if (r.state !== 'calling-tool') return;
        r.state = 'generating';
        announce(RESPONSE);
        input = [{ role: 'user', content: answers }];
      }
      s.overflowed ||= r.overflowed;
      end(r, 'complete');
    } catch (e) {
      if (r.state !== 'generating' && r.state !== 'calling-tool') return;
      const failure = errorOf(e);
      if (failure.error === 'QuotaExceededError') s.overflowed = r.overflowed = true;
      // The browser keeps no cancelled reply: the next prompt rebuilds from the transcript (D6).
      s.stale = true;
      end(r, failure.error === 'AbortError' ? 'aborted' : 'failed', failure.error === 'AbortError' ? null : failure);
    }
  }
  /** The `tool-call` items of a piece that is not text: one item, or a list of them. */
  const toolCallsIn = piece => [piece].flat().filter(x => x?.type === 'tool-call').map(x => x.value);

  // ---------------------------------------------------------------- the names
  const abort = (r, why) => {
    if (agent) advance(r);
    if (r.state !== 'generating' && r.state !== 'calling-tool') return;
    say(`${r.id}: ${why}`);
    end(r, 'aborted', { error: 'AbortError' });
  };
  const done = id => ({ id });
  const responseOf = id => { const r = responses.get(id); return r ?? null; };

  return {
    async languageModelAvailability(args = {}) {
      return agent ? agentAvailability() : browserAvailability(args);
    },
    async createLanguageModel(args = {}, how = {}) {
      if (agent) advanceAll();
      const available = agent ? agentAvailability() : null;
      if (agent) {
        if (available.availability === 'unavailable') return { error: 'NotSupportedError' };
        if (args.tools?.length && !available.tools) return { error: 'NotSupportedError' };
        if (available.availability === 'downloadable' || available.availability === 'downloading') {
          if (!downloading) {
            // A download is a person's act (D5, D10): the app's own call, in the foreground.
            if (how.origin !== 'app') { say(`create: refused, person-required (${how.origin} origin) to start a download`); return { error: 'NotAllowedError' }; }
            if (!foreground()) { say('create: refused, a download starts in the foreground'); return { error: 'NotAllowedError' }; }
            downloading = { progress: 0 };
            say('download started (agent)');
            announce(AVAILABILITY);
          }
          await new Promise((resolve, reject) => waiting.push({ resolve, reject }));
        }
        try { return createdOr(makeSession(args)); } catch (e) { return errorOf(e, 'TypeError'); }
      }
      if (!Browser) return { error: 'NotSupportedError' };
      // The browser's create runs here, inside the press's dispatch when it came from one, so its activation holds (D15 item 5).
      let made;
      try {
        const { system, turns } = turnsOf(args.initialPrompts ?? []);
        const draft = { system, turns, tools: args.tools ?? [] };
        const options = {
          expectedInputs: args.expectedInputs,
          expectedOutputs: draft.tools.length ? [...(args.expectedOutputs ?? [{ type: 'text' }]), { type: 'tool-call' }] : args.expectedOutputs,
          initialPrompts: messagesOf({ ...draft, turns: draft.turns }),
          monitor: m => m.addEventListener('downloadprogress', e => {
            downloading ??= { progress: 0 };
            downloading.progress = e.loaded;
            announce(AVAILABILITY);
          }),
        };
        const creating = Browser.create(options);
        made = await creating;
      } catch (e) {
        downloading = null;
        say(`create: ${e?.name ?? 'failed'}: ${e?.message ?? e}`);
        return errorOf(e, 'NotSupportedError');
      }
      if (downloading) { downloading = null; announce(AVAILABILITY); }
      try { return createdOr(makeSession(args, made)); } catch (e) { made.destroy?.(); return errorOf(e, 'TypeError'); }
    },
    async prompt(args, how) {
      const s = lookup(args.session);
      if (!s) return { error: 'InvalidStateError' };
      if (how?.origin === 'tool' && inHandlerOf(s)) return { error: 'InvalidStateError' };
      const r = respond(s, args, how, false);
      if (isError(r)) return r;
      await r.done;
      if (r.request != null) byRequest.delete(r.request);
      if (r.error) return { error: r.error };
      if (!r.typed) return { text: r.text, contextUsage: r.contextUsage, overflowed: r.overflowed };
      try { return JSON.parse(r.text); } catch { return { error: 'SyntaxError' }; }
    },
    async promptStreaming(args, how) {
      const s = lookup(args.session);
      if (!s) return { error: 'InvalidStateError' };
      const r = respond(s, args, how, true);
      if (isError(r)) return r;
      return { id: r.id, session: s.id };
    },
    async languageModelResponse({ response }) {
      if (!response) return NO_RESPONSE;
      const r = responseOf(response);
      return r ? responseInfo(r) : { error: 'InvalidStateError' };
    },
    async abortLanguageModelResponse({ response }) {
      const r = responseOf(response);
      if (!r) return { error: 'InvalidStateError' };
      abort(r, 'aborted by the app');
      return done(r.id);
    },
    async appendLanguageModel({ session, input }) {
      const s = lookup(session);
      if (!s) return { error: 'InvalidStateError' };
      return queued(s, async () => {
        const turn = { input, calls: [], reply: null };
        if (!fit(s, cost(input))) return { error: 'QuotaExceededError' };
        s.turns.push(turn);
        if (s.platform && !s.stale) try { await s.platform.append([{ role: 'user', content: input }]); } catch { s.stale = true; }
        announce(SESSION);
        return sessionInfo(s);
      });
    },
    async measureContextUsage({ session, input, responseConstraint }) {
      const s = lookup(session);
      if (!s) return { error: 'InvalidStateError' };
      const schema = typeof responseConstraint === 'string' ? JSON.parse(responseConstraint) : responseConstraint;
      if (!agent && s.platform && !s.stale) {
        const measure = s.platform.measureContextUsage ?? s.platform.measureInputUsage;
        if (measure) try { return { usage: await measure.call(s.platform, input, schema ? { responseConstraint: schema } : {}), contextWindow: windowOf(s) }; } catch (e) { return errorOf(e); }
      }
      return { usage: cost(input) + (schema ? schemaCost(schema) : 0), contextWindow: windowOf(s) };
    },
    async languageModelSession({ session }) {
      if (!session) return NO_SESSION;
      if (agent) advanceAll();
      const s = lookup(session);
      return s ? sessionInfo(s) : { error: 'InvalidStateError' };
    },
    async cloneLanguageModel({ session }) {
      const s = lookup(session);
      if (!s) return { error: 'InvalidStateError' };
      return queued(s, () => {
        const c = { ...s, id: `session-${++serial}`, turns: s.turns.map(t => ({ ...t, calls: [...t.calls] })), queue: [], responding: null, ended: [], count: 0, platform: null, stale: true };
        sessions.set(c.id, c);
        say(`${c.id}: cloned from ${s.id}`);
        return createdInfo(c);
      });
    },
    async destroyLanguageModel({ session }) {
      const s = lookup(session);
      if (!s) return { error: 'InvalidStateError' };
      drop(s, 'destroyed by the app');
      return done(session);
    },
    async prewarmLanguageModel({ session }) {
      const s = lookup(session);
      if (!s) return { error: 'InvalidStateError' };
      if (!agent && Browser) try { await platform(s); } catch (e) { return errorOf(e); }
      return done(session);
    },
    /** Under the agent: the preferred availability, laid over the agent model's, and the script, rate and window (§9). */
    preferred(values = {}) {
      const a = values.languageModelAvailability;
      overlay = a && typeof a === 'object' && !isError(a) ? a : {};
      const seen = JSON.stringify(values.responses ?? null);
      if (seen !== scriptSeen) { scriptSeen = seen; script = Array.isArray(values.responses) ? values.responses.map(e => ({ ...e })) : []; say(`script: ${script.length} replies`); }
      rate = typeof values.rate === 'number' ? values.rate : TOKEN_MS;
      agentWindow = typeof values.contextWindow === 'number' && values.contextWindow > 0 ? values.contextWindow : WINDOW;
      advanceAll();
      const now_ = agentAvailability();
      if (now_.availability === 'available' && (downloading || waiting.length)) {
        downloading = null;
        say('download finished (agent)');
        for (const w of waiting.splice(0)) w.resolve();
      } else if (now_.availability === 'unavailable' && waiting.length) {
        downloading = null;
        for (const w of waiting.splice(0)) w.reject(Object.assign(new Error('the model became unavailable'), { name: 'NotSupportedError' }));
      }
    },
    /** A call let go (D15 item 2): its prompt's generation aborted. */
    cancel(request) {
      const r = byRequest.get(request);
      if (!r) return;
      byRequest.delete(request);
      abort(r, `cancelled: its call (request ${request}) was let go`);
    },
    /** Sessions and responses whose reply was dropped: never the app's (D6). */
    dispose(ids) {
      for (const id of ids) {
        const s = sessions.get(id);
        if (s) drop(s, 'its reply was dropped');
        const r = responses.get(id);
        if (r) abort(r, 'its reply was dropped');
      }
    },
    /** The end of the app's session: every response aborted, every session dropped (D6). */
    destroy() { for (const s of [...sessions.values()]) drop(s, 'the app\'s session ended'); },
    release() {},
    /** `state.extensions` (§9): the sessions, each response, the script's remaining entries. */
    state() {
      if (agent) advanceAll();
      return {
        sessions: [...sessions.values()].map(s => ({ id: s.id, turns: s.turns.length, contextUsage: usage(s), contextWindow: windowOf(s), tools: s.tools.map(t => t.name), overflowed: s.overflowed, responding: s.responding?.id ?? null })),
        responses: [...responses.values()].map(r => ({ id: r.id, session: r.session.id, state: r.state, text: r.text.length, toolCalls: r.calls.map(c => ({ name: c.name, state: c.state, origin: `tool:model:${r.handle ?? r.id}` })), ...(r.error ? { error: r.error } : {}) })),
        ...(agent ? { script: script.length, rate, contextWindow: agentWindow } : {}),
        ...(downloading ? { downloading: downloading.progress } : {}),
      };
    },
  };

  function createdOr(s) { return isError(s) ? s : createdInfo(s); }
  function drop(s, why) {
    for (const r of responses.values()) if (r.session === s) { abort(r, why); responses.delete(r.id); }
    s.queue = [];
    s.platform?.destroy?.();
    sessions.delete(s.id);
    say(`${s.id}: ${why}`);
    announce(SESSION);
  }
  /** A handler of `s`'s own response prompting `s` would wait on itself (D9, reentry). */
  function inHandlerOf(s) { return s.responding?.calls.some(c => c.state === 'running'); }
}
