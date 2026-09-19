#!/usr/bin/env bun
import { resolve } from 'node:path';
import { existsSync } from 'node:fs';

const name = process.argv[2];
if (!name || (!name.includes('/') && !/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(name))) throw new Error('Usage: bun game/dev.mjs <name|path> [dev options]');
const app = name.includes('/') ? resolve(name) : resolve(import.meta.dir, 'games', name);
if (!existsSync(resolve(app, 'app.contract'))) throw new Error(`No game: ${app}`);
Object.assign(process.env, {EXACT_APP_DIR:app, EXACT_UPDATE_TRUST:'development', EXACT_LOOPBACK:'1', EXACT_WEB_DIST:resolve(app, 'dist')});
process.argv = [process.argv[0], resolve(import.meta.dir, '../host/web/dev.mjs'), ...process.argv.slice(3)];
await import('../host/web/dev.mjs');
