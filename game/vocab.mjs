// The game vocabulary is rustdoc's public API, with defaults evaluated by Rust.
import {spawnSync} from 'node:child_process';
import {readFileSync, mkdirSync, writeFileSync} from 'node:fs';
import {resolve} from 'node:path';

const root = resolve(import.meta.dir, '..');
const game = resolve(root, 'game');
const help = 'Usage: bun exact.mjs game vocab [<Component|resource|World verb|Type::method>] [--json]\nNo name lists the public engine vocabulary. Generated from rustdoc and Rust Default implementations.';
function run(cmd, args, options = {}) {
  const env = {...process.env, ...options.env, CARGO_TARGET_DIR:resolve(game, 'target/vocab')};
  const result = spawnSync(cmd, args, {cwd: game, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, ...options, env});
  if (result.error || result.status !== 0) {
    const diagnostics = result.stdout.split('\n').flatMap(line => {
      try { const m = JSON.parse(line); return m.message?.rendered ? [m.message.rendered] : []; } catch { return []; }
    }).join('');
    throw new Error(result.error?.message ?? (diagnostics + result.stderr || `${cmd} failed`));
  }
  return result.stdout;
}
const itemIds = item => item.inner.struct?.impls ?? item.inner.enum?.impls ?? [];
const traitName = impl => impl?.trait?.path?.split('::').at(-1);

/** Public, reachable declarations and inherent methods, including re-exported names. */
export function vocabulary(doc) {
  const index = doc.index, paths = new Map(), visited = new Set();
  function walk(id, prefix, explicit = false) {
    const item = index[id];
    if (!item || item.crate_id !== 0 || visited.has(`${id}:${prefix}`)) return;
    visited.add(`${id}:${prefix}`);
    if (item.inner.use) {
      const use = item.inner.use;
      if (use.is_glob) for (const child of index[use.id]?.inner.module?.items ?? []) walk(child, prefix);
      else walk(use.id, `${prefix}::${use.name}`, true);
      return;
    }
    const path = explicit ? prefix : `${prefix}::${item.name}`;
    if (!paths.has(String(id)) || path.length < paths.get(String(id)).length) paths.set(String(id), path);
    for (const child of item.inner.module?.items ?? []) {
      if (index[child]?.visibility === 'public') walk(child, path);
    }
  }
  for (const id of index[doc.root].inner.module.items) if (index[id]?.visibility === 'public') walk(id, 'exact_game');
  const entries = [];
  for (const [id, path] of paths) {
    const item = index[id];
    if (item.inner.type_alias) { entries.push({name:item.name, path, item, kind:'alias', replacement:type(item.inner.type_alias.type)}); continue; }
    if (!item.inner.struct && !item.inner.enum) continue;
    const impls = itemIds(item).map(id => index[id]?.inner.impl).filter(Boolean);
    const policy = impls.find(i => ['Component', 'Resource'].includes(traitName(i)));
    const defaults = impls.some(i => traitName(i) === 'Default');
    if (policy || !item.inner.struct?.generics.params.length) entries.push({
      name: item.name, path, item, kind: traitName(policy) ?? 'type', defaults,
      presentation: policy?.items.some(id => index[id]?.name === 'PRESENTATION' && index[id].inner.assoc_const?.value === 'true') ?? false,
    });
    for (const impl of impls.filter(i => !i.trait)) for (const id of impl.items) {
      const method = index[id];
      if (method?.visibility === 'public' && method.inner.function) entries.push({
        name: `${item.name}::${method.name}`, short: method.name, path: `${path}::${method.name}`,
        item: method, kind: item.name === 'World' ? 'World verb' : 'method',
      });
    }
  }
  return entries.sort((a,b) => a.name.localeCompare(b.name));
}
function type(t) {
  if (!t) return '()';
  if (t.primitive || t.generic) return t.primitive ?? t.generic;
  if (t.resolved_path) {
    const p = t.resolved_path, args = p.args?.angle_bracketed?.args.map(a => a.type ? type(a.type) : a.lifetime ?? '_');
    return p.path + (args?.length ? `<${args.join(', ')}>` : '');
  }
  if (t.borrowed_ref) return `&${t.borrowed_ref.is_mutable ? 'mut ' : ''}${type(t.borrowed_ref.type)}`;
  if (t.tuple) return `(${t.tuple.map(type).join(', ')})`;
  if (t.array) return `[${type(t.array.type)}; ${t.array.len}]`;
  if (t.slice) return `[${type(t.slice)}]`;
  if (t.impl_trait) return 'impl ' + t.impl_trait.map(b => b.trait_bound?.trait.path ?? '?').join(' + ');
  return JSON.stringify(t);
}
function fields(item, index) {
  const from = (ids, variant) => ids.filter(id => id != null).map((id, i) => ({
    name: index[id].name ?? String(i), variant, type: type(index[id].inner.struct_field), docs: index[id].docs ?? '',
  }));
  const kind = item.inner.struct?.kind;
  if (kind) return from(kind.plain?.fields ?? kind.tuple ?? []);
  return (item.inner.enum?.variants ?? []).flatMap(id => {
    const v = index[id], k = v.inner.variant.kind;
    return from(k.struct?.fields ?? k.tuple ?? [], v.name);
  });
}
function fieldDefault(entry, field) {
  if (!entry.defaults) return 'no Default implementation';
  const value = field.variant ? entry.default?.[field.variant] : entry.default;
  if (value === undefined) return `default uses another variant`;
  return JSON.stringify(value?.[field.name] ?? (Array.isArray(value) ? value[Number(field.name)] : value));
}
function recipes(entry, doc) {
  const names = [entry.short ?? entry.name, entry.name.split('::')[0]];
  return Object.values(doc.index).filter(i => i.crate_id === 0 && i.inner.module && i.docs?.includes('# Recipe:'))
    .filter(i => names.some(n => new RegExp(`\\b${n}\\b`).test(i.docs)))
    .map(i => `game/recipes/${i.name.replaceAll('_', '-')}.md`);
}

