// Tests only (browser-surface part 4): the cookie import's ImportIO over Bun, so the ported reference tests run the
// clone's readers against fixture browser stores they build in a temporary directory, as the reference's tests do
// with NodeServices. The app's ImportIO is the module's (browser-import-io.ts `nativeImportIO`, T3BrowserImportIO.swift),
// whose snapshot, EPERM and fcntl answers its AppKit tests check (macos/tests/browser-profiles).
import { Database } from 'bun:sqlite';
import * as NodeCrypto from 'node:crypto';
import * as NodeFs from 'node:fs';
import * as NodeOs from 'node:os';
import * as NodePath from 'node:path';
import { spawnSync } from 'node:child_process';
import { ImportFsError, ImportSqlError, type ImportIO, type SqlRow, type SqlValue } from './browser-import-io';

const fsError = (cause: unknown): ImportFsError => new ImportFsError(String((cause as { code?: unknown })?.code ?? 'EIO'), String((cause as Error)?.message ?? cause));
function fs<T>(run: () => T): T { try { return run(); } catch (cause) { throw fsError(cause); } }
const toRow = (row: Record<string, unknown>): SqlRow => Object.fromEntries(Object.entries(row).map(([key, value]) =>
  [key, value instanceof Uint8Array ? new Uint8Array(value) : typeof value === 'bigint' ? Number(value) : value as SqlValue]));

const FCNTL_PROBE = "import fcntl,os,sys\nfd=os.open(sys.argv[1],os.O_WRONLY)\ntry:\n  fcntl.lockf(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)\nexcept BlockingIOError:\n  print('held')\nelse:\n  print('free')";

/** A scratch directory removed by `cleanup` (the reference's makeTempDirectoryScoped). */
export function tempDirectory(prefix: string): { path: string; cleanup(): void } {
  const path = NodeFs.mkdtempSync(NodePath.join(NodeOs.tmpdir(), prefix));
  return { path, cleanup: () => NodeFs.rmSync(path, { recursive: true, force: true }) };
}

export function bunImportIO(overrides: Partial<ImportIO> = {}): ImportIO {
  const io: ImportIO = {
    stat: async path => fs(() => { const info = NodeFs.statSync(path); return info.isFile() ? 'File' : info.isDirectory() ? 'Directory' : 'Other'; }),
    readDirectory: async path => fs(() => NodeFs.readdirSync(path)),
    readFileString: async path => fs(() => NodeFs.readFileSync(path, 'utf8')),
    readFile: async path => fs(() => new Uint8Array(NodeFs.readFileSync(path))),
    open: async path => fs(() => NodeFs.closeSync(NodeFs.openSync(path, 'r'))),
    readLink: async path => fs(() => NodeFs.readlinkSync(path)),
    async query(database, sql, params = []) {
      let db: Database | undefined;
      try {
        db = new Database(database, { readonly: true });
        return (db.query(sql).all(...(params as never[])) as Record<string, unknown>[]).map(toRow);
      } catch (cause) { throw new ImportSqlError(String((cause as Error)?.message ?? cause)); }
      finally { db?.close(); }
    },
    async snapshot(database) {
      const directory = NodeFs.mkdtempSync(NodePath.join(NodeOs.tmpdir(), 't3code-cookie-import-'));
      const target = NodePath.join(directory, NodePath.basename(database));
      let db: Database | undefined;
      try { db = new Database(database, { readonly: true }); db.run('VACUUM INTO ?', [target]); }
      catch (cause) { NodeFs.rmSync(directory, { recursive: true, force: true }); throw new ImportSqlError(String((cause as Error)?.message ?? cause)); }
      finally { db?.close(); }
      return { path: target, release: async () => NodeFs.rmSync(directory, { recursive: true, force: true }) };
    },
    keychainPassword: async () => null,
    pbkdf2Sha1: async (passphrase, salt, iterations, length) => new Uint8Array(NodeCrypto.pbkdf2Sync(passphrase, salt, iterations, length, 'sha1')),
    async decrypt(items) {
      return items.map(item => {
        try {
          if (item.mode === 'cbc') {
            const decipher = NodeCrypto.createDecipheriv(item.key.length === 32 ? 'aes-256-cbc' : 'aes-128-cbc', item.key, item.iv);
            return new Uint8Array(Buffer.concat([decipher.update(item.data), decipher.final()]));
          }
          const decipher = NodeCrypto.createDecipheriv('aes-256-gcm', item.key, item.nonce);
          decipher.setAuthTag(item.tag);
          return new Uint8Array(Buffer.concat([decipher.update(item.data), decipher.final()]));
        } catch { return null; }
      });
    },
    sha256: async items => items.map(item => new Uint8Array(NodeCrypto.createHash('sha256').update(item).digest())),
    signal0: async pid => { try { process.kill(pid, 0); } catch (cause) { throw fsError(cause); } },
    hostname: async () => NodeOs.hostname(),
    localAddresses: async () => ['127.0.0.1'],
    async lockHeld(path) {
      for (const interpreter of ['/usr/bin/python3', 'python3']) {
        const run = spawnSync(interpreter, ['-c', FCNTL_PROBE, path], { encoding: 'utf8' });
        const verdict = String(run.stdout ?? '').trim();
        if (verdict === 'held') return true;
        if (verdict === 'free') return false;
      }
      return false;
    },
  };
  return { ...io, ...overrides };
}
