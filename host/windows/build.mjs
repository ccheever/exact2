#!/usr/bin/env bun
// The ordinary bake, packaged as a native executable, GPU DLL and baked assets.
//
//   bun host/windows/build.mjs <app> [--run] [--release] [--out <dir>]
//
// `--run` opens it. From SSH, which has no display a window can show on (docs/testing.md, "Windows: the
// NucBox"), it opens on the desktop of the user signed in at the console, through a one-off scheduled task, as
// host/windows/reference/draw.mjs opens WinUI's, and returns once the window is up.
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { delimiter, dirname, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { bakeTarget, buildBake, copyShaders, developmentBuildEnv, resolveApp, windowsFile } from '../../scripts/app.mjs';

const quote = (s) => `'${String(s).replaceAll("'", "''")}'`;
const powershell = (command) => spawnSync('powershell', ['-NoProfile', '-NonInteractive', '-Command', command], { encoding: 'utf8', windowsHide: true });

/** The directory of the Windows SDK's newest x64 `fxc.exe`, which ink's Direct3D 11 shaders compile with
 * (host/ink/build.rs) and a shell's PATH outside a Developer Command Prompt lacks; null when PATH has one or there is
 * no SDK. Without it the shaders are empty and an ink window refuses to open. */
function sdkFxc() {
  if (String(process.env.PATH ?? '').split(delimiter).some((dir) => dir && existsSync(resolve(dir, 'fxc.exe')))) return null;
  const bin = resolve(process.env['ProgramFiles(x86)'] ?? 'C:\\Program Files (x86)', 'Windows Kits', '10', 'bin');
  let versions = []; try { versions = readdirSync(bin).filter((v) => /^\d+(\.\d+)+$/.test(v)); } catch {}
  const newest = versions.sort((a, b) => a.localeCompare(b, 'en', { numeric: true })).reverse().find((v) => existsSync(resolve(bin, v, 'x64', 'fxc.exe')));
  return newest ? resolve(bin, newest, 'x64') : null;
}

/** Whether this process has no desktop of its own: an SSH session's windows are never shown. */
export const remote = () => !!process.env.SSH_CONNECTION;

/** The process `executable` that `--run` (or a window capture) last opened from its directory and that is still that
 * executable's: its PID, recorded in `<dir>/opened.json` when it opened, else null. */
function opened(executable) {
  let pid;
  try { pid = JSON.parse(readFileSync(resolve(dirname(executable), 'opened.json'), 'utf8')).pid; } catch { return null; }
  const path = powershell(`(Get-Process -Id ${Number(pid)} -ErrorAction SilentlyContinue).Path`).stdout.trim();
  return path.toLowerCase() === executable.toLowerCase() ? pid : null;
}

/** Ends the copy `--run` opened from `executable`'s directory, so the build can replace its files. */
export function closeOpened(executable) {
  const pid = opened(executable);
  if (pid) powershell(`Stop-Process -Id ${pid} -Force; Wait-Process -Id ${pid} -Timeout 10 -ErrorAction SilentlyContinue`);
  rmSync(resolve(dirname(executable), 'opened.json'), { force: true });
}

/** Opens `executable` on the console session's desktop, from its directory, and waits (at most 30 s) for its window.
 * With `shot`, the window's frame as the screen shows it (DWM's extended frame bounds: the title bar, Mica and the
 * content, no shadow) is written there as a PNG after `settle` ms, and the app is closed. Otherwise it keeps running
 * and its PID is recorded beside it (`closeOpened`). The PowerShell that does it runs headless (`conhost --headless`),
 * so no console window shows. The app's stderr goes to `opened.log` beside it. Returns `{ pid }` or `{ error }`. */
export async function openOnDesktop(executable, { shot = null, settle = 1500 } = {}) {
  const dir = dirname(executable), stamp = `${process.pid}-${Date.now()}`, result = resolve(tmpdir(), `exact-desktop-${stamp}.json`), log = resolve(dir, 'opened.log');
  const script = `$ErrorActionPreference = 'Stop'
try {
  Add-Type -AssemblyName System.Drawing
  Add-Type -TypeDefinition @'
using System; using System.Runtime.InteropServices;
public static class ExactDesktop {
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr window);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr window, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr window, int attribute, out Rect rect, int size);
}
'@
  [void][ExactDesktop]::SetProcessDPIAware()
  $p = Start-Process -FilePath ${quote(executable)} -WorkingDirectory ${quote(dir)} -RedirectStandardError ${quote(log)} -PassThru
  $deadline = (Get-Date).AddSeconds(30)
  while ($p.MainWindowHandle -eq 0 -and -not $p.HasExited -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 100; $p.Refresh() }
  if ($p.MainWindowHandle -eq 0) { throw "no window in 30 s$(if ($p.HasExited) { " (it exited: $((Get-Content ${quote(log)} -Tail 2) -join ' '))" })" }
  $out = @{ pid = $p.Id }
  ${shot ? `$w = $p.MainWindowHandle
  [void][ExactDesktop]::SetWindowPos($w, [IntPtr](-1), 0, 0, 0, 0, 3)
  $out.foreground = [ExactDesktop]::SetForegroundWindow($w)
  Start-Sleep -Milliseconds ${Number(settle)}
  $r = New-Object ExactDesktop+Rect
  [void][ExactDesktop]::DwmGetWindowAttribute($w, 9, [ref]$r, 16)
  $bitmap = New-Object System.Drawing.Bitmap ($r.Right - $r.Left), ($r.Bottom - $r.Top)
  $g = [System.Drawing.Graphics]::FromImage($bitmap)
  $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bitmap.Size)
  $bitmap.Save(${quote(shot)}, [System.Drawing.Imaging.ImageFormat]::Png)
  $out.size = @($bitmap.Width, $bitmap.Height)
  [void]$p.CloseMainWindow()
  if (-not $p.WaitForExit(5000)) { $p.Kill() }` : ''}
  $out | ConvertTo-Json -Compress | Set-Content -Encoding UTF8 ${quote(result)}
} catch { @{ error = "$_" } | ConvertTo-Json -Compress | Set-Content -Encoding UTF8 ${quote(result)} }`;
  const encoded = Buffer.from(script, 'utf16le').toString('base64'), task = `Exact-open-${stamp}`;
  const registered = powershell([
    `$a = New-ScheduledTaskAction -Execute 'conhost.exe' -Argument ${quote(`--headless powershell.exe -NoProfile -NonInteractive -EncodedCommand ${encoded}`)}`,
    `$p = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive`,
    `Register-ScheduledTask -TaskName ${quote(task)} -Action $a -Principal $p -Force | Out-Null`,
    `Start-ScheduledTask -TaskName ${quote(task)}`,
  ].join('; '));
  try {
    if (registered.status !== 0) return { error: `the scheduled task did not start: ${registered.stderr.trim().split('\n')[0]}` };
    for (const started = Date.now(); !existsSync(result) && Date.now() - started < 45_000;) await Bun.sleep(200);
    if (!existsSync(result)) return { error: 'nothing opened in 45 s: is someone signed in at the desktop? (query user names the console session)' };
    await Bun.sleep(100); // Set-Content's last write
    const said = JSON.parse(readFileSync(result, 'utf8').replace(/^﻿/, ''));
    if (said.pid && !shot) writeFileSync(resolve(dir, 'opened.json'), JSON.stringify({ pid: said.pid }) + '\n');
    return said;
  } finally {
    powershell(`Unregister-ScheduledTask -TaskName ${quote(task)} -Confirm:$false -ErrorAction SilentlyContinue`);
    rmSync(result, { force: true });
  }
}

