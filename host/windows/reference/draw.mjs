// The WinUI reference's drawing (LLP 1115.004 D13): builds Reference.csproj at
// the pinned Windows App SDK and runs it once per scheme of specimens.json,
// writing `<out>/<scheme>/<id>.png` and `geometry.json`.
//
//   bun host/windows/reference/draw.mjs [--out <dir>]
//
// Windows only, on the oracle's machine (docs/testing.md, "Windows: the
// NucBox"), with the .NET 9 SDK. WinUI draws nothing from an SSH session, so
// each run is a one-off scheduled task in the signed-in desktop session; the
// user must be signed in there. The default out directory is
// host/windows/reference/build/oracle.
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync } from 'node:fs';
import { join, resolve } from 'node:path';

const here = import.meta.dir;
const args = process.argv.slice(2);
const option = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : null);
const out = resolve(option('--out') ?? join(here, 'build/oracle'));
const specimens = join(here, 'specimens.json');
/** How long one scheme's run may take. */
const LIMIT_MS = 120_000;

if (process.platform !== 'win32') throw new Error('the WinUI reference draws on Windows (docs/testing.md, "Windows: the NucBox")');

function run(command, argv) {
  const r = spawnSync(command, argv, { cwd: here, encoding: 'utf8' });
  if (r.error) throw r.error;
  if (r.status !== 0) throw new Error(`${command} ${argv.join(' ')} failed:\n${r.stdout}${r.stderr}`);
  return r.stdout;
}

const quote = (s) => `'${s.replaceAll("'", "''")}'`;

// The lock file holds every package at the pinned SDK's versions; a restore that would move one fails.
const locked = existsSync(join(here, 'packages.lock.json')) ? ['--locked-mode'] : [];
const exe = join(here, 'bin/Release/net9.0-windows10.0.26100.0/win-x64/Reference.exe');
// A run that outlived its limit holds the executable, and the next build cannot replace it.
const stopLeftover = () => run('powershell', ['-NoProfile', '-Command',
  `Get-Process Reference -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq ${quote(exe)} } | Stop-Process -Force; exit 0`]);
stopLeftover();
run('dotnet', ['restore', ...locked]);
run('dotnet', ['build', '--no-restore', '-c', 'Release']);

const { schemes } = JSON.parse(readFileSync(specimens, 'utf8'));
for (const scheme of schemes) {
  const dir = join(out, scheme);
  rmSync(dir, { recursive: true, force: true });
  const task = `ExactWinuiReference-${scheme}`;
  const argv = `--scheme ${scheme} --specimens "${specimens}" --out "${out}"`;
  run('powershell', ['-NoProfile', '-Command', [
    `$a = New-ScheduledTaskAction -Execute ${quote(exe)} -Argument ${quote(argv)}`,
    `$p = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive`,
    `Register-ScheduledTask -TaskName ${quote(task)} -Action $a -Principal $p -Force | Out-Null`,
    `Start-ScheduledTask -TaskName ${quote(task)}`,
  ].join('; ')]);
  const started = Date.now();
  while (!existsSync(join(dir, 'done.txt')) && !existsSync(join(dir, 'error.txt')) && Date.now() - started < LIMIT_MS) {
    await Bun.sleep(250);
  }
  run('powershell', ['-NoProfile', '-Command', `Unregister-ScheduledTask -TaskName ${quote(task)} -Confirm:$false`]);
  stopLeftover();
  if (existsSync(join(dir, 'error.txt'))) throw new Error(`${scheme}: ${readFileSync(join(dir, 'error.txt'), 'utf8')}`);
  if (!existsSync(join(dir, 'done.txt'))) throw new Error(`${scheme}: no result in ${LIMIT_MS / 1000} s; is the user signed in at the desktop?`);
  const geometry = JSON.parse(readFileSync(join(dir, 'geometry.json'), 'utf8'));
  console.log(`${scheme}: ${Object.keys(geometry.specimens).length} specimens at scale ${geometry.scale} → ${dir}`);
}
