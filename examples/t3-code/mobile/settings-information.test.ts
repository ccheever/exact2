import { expect, test } from 'bun:test';
import { mobileInformationPrepare, mobileInformationSnapshot, mobileInformationCommand, mobileLegalDocument } from './settings-information';
import { obj } from './shared/domain';
import type { Native } from './shared/protocol';
import manifest from './assets/mobile-third-party-licenses.json';
const calls:unknown[]=[];
function native(notices:unknown=manifest,opened=true):Native {return {available:true,watch(){},async later(input){calls.push(input);return {ok:true,generation:1,value:obj(input).op==='mobileOpenURL'?{opened}:{version:'2.3.4',build:'42',notices,noticeCoverage:manifest.coverage}};}};}
test('bundle identity and actual manifest decode without any server or storage operation',async()=>{
 calls.length=0;await mobileInformationPrepare('settingsAbout',native());const data=mobileInformationSnapshot();
 expect(data.version).toBe('2.3.4');expect(data.build).toBe('42');expect(data.licensesReady).toBe(true);expect(data.licenses.length).toBe(manifest.entries.length);
 expect(calls.map(call=>obj(call).op)).toEqual(['mobileInformation']);expect(data.coverage).toContain('have not been verified');
});
test('search requires every term; encoded detail ID returns full notice and real source',async()=>{
 await mobileInformationPrepare('settingsOpenSourceLicenses',native());const data=mobileInformationSnapshot('  SHIKI 4.2.0 themes ');
 expect(data.licenses.map(row=>row.name)).toEqual(['@shikijs/themes']);const detail=mobileInformationSnapshot('',data.licenses[0]!.id).detail;
 expect(detail.notice).toContain('Copyright (c) 2021 Pine Wu');expect(detail.sourceURL).toBe('https://github.com/shikijs/shiki');expect(mobileInformationSnapshot('','missing').detail.available).toBe(false);
});
test('missing/invalid manifests are unavailable rather than an empty-success notice list',async()=>{
 await mobileInformationPrepare('settingsOpenSourceLicenses',native(null));expect(mobileInformationSnapshot().licensesReady).toBe(false);expect(mobileInformationSnapshot().licenseMessage).toContain('unavailable');
 await mobileInformationPrepare('settingsOpenSourceLicenses',native({...manifest,entries:[manifest.entries[0],manifest.entries[0]]}));expect(mobileInformationSnapshot().licensesReady).toBe(false);
});
test('unsupported caches and crash logs never report zero or dispatch destructive operations',async()=>{
 const handle=native();await mobileInformationPrepare('settingsDiagnostics',handle);calls.length=0;const data=mobileInformationSnapshot();
 expect(data.storage.available).toBe(false);expect(data.diagnostics.available).toBe(false);expect(data.diagnostics.title).toBe('Crash log unavailable');
 expect((await mobileInformationCommand('clear-cache','all',handle)).message).toContain('unavailable');expect((await mobileInformationCommand('copy-crashes','',handle)).message).toContain('unavailable');expect(calls).toEqual([]);
});
test('external source must belong to actual notice and acknowledge native open',async()=>{
 const handle=native();await mobileInformationPrepare('settingsOpenSourceLicenses',handle);const id=mobileInformationSnapshot('shikijs themes').licenses[0]!.id;calls.length=0;
 expect((await mobileInformationCommand('license-source','https://attacker.test',handle)).message).toContain('unavailable');expect(calls).toEqual([]);
 expect((await mobileInformationCommand('license-source',id,handle)).message).toBe('');expect(obj(calls[0]).url).toBe('https://github.com/shikijs/shiki');
 expect((await mobileInformationCommand('license-source',id,native(manifest,false))).message).toContain('could not be opened');
});
test('legal allowlist is origin plus exact path, ignoring query/hash/trailing slashes',()=>{
 for(const url of ['https://t3.codes/legal','https://t3.codes/privacy-policy/?a=b#section','https://t3.codes:443/security-policy'])expect(mobileLegalDocument(url)).toBe(true);
 for(const url of ['https://t3.codes/legal/child','https://t3.codes.evil.test/legal','http://t3.codes/legal','javascript:alert(1)','mailto:help@t3.codes'])expect(mobileLegalDocument(url)).toBe(false);
});
test('late bundle read cannot replace the new route preparation',async()=>{
 let release!:(value:unknown)=>void;const delayed:Native={available:true,watch(){},later(){return new Promise(resolve=>{release=resolve;});}};
 const old=mobileInformationPrepare('settingsAbout',delayed);await mobileInformationPrepare('settingsAbout',native());release({ok:true,generation:1,value:{version:'stale',notices:manifest}});await old;expect(mobileInformationSnapshot().version).toBe('2.3.4');
});