/** Build only generated artifacts; Cargo fingerprints rustdoc and the library inputs. */
export function generate() {
  run('cargo', ['rustdoc', '--manifest-path', resolve(game, 'Cargo.toml'), '-p', 'exact-game', '--locked', '--lib', '--', '-Z', 'unstable-options', '--output-format', 'json'], {env: {...process.env, RUSTC_BOOTSTRAP:'1'}});
  const metadata = JSON.parse(run('cargo', ['metadata', '--no-deps', '--format-version', '1']));
  const target = metadata.target_directory;
  const doc = JSON.parse(readFileSync(resolve(target, 'doc/exact_game.json'), 'utf8'));
  const entries = vocabulary(doc);
  // Let Rust evaluate all public component/resource defaults. No parallel table of values.
  const built = run('cargo', ['build', '-p', 'exact-game', '--locked', '--lib', '--message-format=json']);
  const artifact = built.split('\n').filter(Boolean).map(s => JSON.parse(s)).findLast(m => m.reason === 'compiler-artifact' && m.target.name === 'exact_game');
  const rlib = artifact.filenames.find(f => f.endsWith('.rlib'));
  const folder = resolve(target, 'game-vocab'); mkdirSync(folder, {recursive:true});
  const source = `fn main() {\n${entries.filter(e => ['Component','Resource'].includes(e.kind) && e.defaults).map(e =>
    `println!("{}\\t{}", ${JSON.stringify(e.name)}, exact_game::json::to_string(&${e.path}::default()).expect("default serializes"));`).join('\n')}\n}`;
  const file = resolve(folder, 'defaults.rs'), exe = resolve(folder, 'defaults');
  // rustc's dependencies can change at the same filename, so always compile the tiny evaluator.
  writeFileSync(file, source);
  run('rustc', ['--edition=2021', file, '-L', `dependency=${resolve(target, 'debug/deps')}`, '--extern', `exact_game=${rlib}`, '-o', exe]);
  const defaults = Object.fromEntries(run(exe, []).trim().split('\n').filter(Boolean).map(line => {
    const at = line.indexOf('\t'); return [line.slice(0, at), JSON.parse(line.slice(at+1))];
  }));
  const cards = entries.map(e => {
    const {item, ...rest} = e;
    const sig = item.inner.function?.sig;
    return {...rest, docs: item.docs ?? '', fields: fields(item, doc.index),
      arguments: sig?.inputs.map(([name,t]) => ({name, type:type(t)})), returns: sig ? type(sig.output) : undefined,
      default: e.defaults ? defaults[e.name] : undefined, recipes: recipes(e, doc),
      deprecated: item.deprecation ?? undefined, source: item.span};
  });
  writeFileSync(resolve(folder, 'cards.json'), JSON.stringify(cards));
  return cards;
}

