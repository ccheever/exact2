// Session setup shared by the agent CLI and its programmatic driver.
import { resolve } from 'node:path';

export function parseFlags(argv) {
  const flags = { json: false };
  const rest = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--json') flags.json = true;
    else if (argv[i] === '--world') flags.world = resolve(argv[++i]);
    else if (argv[i] === '--plan') flags.plan = resolve(argv[++i]);
    else if (argv[i] === '--app') flags.app = argv[++i];
    else if (argv[i] === '--size') flags.size = argv[++i].split('x').map(Number);
    else if (argv[i] === '--test') flags.test = argv[++i];
    else if (argv[i] === '--session') flags.session = argv[++i];
    else if (argv[i] === '--url') flags.url = argv[++i];
    else if (argv[i] === '--device') flags.device = true;
    else if (argv[i] === '--seed') flags.seed = Number(argv[++i]);
    else if (argv[i] === '--locale') flags.locale = argv[++i];
    else if (argv[i] === '--time-zone') flags.timeZone = argv[++i];
    else if (argv[i] === '--timing') flags.timing = argv[++i];
    else if (argv[i] === '--phone') flags.phone = argv[++i];
    else if (argv[i] === '--storage') flags.storage = argv[++i];
    else rest.push(argv[i]);
  }
  return { flags, rest };
}

export function launchFacts({seed, locale, timeZone, env = {}}) {
  seed = Number(seed ?? env.EXACT_AGENT_SEED ?? 1);
  locale = locale ?? env.EXACT_AGENT_LOCALE ?? 'en-US';
  timeZone = timeZone ?? env.EXACT_AGENT_TIME_ZONE ?? 'UTC';
  if (!Number.isSafeInteger(seed) || seed < 0) throw new Error('seed: an integer from 0 through 2^53 - 1');
  // Refuse malformed drive input before starting a process on any host.
  locale = Intl.getCanonicalLocales(locale)[0];
  if (!locale) throw new Error('locale: a BCP 47 language tag');
  new Intl.DateTimeFormat(locale, {timeZone}).format(0);
  return {seed, locale, timeZone};
}

export function launchEnvironment(facts) {
  return {EXACT_AGENT_SEED: String(facts.seed), EXACT_AGENT_LOCALE: facts.locale, EXACT_AGENT_TIME_ZONE: facts.timeZone};
}

/** The DevTools protocol over Chrome's --remote-debugging-pipe (fd 3 in, fd 4 out; NUL-delimited JSON). A closed pipe or a dead Chrome fails every pending call; every call has a deadline. */
export class Cdp {
  constructor(input, output) {
    this.input = input;
    this.next = 1;
    this.pending = new Map();
    this.listeners = [];
    this.closed = null;
    let buf = '';
    output.setEncoding('utf8');
    output.on('data', (d) => {
      buf += d;
      let i;
      while ((i = buf.indexOf('\0')) >= 0) {
        const msg = JSON.parse(buf.slice(0, i));
        buf = buf.slice(i + 1);
        if (msg.id) {
          const p = this.pending.get(msg.id);
          this.pending.delete(msg.id);
          if (msg.error) p?.reject(new Error(`${msg.error.message} (${p.method})`));
          else p?.resolve(msg.result);
        } else for (const l of this.listeners) l(msg);
      }
    });
    output.on('end', () => this.fail('the DevTools pipe closed'));
    output.on('error', (e) => this.fail(`the DevTools pipe failed: ${e.message}`));
    input.on('error', (e) => this.fail(`the DevTools pipe failed: ${e.message}`));
  }
  fail(why) {
    this.closed ??= why;
    for (const [id, p] of this.pending) { this.pending.delete(id); p.reject(new Error(`${why} (${p.method})`)); }
  }
  send(method, params = {}, sessionId, timeoutMs = 15000) {
    if (this.closed) return Promise.reject(new Error(`${this.closed} (${method})`));
    const id = this.next++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`${method} did not answer within ${timeoutMs} ms`)); }, timeoutMs);
      this.pending.set(id, { resolve: (v) => { clearTimeout(timer); resolve(v); }, reject: (e) => { clearTimeout(timer); reject(e); }, method });
      this.input.write(JSON.stringify({ id, method, params, sessionId }) + '\0');
    });
  }
}
