// Distinct (id, message) diagnostics per session, from JSON diagnostics and text output, with how many agents saw each.
import fs from 'node:fs';
const across = {};
for (const file of process.argv.slice(2)) {
  const raw = fs.readFileSync(file, 'utf8').replaceAll('\\\\', '\\').replaceAll('\\"', '"').replaceAll('\\"', '"');
  const seen = new Set();
  let i = 0;
  while ((i = raw.indexOf('"id":"lower-', i)) >= 0) {
    const idEnd = raw.indexOf('"', i + 6);
    const id = raw.slice(i + 6, idEnd);
    const mi = raw.indexOf('"message":"', idEnd);
    const msg = mi > 0 && mi - idEnd < 60 ? raw.slice(mi + 11, mi + 11 + 140).split('","')[0] : '';
    seen.add(id + ' :: ' + msg.replace(/\d+/g, 'N'));
    i = idEnd;
  }
  for (const k of seen) (across[k] ??= []).push(file.split('/').slice(-3, -2)[0].replace('exact-', ''));
}
for (const [k, v] of Object.entries(across).sort((a, b) => b[1].length - a[1].length)) console.log(v.join(','), '|', k);