/** Resolve a declaration, qualified field or argument without a separate name registry. */
export function lookup(entries, name) {
  const exact = entries.filter(e => e.name === name || e.path === name || e.short === name || e.name === `World::${name}`);
  if (exact.length) return exact;
  return entries.flatMap(e => {
    const fields = e.fields.filter(f => f.name === name || `${e.name}.${f.variant ? f.variant + '.' : ''}${f.name}` === name);
    if (fields.length) return [{...e, fields}];
    const arguments_ = e.arguments?.filter(a => a.name === name || `${e.name}.${a.name}` === name);
    return arguments_?.length ? [{...e, arguments:arguments_}] : [];
  });
}

export function main(args) {
  if (args.includes('--help') || args.includes('-h')) return console.log(help);
  if (args.some(a => a.startsWith('-') && a !== '--json') || args.filter(a => !a.startsWith('-')).length > 1) throw new Error(help);
  const name = args.find(a => !a.startsWith('-')), entries = generate();
  if (!name) return console.log(args.includes('--json') ? JSON.stringify(entries) : entries.filter(e => e.kind !== 'method').map(e => `${e.name} — ${e.kind}`).join('\n'));
  const found = lookup(entries, name);
  if (!found.length) throw new Error(`game vocab: unknown name ${name}; run game vocab to list names`);
  if (args.includes('--json')) return console.log(JSON.stringify(found, null, 2));
  for (const e of found) {
    console.log(`${e.name} — ${e.kind}\n${e.docs}\n`);
    if (e.arguments) console.log(`Arguments: ${e.arguments.map(a => `${a.name}: ${a.type}`).join(', ')}\nReturns: ${e.returns}`);
    if (e.fields.length) console.log(e.fields.map(f => `${f.variant ? f.variant + "." : ""}${f.name}: ${f.type} — ${f.docs}. Default: ${fieldDefault(e, f)}`).join('\n'));
    if (['Component','Resource'].includes(e.kind)) console.log(`Default: ${e.defaults ? JSON.stringify(e.default) : 'no Default implementation; use its constructor'}\nSaves: ${e.presentation ? 'no; rebuilt in Game::present' : 'yes; registered simulation data'}\nStepping: ${e.docs.split('\n').filter(l => /step|sample|tick/i.test(l)).join(' ') || (e.presentation ? 'Game::present rebuilds it; no simulation step' : 'not stepped automatically; use its explicit operations in tick')}`);
    if (e.replacement) console.log(`Alias: ${e.replacement}`);
    if (e.deprecated) console.log(`Old name: ${e.name}; ${e.deprecated.note}`);
    console.log(`Recipe: ${e.recipes.join(', ') || 'game/recipes/component-values.md'}\nReference: game/engine/README.md\nSource: game/${e.source.filename}:${e.source.begin[0]}`);
  }
}
if (import.meta.main) try { main(process.argv.slice(2)); } catch (e) { console.error(e.message); process.exitCode = 1; }
