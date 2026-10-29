// Writes @exact/oauth's app cases for one profile, or all, from
// `cases.template.contract` (LLP 1146 D6).
//
//   bun cases.mjs <out-dir> [--choose] [<profile>…]
//
// `--choose` adds the step that picks the profile in an app that offers
// several (`apps/oauth-fixture`'s `profile` select). Each profile's file is
// `<out-dir>/oauth-<profile>.test.contract`.
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
import process from 'node:process';
import { PROFILES } from './fixture.ts';

export function casesFor(profile, choose, file = resolve(import.meta.dirname, 'cases.template.contract')) {
  const template = readFileSync(file, 'utf8');
  const out = [];
  let keep = true;
  for (const line of template.split('\n')) {
    const at = /^#if (.+)$/.exec(line);
    if (at) { keep = at[1].split(/[\s,]+/).includes(profile); continue; }
    if (line === '#end') { keep = true; continue; }
    if (!keep || line.startsWith('//')) continue;
    if (line.trim() === '{{CHOOSE}}') {
      if (choose) out.push(`  type "profile" "${profile}"`);
      continue;
    }
    out.push(line.replaceAll('{{PROFILE}}', profile));
  }
  return out.join('\n').replace(/^\n+/, '');
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const choose = args.includes('--choose');
  const [dir, ...names] = args.filter((a) => a !== '--choose');
  if (!dir) throw new Error('usage: bun cases.mjs <out-dir> [--choose] [<profile>…]');
  const profiles = names.length ? names : Object.keys(PROFILES);
  for (const p of profiles) if (!PROFILES[p]) throw new Error(`no profile ${p}; the profiles are ${Object.keys(PROFILES).join(', ')}`);
  mkdirSync(dir, { recursive: true });
  for (const p of profiles) writeFileSync(resolve(dir, `oauth-${p}.test.contract`), casesFor(p, choose));
  console.log(`wrote ${profiles.length} case files to ${dir}`);
}
