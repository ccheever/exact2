import { execFileSync, spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { cpSync, existsSync, lstatSync, mkdirSync, readFileSync, realpathSync, readdirSync, renameSync, rmdirSync, rmSync, statfsSync, writeFileSync } from 'node:fs';
import { homedir, hostname } from 'node:os';
import { basename, dirname, isAbsolute, join, relative, resolve } from 'node:path';

export const GiB = 1024 ** 3;
export const defaultRoot = join(homedir(), '.local/state/exact-t3-coordination/v1');
const hash = value => createHash('sha256').update(value).digest('hex').slice(0, 24);
const fail = message => { throw new Error(message); };
const inside = (parent, child) => child === parent || (!relative(parent, child).startsWith('..') && !isAbsolute(relative(parent, child)));
const json = path => JSON.parse(readFileSync(path, 'utf8'));
const atomic = (path, value) => {
  const temp = `${path}.${randomUUID()}.tmp`;
  writeFileSync(temp, `${JSON.stringify(value, null, 2)}\n`, { mode: 0o600 });
  renameSync(temp, path);
};
export function command(args, cwd) {
  return execFileSync(args[0], args.slice(1), { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
}
const bootId = (() => {
  try { return hash(process.platform === 'darwin' ? command(['sysctl', '-n', 'kern.bootsessionuuid']) : readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim()); }
  catch { return null; }
})();
export function processHostMatches(receipt, current, bindings = {}) {
  if (!receipt || !current) return false;
  if (receipt.bootId) return !!current.bootId && receipt.bootId === current.bootId;
  if (Object.hasOwn(bindings, receipt.host)) return !!current.bootId && bindings[receipt.host]?.bootId === current.bootId;
  return receipt.host === current.host;
}
export function processIdentity(pid) {
  try {
    const value = command(['ps', '-p', String(pid), '-o', 'lstart=', '-o', 'stat=']);
    if (!value || /\bZ/.test(value)) return null;
    return { pid, host: hostname(), bootId, started: value.replace(/\s+[A-Z+<Nssl]+\s*$/, '').trim() };
  } catch {
    try { process.kill(pid, 0); } catch (error) { if (error.code === 'ESRCH') return null; }
    return { pid, host: hostname(), bootId, started: null };
  }
}
function processState(receipt, bindings) {
  if (!processHostMatches(receipt, { host: hostname(), bootId }, bindings)) return 'unknown';
  const current = processIdentity(receipt.pid);
  if (!current) return 'dead';
  if (!receipt.started || !current.started) return 'unknown';
  return current.started === receipt.started ? 'live' : 'dead';
}
function groupMembers(group) {
  if (!Number.isSafeInteger(group) || group <= 0) fail('unknown process group');
  return command(['ps', '-axo', 'pid=,pgid=,stat=']).split('\n').flatMap(line => {
    const [pid, pgid, status] = line.trim().split(/\s+/);
    if (Number(pgid) !== group || status?.startsWith('Z')) return [];
    return [processIdentity(Number(pid)) ?? { pid: Number(pid), host: hostname(), started: null }];
  });
}
function noLinks(path) {
  for (let current = resolve(path);; current = dirname(current)) {
    if (existsSync(current) && lstatSync(current).isSymbolicLink()) fail(`symlink refused: ${current}`);
    if (dirname(current) === current) break;
  }
}
const terminalStatuses = new Set(['completed', 'failed', 'cancelled', 'interrupted', 'rolled_back']);
export function terminalProof(proof, task) {
  const native = proof.nativeCollaboration?.agents?.find(a => a.agent_name === task.thread);
  if (task.thread.startsWith('/root/') && proof.worktreePath === task.worktree &&
      native && Object.keys(native.agent_status ?? {}).length === 1 &&
      typeof native.agent_status.completed === 'string' &&
      !task.outputs.length && !task.processes.length && !task.pending.length) {
    return { threadId: task.thread, status: 'completed', kind: 'native-investigation' };
  }
  const t = proof.structuredContent?.thread ?? proof.thread;
  if (!t || t.threadId !== task.thread || t.worktreePath !== task.worktree ||
      !terminalStatuses.has(t.status) || t.activeRunId || t.pendingRequestCount !== 0) {
    fail('terminal T3 thread proof required; active or unknown owner refused');
  }
  return { threadId: t.threadId, status: t.status, updatedAt: t.updatedAt };
}

export class Coordination {
  constructor(worktree, { root = defaultRoot, freeBytes } = {}) {
    this.worktree = realpathSync(worktree);
    this.common = realpathSync(command(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], this.worktree));
    this.repo = hash(this.common);
    this.root = resolve(root);
    noLinks(this.root);
    mkdirSync(this.root, { recursive: true, mode: 0o700 });
    this.freeBytes = freeBytes ?? (path => { const s = statfsSync(path); return s.bavail * s.bsize; });
  }
  async transaction(fn) {
    const lock = join(this.root, 'mutex');
    const deadline = Date.now() + 10000;
    for (;;) {
      try { mkdirSync(lock); break; } catch (error) {
        if (error.code !== 'EEXIST') throw error;
        if (Date.now() >= deadline) fail('coordination mutex busy; inspect owner.json, never reclaim by age');
        await new Promise(r => setTimeout(r, 50));
      }
    }
    try {
      atomic(join(lock, 'owner.json'), processIdentity(process.pid));
      const path = join(this.root, 'state.json');
      const s = existsSync(path) ? json(path) : { repos: {}, tasks: {}, devices: {}, leases: {}, events: [] };
      const result = fn(s);
      atomic(path, s);
      return result;
    } finally { rmSync(lock, { recursive: true }); }
  }
  recoverMutex() {
    const lock = join(this.root, 'mutex');
    const owner = json(join(lock, 'owner.json'));
    const state = existsSync(join(this.root, 'state.json')) ? json(join(this.root, 'state.json')) : {};
    if (this.processState(state, owner) !== 'dead') fail('mutex owner live or unknown; refused');
    // Exclusive recovery must not race a second recovery or a new claim.
    const recovery = join(this.root, 'mutex-recovery');
    mkdirSync(recovery);
    try {
      if (JSON.stringify(json(join(lock, 'owner.json'))) !== JSON.stringify(owner)) fail('mutex owner changed');
      rmSync(join(lock, 'owner.json'));
      rmdirSync(lock);
    } finally { rmSync(recovery, { recursive: true }); }
    return { recovered: owner };
  }
  event(s, task, action, details = {}) {
    const entry = { at: new Date().toISOString(), task: task?.id, thread: task?.thread, worktree: task?.worktree, action, ...details };
    s.events.push(entry);
    return entry;
  }
  processState(s, receipt) { return processState(receipt, s.repos?.[this.repo]?.processHosts); }
  bindProcessHost(thread, proof) {
    return this.transaction(s => {
      this.coordinator(s, thread);
      if (!bootId || proof.bootId !== bootId || proof.toHost !== hostname() || !proof.fromHost || proof.fromHost === proof.toHost) fail('current boot and changed hostname required');
      const task = this.task(s, proof.anchor?.taskId);
      const lease = Object.values(s.leases).find(l => l.task === task.id && l.operation === proof.anchor.operation);
      const old = lease?.guard;
      if (!old || old.host !== proof.fromHost || old.bootId || old.pid !== proof.anchor.pid || old.started !== proof.anchor.started) fail('matching legacy launcher witness required');
      const current = processIdentity(old.pid);
      if (!current?.started || current.started !== old.started || current.bootId !== bootId) fail('legacy launcher witness is dead, changed or unknown');
      const cwd = process.platform === 'linux' ? realpathSync(`/proc/${old.pid}/cwd`) : command(['lsof', '-a', '-p', String(old.pid), '-d', 'cwd', '-Fn']).split('\n').find(line => line.startsWith('n'))?.slice(1);
      if (!cwd || realpathSync(cwd) !== task.worktree) fail('launcher witness worktree mismatch');
      const argv = command(['ps', '-p', String(old.pid), '-o', 'command=']);
      if (!argv.includes(`--task ${task.id}`) || !argv.includes(`--thread ${task.thread}`)) fail('launcher witness ownership arguments mismatch');
      const script = argv.trim().split(/\s+/)[1];
      const fixture = this.root !== resolve(defaultRoot) && script === join(import.meta.dir, 'fixture.mjs');
      if (!fixture && (basename(script ?? '') !== 'coordination.mjs' || realpathSync(command(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], dirname(resolve(cwd, script)))) !== this.common)) fail('launcher witness helper mismatch');
      const bindings = s.repos[this.repo].processHosts ??= {};
      if (bindings[proof.fromHost] && bindings[proof.fromHost].bootId !== bootId) fail('legacy hostname is bound to another boot');
      const binding = { bootId, toHost: hostname(), anchor: { ...proof.anchor, observed: current, worktree: cwd, commandHash: hash(argv) } };
      bindings[proof.fromHost] = binding;
      return this.event(s, task, 'process-host-bound', { fromHost: proof.fromHost, binding });
    });
  }
  task(s, id, thread) {
    const task = s.tasks[id] ?? fail('unknown task');
    if (task.repo !== this.repo) fail('different repository');
    if (thread !== undefined && task.thread !== thread) fail('task belongs to another thread');
    return task;
  }
  active(s, id, thread) {
    const task = this.task(s, id, thread);
    if (task.status !== 'active' || task.worktree !== this.worktree) fail('task is not active in this worktree');
    return task;
  }
  coordinator(s, thread) {
    if (s.repos[this.repo]?.coordinator !== thread) fail('coordinator thread required');
  }
  init(thread) {
    return this.transaction(s => {
      if (s.repos[this.repo] && s.repos[this.repo].coordinator !== thread) fail('repository already has a coordinator');
      return s.repos[this.repo] ??= { common: this.common, coordinator: thread };
    });
  }
  register(issue, thread) {
    return this.transaction(s => {
      if (!s.repos[this.repo]) fail('initialize coordinator first');
      const existing = Object.values(s.tasks).find(t => t.repo === this.repo && t.issue === issue && t.status !== 'retired');
      if (existing) {
        if (existing.thread === thread && existing.worktree === this.worktree && existing.status === 'active') return existing;
        fail(`issue already owned by ${existing.thread}: ${existing.id}`);
      }
      if (Object.values(s.tasks).some(t => t.worktree === this.worktree && t.status !== 'retired')) fail('worktree already owned');
      const branch = command(['git', 'symbolic-ref', '--short', 'HEAD'], this.worktree);
      const task = { id: randomUUID(), repo: this.repo, issue, thread, worktree: this.worktree, branch, status: 'active', outputs: [], processes: [], evidence: [], pending: [] };
      s.tasks[task.id] = task;
      this.event(s, task, 'registered');
      return task;
    });
  }
  status() { return this.transaction(s => s); }
  grant(id, thread) {
    return this.transaction(s => {
      this.coordinator(s, thread);
      const task = this.task(s, id);
      if (task.status !== 'active') fail('inactive task');
      const key = `publish:${this.repo}`;
      if (s.leases[key]) fail(`publishing turn owned by ${s.leases[key].task}`);
      s.leases[key] = { task: id, grantedBy: thread };
      return this.event(s, task, 'publish-granted');
    });
  }
  releaseTurn(id, thread) {
    return this.transaction(s => {
      const task = this.task(s, id, thread);
      const key = `publish:${this.repo}`;
      if (s.leases[key]?.task !== id) fail('publishing turn belongs to another task');
      if (s.leases[key].operation) fail('audit the launched operation before releasing its turn');
      if (s.leases[key].guard && this.processState(s, s.leases[key].guard) !== 'dead') fail('publisher still running or unknown');
      delete s.leases[key];
      return this.event(s, task, 'publish-released');
    });
  }
  clean(task) {
    if (command(['git', 'status', '--porcelain', '--untracked-files=all'], task.worktree)) fail('uncommitted source or untracked files; preserve first');
    if (existsSync(join(command(['git', 'rev-parse', '--absolute-git-dir'], task.worktree), 'rebase-merge')) ||
        existsSync(join(command(['git', 'rev-parse', '--absolute-git-dir'], task.worktree), 'rebase-apply'))) fail('unfinished rebase');
  }
  safePath(task, path) {
    path = resolve(task.worktree, path);
    noLinks(path);
    const rel = relative(task.worktree, path);
    const roots = ['target', 'examples/t3-code/target', 'examples/t3-code/mobile/target', 'examples/t3-code/mobile/dist', 'examples/t3-code/mobile/assets/browser-mobile'];
    if (!inside(task.worktree, path) || !roots.some(r => rel === r || rel.startsWith(`${r}/`))) fail('output outside disposable build roots');
    if (command(['git', 'ls-files', '--', rel], task.worktree)) fail('tracked source in output');
    for (let parent = path; parent !== task.worktree; parent = dirname(parent)) {
      if (existsSync(join(parent, '.git'))) fail('nested Git repository in output');
    }
    return path;
  }
  allocate(s, task, path, budgetGiB) {
    if (!Number.isFinite(budgetGiB) || budgetGiB <= 0) fail('positive --budget-gib required');
    path = this.safePath(task, path);
    if (existsSync(path)) fail('output exists; refuse adopting unrelated output');
    let volume = dirname(path);
    while (!existsSync(volume)) volume = dirname(volume);
    const free = this.freeBytes(volume);
    if (free - budgetGiB * GiB < 50 * GiB) fail(`50 GiB reserve refused: free=${free}, allocation=${budgetGiB * GiB}`);
    if (Object.values(s.tasks).some(t => t.outputs.some(o => !o.cleaned && (inside(path, o.path) || inside(o.path, path))))) fail('output overlaps another receipt');
    if (!command(['git', 'check-ignore', '--no-index', `${relative(task.worktree, path)}/`], task.worktree)) fail('output must be Git-ignored');
    mkdirSync(path, { recursive: true });
    const st = lstatSync(path);
    const output = { path, dev: st.dev, ino: st.ino, budgetGiB, freeBefore: free };
    task.outputs.push(output);
    return output;
  }
  recordOutput(id, thread, path, budgetGiB) {
    return this.transaction(s => this.allocate(s, this.active(s, id, thread), path, budgetGiB));
  }
  async run(id, thread, kind, args, options = {}) {
    if (!args.length || !['native-build', 'process', 'publish'].includes(kind)) fail('command and valid run kind required');
    const operation = randomUUID();
    const key = kind === 'native-build' ? 'native-build' : kind === 'publish' ? `publish:${this.repo}` : `process:${operation}`;
    await this.transaction(s => {
      const task = this.active(s, id, thread);
      if (key) {
        if (kind === 'publish') {
          if (s.leases[key]?.task !== id || s.leases[key].guard) fail('unused coordinator publishing turn required');
          this.clean(task);
        } else if (s.leases[key]) fail(`native build owned by ${s.leases[key].task}`);
        if (kind === 'native-build') {
          if (options.output) this.allocate(s, task, options.output, options.budgetGiB);
          else {
            const output = task.outputs.find(o => !o.cleaned && o.path === resolve(task.worktree, options.existingOutput ?? ''));
            if (!output) fail('owned --output or --existing-output required');
            this.safePath(task, output.path);
            const st = lstatSync(output.path);
            if (st.dev !== output.dev || st.ino !== output.ino) fail('output identity changed');
            const free = this.freeBytes(output.path);
            if (!Number.isFinite(options.budgetGiB) || options.budgetGiB <= 0 || free - options.budgetGiB * GiB < 50 * GiB) fail('50 GiB reserve refused');
          }
        }
        s.leases[key] = { ...s.leases[key], task: id, guard: processIdentity(process.pid), operation, phase: 'running', kind };
      }
      task.pending.push(operation);
      task.processes.push({ ...processIdentity(process.pid), operation, role: 'guard' });
      this.event(s, task, `${kind}-started`, { operation, args });
    });
    let child;
    try {
      child = spawn(args[0], args.slice(1), { cwd: this.worktree, detached: true, stdio: 'inherit', env: { ...process.env, ...options.env, ...(kind === 'native-build' ? { CARGO_TARGET_DIR: resolve(this.worktree, options.output ?? options.existingOutput) } : {}) } });
      const completion = new Promise((res, rej) => { child.once('error', rej); child.once('exit', (code, signal) => res({ code, signal })); });
      await this.transaction(s => {
        const task = this.active(s, id, thread);
        const identity = processIdentity(child.pid);
        if (identity) task.processes.push({ ...identity, operation, role: 'child', args });
        s.leases[key].group = child.pid;
        task.pending = task.pending.filter(p => p !== operation);
      });
      const result = await completion;
      return { ...result, operation, group: child.pid, requiresAudit: kind !== 'publish' };
    } finally {
      await this.transaction(s => {
        const task = this.task(s, id, thread);
        if (task.pending.includes(operation)) fail('incomplete process launch; ownership retained for audit');
        const lease = s.leases[key];
        const members = groupMembers(lease.group);
        for (const member of members) if (!task.processes.some(p => p.operation === operation && p.pid === member.pid)) {
          task.processes.push({ ...member, operation, role: 'descendant' });
        }
        task.processes = task.processes.filter(p => p.operation !== operation || p.role !== 'guard');
        lease.phase = 'awaiting-audit';
        // An exited launcher does not prove that its descendants exited.
        if (kind === 'publish' && !members.length) {
          task.processes = task.processes.filter(p => p.operation !== operation);
          delete s.leases[key];
        }
        this.event(s, task, `${kind}-ended`, { operation, group: lease.group, requiresAudit: !!s.leases[key] });
      });
    }
  }
  auditRun(id, thread, audit) {
    return this.transaction(s => {
      const task = this.task(s, id);
      if (thread !== task.thread) this.coordinator(s, thread);
      const entry = Object.entries(s.leases).find(([, l]) => l.task === id && l.operation === audit.operation);
      if (!entry) fail('unknown operation or already released');
      const [key, lease] = entry;
      if (audit.taskId !== id || audit.worktree !== task.worktree || audit.auditorThreadId !== thread || !Array.isArray(audit.processes)) fail('matching explicit process audit required');
      if (audit.newBootBoundary) {
        this.coordinator(s, thread);
        const boundary = audit.newBootBoundary;
        const previousBoot = lease.guard?.bootId;
        const matchingLease = (key === 'native-build' && lease.kind === 'native-build') ||
          (key === `process:${audit.operation}` && lease.kind === 'process');
        if (!matchingLease || lease.phase !== 'awaiting-audit' || task.pending.length ||
            !bootId || !previousBoot || previousBoot === bootId || boundary.fromBootId !== previousBoot || boundary.toBootId !== bootId ||
            boundary.host !== hostname() || lease.guard.host !== boundary.host || boundary.sameHostRestartConfirmed !== true ||
            typeof boundary.confirmationReference !== 'string' || !boundary.confirmationReference.trim() || audit.noUnrecordedDescendants !== false) {
          fail('confirmed same-host restart boundary required');
        }
        const retired = task.processes.filter(p => p.operation === audit.operation);
        const members = [lease.guard, ...retired, ...audit.processes];
        if (!retired.length || members.some(p => !Number.isSafeInteger(p.pid) || p.pid <= 0 || !p.started || p.bootId !== previousBoot || p.host !== boundary.host) ||
            [...retired, lease.guard].some(p => !audit.processes.some(a => a.pid === p.pid && a.started === p.started && a.bootId === p.bootId && a.host === p.host)) ||
            audit.processes.some(a => ![...retired, lease.guard].some(p => a.pid === p.pid && a.started === p.started))) {
          fail('matching recorded prior-boot identities required');
        }
        task.processes = task.processes.filter(p => p.operation !== audit.operation);
        delete s.leases[key];
        return this.event(s, task, 'process-audit-released', {
          operation: audit.operation, group: lease.group, audit, held: false,
          releaseBasis: 'confirmed-same-host-restart', historicalLifetime: 'unknown', retiredProcesses: retired, guard: lease.guard,
        });
      }
      if (lease.phase === 'running' && this.processState(s, lease.guard) !== 'dead') fail('launcher still live or unknown');
      for (const member of [...groupMembers(lease.group), ...audit.processes]) {
        if (!Number.isSafeInteger(member.pid) || member.pid <= 0 || !processHostMatches(member, { host: hostname(), bootId }, s.repos[this.repo]?.processHosts) || !member.started) fail('unknown audited process identity');
        if (!task.processes.some(p => p.operation === audit.operation && p.pid === member.pid && p.started === member.started)) {
          task.processes.push({ ...member, operation: audit.operation, role: 'descendant' });
        }
      }
      const held = task.pending.length || task.processes.some(p => p.operation === audit.operation && this.processState(s, p) !== 'dead') || audit.noUnrecordedDescendants !== true;
      if (!held) {
        task.processes = task.processes.filter(p => p.operation !== audit.operation);
        delete s.leases[key];
      }
      return this.event(s, task, held ? 'process-audit-held' : 'process-audit-released', { operation: audit.operation, group: lease.group, audit, held: !!held });
    });
  }
  async publish(id, thread, verify, { remote = 'origin', branch = 'feat(example)/t3-code-ios' } = {}) {
    // A separate child keeps the turn for fetch, rebase, verification and ordinary push.
    return this.run(id, thread, 'publish', [process.execPath, join(import.meta.dir, 'publish.mjs'), remote, branch, ...verify]);
  }
  stop(id, thread, pid) {
    return this.transaction(s => {
      const task = this.task(s, id, thread);
      const receipt = task.processes.find(p => p.pid === pid && p.role !== 'guard') ?? fail('PID was not launched by this task');
      const state = this.processState(s, receipt);
      if (state === 'unknown') fail('unknown process identity');
      if (state === 'live') process.kill(pid, 'SIGTERM');
      return this.event(s, task, 'process-stop', { receipt, state });
    });
  }
  preserve(id, thread, evidence) {
    return this.transaction(s => {
      const task = this.active(s, id, thread);
      this.clean(task);
      for (const file of evidence) {
        const path = resolve(task.worktree, file);
        noLinks(path);
        if (!lstatSync(path).isFile() || task.outputs.some(o => inside(o.path, path))) fail('evidence must be a file outside disposable output');
        const dest = join(this.root, 'evidence', id, `${randomUUID()}-${basename(path)}`);
        mkdirSync(dirname(dest), { recursive: true });
        cpSync(path, dest, { errorOnExist: true, force: false });
        task.evidence.push(dest);
      }
      const commit = command(['git', 'rev-parse', 'HEAD'], task.worktree);
      const ref = `refs/t3-coordination/${id}`;
      command(['git', 'update-ref', ref, commit], task.worktree);
      task.checkpoint = { commit, ref };
      return this.event(s, task, 'preserved', { checkpoint: task.checkpoint, evidence: task.evidence });
    });
  }
  checkEvidence(task) {
    for (const path of task.evidence) {
      noLinks(path);
      if (!lstatSync(path).isFile()) fail('preserved evidence missing');
    }
  }
  resource(id, thread, kind, value, release = false) {
    return this.transaction(s => {
      const task = this.active(s, id, thread);
      if (!['port', 'storage'].includes(kind) || !value) fail('port or storage resource identifier required');
      const key = `${kind}:${value}`;
      if (release) {
        if (s.leases[key]?.task !== id) fail('resource belongs to another task or unknown owner');
        if (task.pending.length || task.processes.some(p => this.processState(s, p) !== 'dead')) fail('stop owned processes before releasing resource');
        delete s.leases[key];
      } else {
        if (s.leases[key]) fail('resource already owned');
        s.leases[key] = { task: id, thread, worktree: task.worktree, kind, value };
      }
      return this.event(s, task, release ? 'resource-released' : 'resource-claimed', { kind, value });
    });
  }
  idle(task, s) {
    if (task.pending.length) fail('incomplete process launch; audit required');
    if (task.processes.some(p => this.processState(s, p) !== 'dead')) fail('task has live or unknown processes');
    if (Object.values(s.leases).some(l => l.task === task.id)) fail('task owns a build or publishing turn');
    if (Object.values(s.devices).some(d => d.task === task.id)) fail('release devices first');
  }
  cleanup(id, thread) {
    return this.transaction(s => {
      const task = this.task(s, id, thread);
      if (task.worktree !== this.worktree) fail('cleanup must run in owned worktree');
      this.idle(task, s);
      this.clean(task);
      if (!task.checkpoint || command(['git', 'rev-parse', 'HEAD'], task.worktree) !== task.checkpoint.commit || !task.evidence.length) fail('preserve current source and evidence first');
      this.checkEvidence(task);
      if (command(['git', 'rev-parse', task.checkpoint.ref], task.worktree) !== task.checkpoint.commit) fail('source checkpoint ref changed');
      const receipts = [];
      for (const output of task.outputs.filter(o => !o.cleaned)) {
        const path = this.safePath(task, output.path);
        if (!existsSync(path)) fail('owned output disappeared; audit required');
        const st = lstatSync(path);
        if (st.dev !== output.dev || st.ino !== output.ino) fail('output identity changed');
        const inspect = dir => {
          for (const entry of readdirSync(dir, { withFileTypes: true })) {
            if (entry.name === '.git') fail('nested Git repository in output');
            if (entry.isDirectory()) inspect(join(dir, entry.name));
          }
        };
        inspect(path);
        const kib = Number(command(['du', '-sk', path], task.worktree).split(/\s/)[0]);
        const freeBefore = this.freeBytes(dirname(path));
        rmSync(path, { recursive: true });
        output.cleaned = true;
        receipts.push({ path, kibRemoved: kib, freeBefore, freeAfter: this.freeBytes(dirname(path)) });
      }
      return this.event(s, task, 'cleaned', { receipts, retained: task.evidence, checkpoint: task.checkpoint });
    });
  }
  recover(id, thread, proof) {
    return this.transaction(s => {
      this.coordinator(s, thread);
      const task = this.task(s, id);
      const terminal = terminalProof(proof, task);
      if (terminal.kind === 'native-investigation' &&
          (Object.values(s.leases).some(l => l.task === id) || Object.values(s.devices).some(d => d.task === id))) {
        fail('native investigation owns resources; T3-bound recovery required');
      }
      if (task.pending.length) fail('incomplete process launch; audit required');
      if (task.processes.some(p => this.processState(s, p) !== 'dead')) fail('task has live or unknown processes');
      for (const [key, lease] of Object.entries(s.leases)) if (lease.task === id) {
        if (lease.guard && this.processState(s, lease.guard) !== 'dead') fail('lease process live or unknown');
        if (!lease.operation) delete s.leases[key];
      }
      task.status = 'terminal';
      task.terminal = terminal;
      return this.event(s, task, 'terminal-recovered', { terminal });
    });
  }
  retire(id, thread, proof) {
    return this.transaction(s => {
      this.coordinator(s, thread);
      const task = this.task(s, id);
      terminalProof(proof, task);
      if (task.status !== 'terminal' || task.worktree === this.worktree) fail('recover terminal worker first; cannot retire caller');
      this.idle(task, s);
      this.clean(task);
      if (!task.checkpoint || !task.evidence.length || task.outputs.some(o => !o.cleaned)) fail('source, evidence and cleanup receipts required');
      this.checkEvidence(task);
      if (command(['git', 'rev-parse', 'HEAD'], task.worktree) !== command(['git', 'rev-parse', task.checkpoint.ref], this.worktree)) fail('checkpoint differs from worktree HEAD');
      const ignored = command(['git', 'ls-files', '--others', '--ignored', '--exclude-standard'], task.worktree);
      if (ignored) fail('ignored files remain; preserve or explicitly clean them first');
      command(['git', 'worktree', 'remove', task.worktree], this.worktree);
      task.status = 'retired';
      return this.event(s, task, 'retired', { checkpoint: task.checkpoint, evidence: task.evidence });
    });
  }
  enroll(thread, device, inventory, created = false, taskId) {
    return this.transaction(s => {
      this.coordinator(s, thread);
      const task = taskId === undefined ? null : this.task(s, taskId);
      if (task) {
        if (task.status !== 'active') fail('inactive task');
        if (created !== true) fail('direct assignment requires a newly created simulator');
        if (Object.values(s.devices).some(d => d.task === task.id)) fail('task already owns a simulator');
        const matches = Array.isArray(inventory) ? inventory.filter(d => d?.host === device.host && d?.udid === device.udid) : [];
        if (![device.host, device.udid, device.runtime].every(v => typeof v === 'string' && v.trim()) ||
            matches.length !== 1 || matches[0].class !== device.class || matches[0].runtime !== device.runtime) fail('fresh inventory must match the exact simulator host, UDID, class and runtime');
      }
      const found = inventory.find(d => d.host === device.host && d.udid === device.udid);
      if (!found || found.state !== 'Shutdown' || !['phone', 'ipad'].includes(device.class) || !device.runtime) fail('audited shutdown simulator and compatible class/runtime required');
      const key = `${device.host}:${device.udid}`;
      if (s.devices[key]) fail('device already registered');
      s.devices[key] = { ...device, created, status: 'idle', repo: this.repo, enrolledBy: thread };
      if (task) {
        const assigned = s.devices[key];
        assigned.status = 'owned'; assigned.task = task.id; assigned.thread = task.thread; assigned.worktree = task.worktree;
        this.event(s, task, 'device-claimed', { device: assigned, grantedBy: thread });
      } else this.limitIdle(s, device.host, device.class);
      return s.devices[key];
    });
  }
  limitIdle(s, host, deviceClass) {
    const idle = Object.values(s.devices).filter(d => d.host === host && d.class === deviceClass && d.created && d.status === 'idle');
    for (const d of idle.slice(1)) d.status = 'retiring';
  }
  claimDevice(id, thread, host, deviceClass, runtime) {
    return this.transaction(s => {
      const task = this.active(s, id, thread);
      if (Object.values(s.devices).some(d => d.task === id)) fail('task already owns a simulator');
      const device = Object.values(s.devices).find(d => d.host === host && d.class === deviceClass && d.runtime === runtime && d.repo === this.repo && d.status === 'idle');
      if (!device) fail('no released compatible simulator; coordinator must audit an unused shutdown device or create and enroll one');
      device.status = 'owned'; device.task = id; device.thread = thread; device.worktree = task.worktree;
      this.event(s, task, 'device-claimed', { device });
      return device;
    });
  }
  releaseDevice(id, thread, host, udid, inventory) {
    return this.transaction(s => {
      const task = this.task(s, id, thread);
      const device = s.devices[`${host}:${udid}`];
      if (device?.task !== id) fail('device belongs to another task or unknown owner');
      if (task.processes.some(p => this.processState(s, p) !== 'dead')) fail('stop task processes before device release');
      if (!inventory.some(d => d.host === host && d.udid === udid && d.state === 'Shutdown')) fail('shut down owned device and supply fresh inventory before release');
      delete device.task; delete device.thread; delete device.worktree;
      device.status = 'idle';
      this.limitIdle(s, host, device.class);
      return this.event(s, task, 'device-released', { device });
    });
  }
  designateDeviceSurplus(thread, proof) {
    return this.transaction(s => {
      this.coordinator(s, thread);
      const age = Date.now() - proof?.observedAtMs;
      if (!Number.isFinite(age) || age < 0 || age > 60000 ||
          typeof proof.reason !== 'string' || !proof.reason.trim() || proof.reason.length > 512) fail('fresh surplus reason and observation required');
      if (!Array.isArray(proof.retainedDevices) || !proof.retainedDevices.length || proof.retainedDevices.length > 2 ||
          !Array.isArray(proof.inventory)) fail('retained pool and Shutdown inventory required');
      const entries = [{ expected: proof.expectedDevice, identity: proof.identity }, ...proof.retainedDevices];
      const records = [], keys = new Set(), paths = new Set(), identities = new Set();
      let parent;
      for (const { expected, identity } of entries) {
        const key = `${expected?.host}:${expected?.udid}`, device = s.devices[key];
        if (keys.has(key) || !device?.created || device.repo !== this.repo || device.status !== 'idle' || device.task || device.thread || device.worktree) fail('distinct released task-created devices from this repository required');
        keys.add(key);
        if (!expected || Object.keys(device).sort().join('\0') !== Object.keys(expected).sort().join('\0') ||
            Object.keys(device).some(k => device[k] !== expected[k])) fail('device record changed');
        if (typeof identity?.path !== 'string' || !isAbsolute(identity.path) || basename(identity.path) !== device.udid ||
            basename(dirname(identity.path)) !== 'Devices') fail('exact local simulator directory required');
        noLinks(identity.path);
        const stat = lstatSync(identity.path), dir = dirname(identity.path);
        if (!stat.isDirectory() || stat.dev !== identity.dev || stat.ino !== identity.ino || (parent && dir !== parent)) fail('local simulator directory identity changed');
        const path = resolve(identity.path), inode = `${stat.dev}:${stat.ino}`;
        if (paths.has(path) || identities.has(inode)) fail('distinct local simulator directories required');
        paths.add(path); identities.add(inode);
        parent = dir;
        const inventory = proof.inventory.filter(d => d?.host === device.host && d?.udid === device.udid);
        if (inventory.length !== 1 || inventory[0].state !== 'Shutdown') fail('fresh exact Shutdown inventory required');
        records.push(device);
      }
      const [device, ...retained] = records;
      if (!retained.some(d => d.class === device.class && d.runtime === device.runtime)) fail('retain a compatible released pool device');
      if (!Array.isArray(proof.evidence) || !proof.evidence.length || proof.evidence.length > 4) fail('preserved surplus evidence required');
      for (const path of proof.evidence) {
        if (typeof path !== 'string' || !inside(join(this.root, 'evidence'), path) ||
            !Object.values(s.tasks).some(t => t.repo === this.repo && t.evidence.includes(path))) fail('surplus evidence is not preserved by this repository');
        noLinks(path);
        if (!lstatSync(path).isFile()) fail('surplus evidence must be a regular file');
      }
      const before = { ...device };
      device.status = 'retiring';
      return this.event(s, null, 'device-surplus-designated', { before, device: { ...device },
        retainedDevices: retained.map(d => ({ ...d })), identities: entries.map(e => e.identity),
        evidence: proof.evidence, reason: proof.reason });
    });
  }
  retireDevice(thread, host, udid, inventory) {
    return this.transaction(s => {
      this.coordinator(s, thread);
      const key = `${host}:${udid}`, device = s.devices[key];
      if (!device?.created || device.status !== 'retiring' || device.task) fail('only surplus released task-created simulators may retire');
      if (inventory.some(d => d.host === host && d.udid === udid)) fail('delete the surplus device and supply fresh inventory first');
      delete s.devices[key];
      return this.event(s, null, 'device-retired', { device });
    });
  }
}
