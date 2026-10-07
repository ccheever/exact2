import { afterEach, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { mobileClient } from './client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { collectLimitAccounts, collectLimitPools, displayLimitWindows, type LimitPresentations } from './settings-usage-limits';
import { mobileUsageConfig } from './settings-usage-types';
import { mobileUsage, mobileUsageCommand, mobileUsageSelection, mobileUsageSnapshot, mobileUsageWindow, usageGrant } from './settings-usage';
import { mobileUsagePresentation, mobileUsageAccount } from './settings-usage-view';
const window=(kind='session',usedPercent=50)=>({id:'primary',kind,label:'Allowance',usedPercent,resetsAt:'2026-10-08T00:00:00Z',windowDurationMins:300});
const nativeAccount=(limits:Obj,email=' A@Example.test '):Obj=>({instanceId:'one',driver:'codex',enabled:true,installed:true,auth:{status:'authenticated',email},usageLimits:limits});
const limits:Obj={checkedAt:'2026-10-07T00:00:00Z',windows:[window()],resetCredits:{availableCount:1}};
test('native and hub accounts deduplicate, freshest windows win, hub redemption clears routing cooldown',()=>{
  const presentations:LimitPresentations=new Map([['a',{entry:{target:{label:'A'}},serverConfig:mobileUsageConfig({providers:[nativeAccount(limits)]})}],['b',{entry:{target:{label:'B'}},serverConfig:mobileUsageConfig({providers:[nativeAccount({...limits,checkedAt:'2026-10-07T02:00:00Z',windows:[window('session',80)]},'a@example.test')],usageLimitSources:[{id:'hub',label:'Hub',accounts:[{id:'account.json',driver:'codex',email:'a@example.test',usageLimits:{...limits,resetCredits:{availableCount:3,nextCreditId:'credit'}}}]}]})}]]);
  const accounts=collectLimitAccounts(presentations);expect(accounts).toHaveLength(1);expect(accounts[0]?.limits.windows[0]?.usedPercent).toBe(80);expect(accounts[0]?.environments).toHaveLength(2);expect(accounts[0]?.redeem).toEqual({environmentId:'b',input:{sourceId:'hub',accountId:'account.json',creditId:'credit'}});
  expect(collectLimitPools(accounts,0)[0]?.windows[0]?.remainingPercent).toBe(20);
});
test('same window id across monthly/session stays separate; missing windows keep fixed columns',()=>{
  const config=mobileUsageConfig({providers:[nativeAccount(limits),{...nativeAccount({...limits,windows:[window('monthly',20)]},'b@test'),instanceId:'two'}]});
  const accounts=collectLimitAccounts(new Map([['a',{entry:{target:{label:'A'}},serverConfig:config}]])),pool=collectLimitPools(accounts,0)[0]!;
  expect(pool.windows).toHaveLength(2);expect(pool.windows[0]?.columns).toHaveLength(2);expect(pool.windows[0]?.columns.filter(column=>column.window===null)).toHaveLength(1);
});
test('invalid quota is not fabricated as zero and Cursor combined quota is hidden with both pools',()=>{
  const config=mobileUsageConfig({providers:[{...nativeAccount({...limits,windows:[{...window(),usedPercent:'unknown'}]}),driver:'cursor'}]});expect(config.providers[0]?.usageLimits?.windows).toEqual([]);
  const cursor=mobileUsageConfig({providers:[{...nativeAccount({...limits,windows:['totalPercentUsed','autoPercentUsed','apiPercentUsed'].map(id=>({...window(),id}))}),driver:'cursor'}]});
  const pool=collectLimitPools(collectLimitAccounts(new Map([['a',{entry:{target:{label:'A'}},serverConfig:cursor}]])),0)[0]!;expect(displayLimitWindows(pool).map(window=>window.id)).toEqual(['autoPercentUsed','apiPercentUsed']);
});
test('explicit empty selection stays empty; diagnostic permissions distinguish old session scopes',()=>{
  expect(mobileUsageSelection('null')).toBeNull();expect(mobileUsageSelection('[]')).toEqual([]);expect(()=>mobileUsageSelection('{}')).toThrow();
  expect(usageGrant({authenticated:true,scopes:['orchestration:read']})).toBe(true);expect(usageGrant({authenticated:true,scopes:['orchestration:read'],auth:{serverUpdateScope:'environment:maintain'}})).toBe(false);expect(usageGrant({authenticated:true,scopes:['diagnostics:read'],permissions:[]})).toBe(false);
});
const original={origin:mobileClient.origin,environmentId:mobileClient.environmentId,connection:mobileClient.connection,config:mobileClient.config,generation:mobileClient.generation};
afterEach(async()=>{await mobileUsage('null','limits','{}','cost',0,null,false,false);Object.assign(mobileClient,original);});
let generation=201;
function transport(permissions=['diagnostics:read','providers:manage']){const calls:Obj[]=[];const id=`usage-${++generation}`,config={settings:{},providers:[nativeAccount(limits)]};Object.assign(mobileClient,{origin:'http://usage.test',environmentId:id,connection:'connected',generation,config});let ratesFail=false;
  const native:Native={available:true,watch(){},async later(input){const request=obj(input);calls.push(request);if(request.method==='server.refreshUsageRates'&&ratesFail)return {ok:false,generation,error:{kind:'server',message:'Rates offline'}};
    const value=request.op==='environments'?{saved:[{origin:'http://usage.test',environmentId:id}]}:request.op==='http'?{authenticated:true,permissions}:request.method==='server.getConfig'?mobileClient.config:request.method==='server.getUsageSummary'?{contractVersion:6,readAt:'2026-10-07T00:00:00Z',sources:[],buckets:[]}:request.method==='provider.consumeResetCredit'?{outcome:'noCredit'}:{};return {ok:true,generation,value};}};
  return {id,native,calls,permissions,failRates:()=>{ratesFail=true;}};
}
const now=Date.parse('2026-10-07T12:00:00Z'),windowJSON=JSON.stringify(mobileUsageWindow(30,now));
test('metric projection is network-free; scope mismatch cannot display another account',async()=>{
  const {native,calls}=transport();const data=await mobileUsage('null','limits',windowJSON,'cost',now,native);expect(data.pools).toHaveLength(1);const count=calls.length;
  expect(mobileUsageSnapshot('null','usage','tokens').metric).toBe('tokens');expect(calls).toHaveLength(count);expect(mobileUsageSnapshot('[]','limits','cost').ready).toBe(false);
});
test('usage rescan still runs after pricing refresh failure; selected empty set sends no summary read',async()=>{
  const {native,calls,failRates}=transport();await mobileUsage('null','usage',windowJSON,'cost',now,native);failRates();const before=calls.filter(call=>call.method==='server.getUsageSummary').length;
  expect((await mobileUsageCommand('refresh-usage','','',now,native)).message).toContain('Rates offline');expect(calls.filter(call=>call.method==='server.getUsageSummary')).toHaveLength(before+1);
  calls.length=0;const empty=await mobileUsage('[]','usage',windowJSON,'cost',now,native);expect(empty.environments).toEqual([]);expect(calls.some(call=>call.method==='server.getUsageSummary')).toBe(false);
});
test('reset credit requires confirmation and current permission; real noCredit outcome is not success',async()=>{
  const {id,native,calls,permissions}=transport();const data=await mobileUsage('null','limits',windowJSON,'cost',now,native),key=data.pools[0]!.windows[0]!.columns[0]!.key;
  expect(mobileUsageAccount(key,'primary','session',now).email).toBe('••••••••');expect((await mobileUsageCommand('redeem',id,key,now,native)).message).toContain('confirmation');expect(calls.some(call=>call.method==='provider.consumeResetCredit')).toBe(false);
  permissions.splice(permissions.indexOf('providers:manage'),1);expect((await mobileUsageCommand('redeem-confirmed',id,key,now,native)).message).toContain('permission');
  permissions.push('providers:manage');expect((await mobileUsageCommand('redeem-confirmed',id,key,now,native)).message).toBe('No reset credit left.');expect(calls.filter(call=>call.method==='provider.consumeResetCredit')).toHaveLength(1);
});
test('future usage contract cannot render made-up zero totals',()=>{
  const data=mobileUsagePresentation({tab:'usage',window:mobileUsageWindow(30,now),metric:'cost',now,presentations:new Map(),external:false,sources:[{id:'a',label:'A',connected:true,summary:{contractVersion:7},error:'',canRead:true,canWrite:false,canManage:false,config:{}}]});
  expect(data.hasUsage).toBe(false);expect(data.total).toBe('');expect(data.notices[0]?.text).toContain('Update this app');
});

test('compact flow keeps local metric changes out of async preparation',async()=>{
  const {mobileAutomationPrepare,mobileAutomationSnapshot,mobileAutomationCommand}=await import('./settings-scheduled-flow');
  const {native,calls}=transport();await mobileAutomationPrepare('settingsUsage','{}','','','',now,native);const count=calls.length;
  const change=await mobileAutomationCommand('usage-local','metric','tokens','',now,native);expect(change.refresh).toBe(false);expect(mobileAutomationSnapshot().usage.metric).toBe('tokens');expect(calls).toHaveLength(count);
  await mobileAutomationPrepare('settings','{}','','','',now,native);expect(mobileAutomationSnapshot().usage.ready).toBe(false);
});


test('opening a usage link preserves the selected time window and does not request refresh',async()=>{
  const {mobileAutomationPrepare,mobileAutomationSnapshot,mobileAutomationCommand}=await import('./settings-scheduled-flow');
  const {native,calls}=transport(),url='https://usage.example.test/account';
  mobileClient.config={settings:{},providers:[nativeAccount({...limits,externalUsage:{label:'Account usage',url}})]};
  const opener:Native={...native,async later(input){if(obj(input).op==='mobileOpenURL'){calls.push(obj(input));return {ok:true,generation:mobileClient.generation,value:{opened:true}};}return native.later(input);}};
  await mobileAutomationPrepare('settingsUsage','{}','','','',now,opener);const before=calls.length;
  const change=await mobileAutomationCommand('usage-command','open-link','',url,now+60_000,opener);
  expect(change.message).toBe('');expect(change.refresh).toBe(false);expect(mobileAutomationSnapshot().now).toBe(now);
  expect(calls.slice(before).map(call=>call.op)).toEqual(['mobileOpenURL']);
  await mobileAutomationPrepare('settings','{}','','','',now,opener);
});

test('nonempty usage provider rows project only declared UI fields',()=>{
  const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'claude',hostId:'host',resolvedHomePath:'/home'},distinctSessions:1}],
    buckets:[{provider:'claude',model:'model',day:'2026-10-07',costUsd:2,totals:{uncachedInputTokens:1000}}]};
  const contract=readFileSync(new URL('./settings-usage.contract',import.meta.url),'utf8');
  const declared=contract.split('shape UsageProvider\n')[1]?.split('shape UsageValue')[0]??'';
  const keys=[...declared.matchAll(/^  (\w+):/gm)].map(match=>match[1]).sort();
  expect(keys).toHaveLength(7);
  for(const metric of ['cost','tokens']){
    const data=mobileUsagePresentation({tab:'usage',window:mobileUsageWindow(30,now),metric,now,presentations:new Map(),external:false,
      sources:[{id:'a',label:'A',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]});
    expect(data.providers).toHaveLength(1);
    expect(Object.keys(data.providers[0]!).sort()).toEqual(keys);
    expect(data.providers[0]).toMatchObject({id:'claude',label:'Claude Code',value:metric==='cost'?'$2.00':'1K',share:'100.0%'});
  }
});
