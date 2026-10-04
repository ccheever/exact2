import {test, expect} from 'bun:test';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {closeFilesystemReader, filesystem, filesystemErrorCode} from './filesystem.mjs';
import {packagedBuildChanges} from './agent-launch.mjs';
import {runCaps} from './caps.mjs';
import {binaryenArchive, binaryenVersion} from './exact.mjs';
import {listPublicFiles, publicFileCards, readStaticFile, readStaticFileAsync, staticFile} from '../host/web/serve.mjs';

test('public web inventory and owned reads agree on native Windows paths', async () => {
  const root=mkdtempSync(resolve(tmpdir(),'exact static paths '));
  try {
    mkdirSync(resolve(root,'stages'));
    writeFileSync(resolve(root,'index.html'),'game');
    writeFileSync(resolve(root,'stages/inspection.wasm'),'inspection');
    expect(listPublicFiles(root)).toEqual(['index.html','stages/inspection.wasm']);
    expect(readStaticFile(root,'/').body.toString()).toBe('game');
    expect((await readStaticFileAsync(root,'/stages/inspection.wasm')).body.toString()).toBe('inspection');
    expect((await publicFileCards(root)).map(file=>[file.name,file.bytes])).toEqual([['index.html',4],['stages/inspection.wasm',10]]);
    for (const path of ['/stages/../../secret','/stages/%2e%2e/%2e%2e/secret','/stages\\inspection.wasm']) expect(staticFile(root,path)).toBeNull();
  } finally { closeFilesystemReader(); rmSync(root,{recursive:true,force:true}); }
});

test('setup selects supported pinned Binaryen archives on Windows and Unix', () => {
  expect(binaryenArchive('version_132','win32','x64')).toBe('binaryen-version_132-x86_64-windows.tar.gz');
  expect(binaryenArchive('version_132','darwin','arm64')).toBe('binaryen-version_132-arm64-macos.tar.gz');
  expect(binaryenArchive('version_132','linux','arm64')).toBe('binaryen-version_132-aarch64-linux.tar.gz');
  expect(()=>binaryenArchive('version_132','win32','arm64')).toThrow('no Binaryen setup');
  expect(binaryenVersion('wasm-opt version 132 (version_132)')).toBe('version 132');
});

test('SDK CLI entrypoint executes with spaces in its real file path', () => {
  const result=spawnSync(process.execPath,[resolve(import.meta.dir,'exact.mjs'),'--help'],{encoding:'utf8'});
  expect(result.status).toBe(0);
  expect(result.stdout).toContain('exact setup [--check]');
});

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