if (import.meta.main) {
  if (process.platform !== 'win32') throw new Error('build the Windows host on Windows with the MSVC Rust target');
  const args = process.argv.slice(2);
  const outAt = args.indexOf('--out');
  if (outAt >= 0 && !args[outAt + 1]) throw new Error('--out needs a directory');
  const app = resolveApp(args.find((arg, i) => !arg.startsWith('--') && (outAt < 0 || i !== outAt + 1)));
  const fxc = sdkFxc();
  if (fxc) process.env.PATH = `${fxc}${delimiter}${process.env.PATH}`;
  else if (app.manifest?.host?.windows?.painter === 'ink' && !String(process.env.PATH).split(delimiter).some((dir) => dir && existsSync(resolve(dir, 'fxc.exe'))))
    console.error('warning: no fxc.exe on PATH or in the Windows SDK: the window cannot open with host.windows.painter "ink" (install the Windows SDK; headless drives paint on the CPU and still work)');
  const target = bakeTarget('windows');
  const profile = args.includes('--release') ? 'release' : 'gpu-dev';
  const build = buildBake(app, 'windows', target, { profile, env: developmentBuildEnv(app) });
  // A running copy holds its executable and DLL open, so another directory lets
  // a second copy be packaged beside it.
  const output = outAt >= 0 ? resolve(args[outAt + 1]) : resolve(app.dir, 'dist-windows');
  mkdirSync(output, {recursive:true});
  const exe = build.products.find((product) => product.path.endsWith(`${app.crate('windows')}.exe`));
  if (!exe) throw new Error('Windows bake produced no executable');
  const executable = resolve(output, windowsFile(app, exe.path));
  // The copy the last --run opened here is this build's to replace.
  closeOpened(executable);
  for (const product of build.products) {
    if (!/\.(exe|dll)$/.test(product.path)) continue;
    const destination = resolve(output, windowsFile(app, product.path));
    try { copyFileSync(product.path, destination); }
    catch (error) {
      if (error.code !== 'EBUSY' && error.code !== 'EPERM') throw error;
      throw new Error(`${destination} is in use, most likely by a running copy of the app: quit it, or package into another directory with --out <dir> (run that copy under the agent with EXACT_WINDOWS_BIN)`);
    }
  }
  for (const name of ['assets','deck']) {
    const source = resolve(app.dir, name);
    if (existsSync(source)) cpSync(source, resolve(output, name), {recursive:true});
  }
  copyShaders(app, resolve(output, 'shaders'), {replace:true});
  writeFileSync(resolve(output, 'compat.json'), JSON.stringify(build.compat, null, 2) + '\n');
  console.log(`Windows: ${executable}`);
  if (args.includes('--run') && remote()) {
    const opened = await openOnDesktop(executable);
    if (opened.error) { console.error(`--run: ${opened.error}`); process.exitCode = 1; }
    else console.log(`opened on the desktop of the console session (pid ${opened.pid}; SSH shows no window); the next build here closes it`);
  } else if (args.includes('--run')) {
    const result = spawnSync(executable, [], {cwd:output, env:process.env, stdio:'inherit'});
    process.exitCode = result.status ?? 1;
  }
}
