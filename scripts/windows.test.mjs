import {test, expect} from 'bun:test';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {filesystem, filesystemErrorCode} from './filesystem.mjs';
import {packagedBuildChanges} from './agent-launch.mjs';
import {runCaps} from './caps.mjs';

test('caps command really runs from a checkout path containing spaces', () => {
  const root=resolve(import.meta.dir,'..');
  const result=spawnSync(process.execPath,[resolve(import.meta.dir,'caps.mjs')],{cwd:root,encoding:'utf8'});
  expect(result.stdout).toContain('caps — budgets declared');
  expect(result.status).toBe(runCaps(root).problems.length ? 1 : 0);
});

test('filesystem errors distinguish Windows access denial from Unix IO failure', () => {
  expect(filesystemErrorCode(5,'win32')).toBe('EACCES');
  expect(filesystemErrorCode(5,'linux')).toBe('EIO');
  expect(filesystemErrorCode(3,'win32')).toBe('ENOENT');
  expect(filesystemErrorCode(32,'win32')).toBe('EBUSY');
  expect(filesystemErrorCode(null,'win32')).toBe('EXACT_FS_REFUSED');
});

test('a real filesystem helper missing-parent response preserves ENOENT', () => {
  const root=mkdtempSync(resolve(tmpdir(),'exact-fs-code-'));
  try { expect(()=>filesystem({root,op:'read',path:'missing/file'})).toThrow(expect.objectContaining({code:'ENOENT'})); }
  finally { rmSync(root,{recursive:true,force:true}); }
});

test('packaged native freshness rejects changed source and copied binary bytes', () => {
  const root=mkdtempSync(resolve(tmpdir(),'exact-package-receipt-'));
  const receipt=resolve(root,'build.json'), source=resolve(root,'source.rs'), binary=resolve(root,'game.exe'), gpu=resolve(root,'game.dll');
  const digest=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
  try {
    expect(packagedBuildChanges(receipt,root)).toEqual(['missing compiler build receipt']);
    for(const path of [source,binary,gpu]) writeFileSync(path,path);
    writeFileSync(receipt,JSON.stringify({version:1,binary:{inputs:[{path:source,name:'source',sha256:digest(source)}],missing:[],directories:[]},products:[binary,gpu].map(path=>({path,sha256:digest(path)}))}));
    expect(packagedBuildChanges(receipt,root)).toEqual([]);
    writeFileSync(source,'changed'); expect(packagedBuildChanges(receipt,root)).toEqual(['source']);
    writeFileSync(source,source); writeFileSync(gpu,'changed'); expect(packagedBuildChanges(receipt,root)).toEqual([gpu]);
    rmSync(binary); expect(packagedBuildChanges(receipt,root)).toEqual([binary,gpu]);
  } finally { rmSync(root,{recursive:true,force:true}); }
});
