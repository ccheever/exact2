import { afterEach, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { mobileClient } from './client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { collectLimitAccounts, collectLimitPools, displayLimitWindows, type LimitPresentations } from './settings-usage-limits';
import { mobileUsageConfig } from './settings-usage-types';
import { mobileUsage, mobileUsageCommand, mobileUsageSelection, mobileUsageSnapshot, mobileUsageWindow, usageGrant } from './settings-usage';
import { mobileUsagePresentation, mobileUsageAccount, mobileUsageProviderRows, mobileUsageReveal } from './settings-usage-view';
import { formatDateTimeShort, formatHourShort, formatUsd, mergeUsage } from './shared/pages-usage';
import { mobileAutomationPrepare, mobileAutomationSnapshot, mobileAutomationCommand } from './settings-scheduled-flow';
import { ClientError } from './shared/protocol';
import { settingsColors, settingsTokens } from './settings-appearance';
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
function presentedLimits(providers:Obj[],at=now,publish=false){
  const config=mobileUsageConfig({providers}),presentations:LimitPresentations=new Map([['a',{entry:{target:{label:'A'}},serverConfig:config}]]);
  return mobileUsagePresentation({tab:'limits',window:mobileUsageWindow(30,at),metric:'cost',now:at,presentations,external:false,
    sources:[{id:'a',label:'A',connected:true,summary:null,error:'',canRead:true,canWrite:false,canManage:false,config}]},publish);
}
test('pooled Limits headers retain window and account order while selecting the first positive refill',()=>{
  const at=Date.parse('2026-10-07T12:00:00Z'),reset=(minutes:number)=>new Date(at+minutes*60_000).toISOString();
  const a={...nativeAccount({...limits,windows:[{...window('weekly',80),id:'weekly',label:'Weekly',resetsAt:reset(300)},
    {...window('session',0),label:'Session',resetsAt:reset(1)}]},'a@test'),displayName:'Alpha'};
  const b={...nativeAccount({...limits,windows:[{...window('session',50),label:'Session',resetsAt:reset(120)}]},'b@test'),instanceId:'two',displayName:'Beta'};
  const data=presentedLimits([a,b],at),pool=data.pools[0]!;
  expect([pool.id,pool.label]).toEqual(['codex','Codex']);
  expect(pool.windows.map(item=>[item.id,item.label,item.remaining,item.nextRefill])).toEqual([
    ['session:primary','Session',75,'↻ +25% in 2h 0m'],['weekly:weekly','Weekly',20,'↻ +80% in 5h 0m'],
  ]);
  expect(pool.windows.map(item=>item.columns.map(column=>[column.key,column.label,column.present,column.remaining]))).toEqual([
    [['a:one','Alpha',true,100],['a:two','Beta',true,50]],
    [['a:one','Alpha',true,20],['a:two','Beta',false,0]],
  ]);
  expect(pool.windows.map(item=>item.description)).toEqual(['','']);expect(data.cursorPoolInsertAt).toBe(1);
});
test('Limits refill and pace copy use the captured clock and preserve zero or absent resets',()=>{
  const at=Date.parse('2026-10-07T12:00:00Z');
  for(const [usedPercent,pace] of [[20,'Under pace'],[50,'On pace'],[80,'Ahead of pace']] as const){
    const data=presentedLimits([nativeAccount({...limits,windows:[{...window('session',usedPercent),windowDurationMins:60,resetsAt:new Date(at+30*60_000).toISOString()}]})],at);
    expect(data.pools[0]!.windows[0]!.pace).toBe(pace);
  }
  for(const [usedPercent,resetsAt,refill] of [[40,new Date(at).toISOString(),'↻ +40% now'],[0,new Date(at).toISOString(),''],[40,undefined,'']] as const){
    const providers=[nativeAccount({...limits,windows:[{...window('session',usedPercent),resetsAt}]})],before=JSON.stringify(providers),data=presentedLimits(providers,at);
    expect(data.pools[0]!.windows[0]!.nextRefill).toBe(refill);expect(JSON.stringify(providers)).toBe(before);
  }
});
test('Limits headings and Cursor window descriptions follow pooled reference labels',()=>{
  const cursor={...nativeAccount({...limits,windows:['totalPercentUsed','apiPercentUsed','autoPercentUsed'].map(id=>({...window(),id}))},'cursor@test'),driver:'cursor'};
  const claude={...nativeAccount(limits,'claude@test'),driver:'claudeAgent'},grok={...nativeAccount(limits,'grok@test'),driver:'grok'};
  const data=presentedLimits([claude,cursor,grok]);expect(data.pools.map(pool=>[pool.id,pool.label])).toEqual([['claudeAgent','Claude'],['cursor','cursor'],['grok','grok']]);
  expect(data.pools[1]!.windows.map(item=>[item.windowId,item.label,item.description])).toEqual([
    ['autoPercentUsed','Cursor Models','Grok and Composer use this first. Auto can use either pool.'],
    ['apiPercentUsed','Other Models','Claude, GPT, and Gemini use this pool. Grok and Composer fall back here.'],
  ]);
  expect(data.pools[0]!.windows[0]!.label).toBe('Allowance');expect(data.pools[0]!.windows[0]!.description).toBe('');
  expect(data.cursorPoolInsertAt).toBe(1);
});
test('pooled Limits columns keep fixed account slots and per-window reset and credit metadata',()=>{
  const at=Date.parse('2026-10-07T12:00:00Z'),reset=(minutes:number)=>new Date(at+minutes*60_000).toISOString();
  const a={...nativeAccount({...limits,resetCredits:{availableCount:2},windows:[{...window('session',0),resetsAt:reset(60)},
    {...window('weekly',100),id:'weekly',resetsAt:undefined}]},'alpha@example.test'),displayName:'Alpha'};
  const b={...nativeAccount({...limits,resetCredits:{availableCount:0},windows:[{...window('session',37.4),resetsAt:reset(90)}]},'beta@example.test'),instanceId:'two',displayName:'Beta'};
  const providers=[a,b],before=JSON.stringify(providers),data=presentedLimits(providers,at),[session,weekly]=data.pools[0]!.windows;
  expect(session!.columns.map(item=>[item.key,item.label,item.present,item.remaining,item.pending,item.resetsIn,item.resetLabel,item.credits])).toEqual([
    ['a:one','Alpha',true,100,true,'resets in 1h 0m','↻ 1h 0m',2],
    ['a:two','Beta',true,63,true,'resets in 1h 30m','↻ 1h 30m',0],
  ]);
  expect(weekly!.columns.map(item=>[item.key,item.present,item.remaining,item.pending,item.resetsIn,item.resetLabel,item.credits])).toEqual([
    ['a:one',true,0,false,'','',2],['a:two',false,0,false,'','',0],
  ]);
  expect(JSON.stringify(providers)).toBe(before);expect(data.cursorPoolInsertAt).toBe(1);
});
test('pooled Limits account names use configured names, email initials and Limits driver fallbacks',()=>{
  for(const [driver,email,displayName,label] of [
    ['codex','alice@example.test','  Personal account  ','Personal account'],
    ['codex','alice@example.test',undefined,'AE'],['codex','@',undefined,'Account'],
    ['claudeAgent',undefined,undefined,'Claude'],['cursor',undefined,undefined,'cursor'],
  ] as const){
    const account={...nativeAccount(limits),driver,displayName,auth:{status:'authenticated',email}},before=JSON.stringify(account);
    const column=presentedLimits([account]).pools[0]!.windows[0]!.columns[0]!;
    expect(column.label).toBe(label);expect(column.key).toBe('a:one');expect(column.email).toBe(email?'••••••••':'');
    expect(JSON.stringify(account)).toBe(before);
  }
});
test('pooled Limits resets retain now copy and omit optional metadata without changing quota values',()=>{
  const at=Date.parse('2026-10-07T12:00:00Z');
  for(const [resetsAt,pending,resetsIn,resetLabel] of [
    [new Date(at-60_000).toISOString(),true,'resets now','resets now'],
    [undefined,false,'',''],
  ] as const){
    const account=nativeAccount({checkedAt:limits.checkedAt,windows:[{...window('session',50),resetsAt}]});
    const item=presentedLimits([account],at).pools[0]!.windows[0]!.columns[0]!;
    expect([item.remaining,item.pending,item.resetsIn,item.resetLabel,item.credits]).toEqual([50,pending,resetsIn,resetLabel,0]);
  }
});
test('pooled Limits segments use Claude orange or Codex neutral and bind the badge theme role',()=>{
  const drivers=['codex','claudeAgent','cursor','grok','opencode','antigravity'];
  for(const driver of drivers){
    const data=presentedLimits([{...nativeAccount(limits),driver}]);
    expect([data.pools[0]!.light,data.pools[0]!.dark]).toEqual(driver==='claudeAgent'?['#d97757','#d97757']:['#3c3c43','#e6e6e6']);
  }
  for(const mode of ['light','dark'])for(const theme of ['t3-code','t3-chat','grove','ocean','ember','iris'])
    expect(settingsColors(mode,theme).subtleStrong).toBe(settingsTokens(mode,theme)['--color-subtle-strong']);
});
test('Account detail becomes unavailable when its captured window disappears while the account remains',()=>{
  const at=Date.parse('2026-10-07T12:00:00Z'),session={...window('session',50),label:'Session'},weekly={...window('weekly',25),id:'weekly',label:'Weekly'};
  const initial=presentedLimits([nativeAccount({...limits,windows:[session,weekly]})],at,true),key=initial.pools[0]!.windows[0]!.columns[0]!.key;
  expect(mobileUsageAccount(key,'primary','session',at).present).toBe(true);
  const changed=presentedLimits([nativeAccount({...limits,windows:[weekly]})],at,true);
  expect(changed.pools[0]!.windows[0]!.columns[0]!.key).toBe(key);
  for(const [id,kind] of [['primary','session'],['weekly','session'],['missing','weekly']]){
    const detail=mobileUsageAccount(key,id!,kind!,at);
    expect([detail.present,detail.error]).toEqual([false,'This account is no longer reporting limits on the selected environments.']);
  }
});
test('Account detail remains unavailable when the captured native account disappears',()=>{
  const at=Date.parse('2026-10-07T12:00:00Z'),initial=presentedLimits([nativeAccount(limits)],at,true),key=initial.pools[0]!.windows[0]!.columns[0]!.key;
  presentedLimits([],at,true);
  const detail=mobileUsageAccount(key,'primary','session',at);
  expect([detail.present,detail.error]).toEqual([false,'This account is no longer reporting limits on the selected environments.']);
});
test('Account detail omits restoration when its reset restores zero or no reset is reported',()=>{
  const at=Date.parse('2026-10-07T12:00:00Z'),reset=new Date(at+60*60_000).toISOString();
  for(const [usedPercent,resetsAt] of [[0,reset],[0.4,reset],[50,undefined]] as const){
    const providers=[nativeAccount({...limits,windows:[{...window('session',usedPercent),resetsAt}]}),
      {...nativeAccount({...limits,windows:[{...window('session',75),resetsAt:reset}]},'second@example.test'),instanceId:'two'}];
    const before=JSON.stringify(providers);presentedLimits(providers,at,true);
    const detail=mobileUsageAccount('a:one','primary','session',at);
    expect(detail.present).toBe(true);expect(detail.restores).toBe('');expect(JSON.stringify(providers)).toBe(before);
  }
});
test('Account detail preserves positive pooled restoration for future and past resets at the captured clock',()=>{
  const at=Date.parse('2026-10-07T12:00:00Z');
  for(const delta of [-60,60]){
    const reset=new Date(at+delta*60_000).toISOString(),providers=[nativeAccount({...limits,windows:[{...window('session',50),resetsAt:reset}]}),
      {...nativeAccount({...limits,windows:[{...window('session',75),resetsAt:reset}]},'second@example.test'),instanceId:'two'}];
    const before=JSON.stringify(providers);presentedLimits(providers,at,true);
    const detail=mobileUsageAccount('a:one','primary','session',at);
    expect([detail.present,detail.error,detail.restores]).toEqual([true,'','Restores 25% of the pool']);
    expect(detail.resets.length).toBeGreaterThan(0);expect(JSON.stringify(providers)).toBe(before);
  }
});
test('Account detail keeps native environment rows separate and names a hub source without inventing a sign-in',()=>{
  const at=Date.parse('2026-10-07T12:00:00Z');
  const native:LimitPresentations=new Map([
    ['a',{entry:{target:{label:'Mac A'}},serverConfig:mobileUsageConfig({providers:[{...nativeAccount(limits,'shared@example.test'),displayName:'Work account'}]})}],
    ['b',{entry:{target:{label:'Mac B'}},serverConfig:mobileUsageConfig({providers:[nativeAccount(limits,'shared@example.test')]})}],
  ]);
  const hub:LimitPresentations=new Map([['a',{entry:{target:{label:'Mac A'}},serverConfig:mobileUsageConfig({providers:[],usageLimitSources:[{id:'hub',label:'CLI Proxy',accounts:[{id:'account.json',driver:'codex',email:'hub@example.test',usageLimits:limits}]}]})}]]);
  for(const item of [
    {presentations:native,key:'a:one',title:'Signed in',rows:[{id:'a',label:'Mac A'},{id:'b',label:'Mac B'}]},
    {presentations:hub,key:'hub:account.json',title:'Source',rows:[{id:'source',label:'CLI Proxy'}]},
  ]){
    const before=JSON.stringify([...item.presentations]);
    mobileUsagePresentation({tab:'limits',window:mobileUsageWindow(30,at),metric:'cost',now:at,presentations:item.presentations,external:false,sources:[]},true);
    const detail=mobileUsageAccount(item.key,'primary','session',at);
    expect(detail.present).toBe(true);expect(detail.sourceTitle).toBe(item.title);expect(detail.sourceItems).toEqual(item.rows);
    expect(JSON.stringify([...item.presentations])).toBe(before);
  }
});
test('Account detail uses pooled driver labels and medium-date short-time reset copy',()=>{
  const at=Date.parse('2026-10-07T12:00:00Z'),resetsAt='2026-10-08T18:23:47Z';
  const resetText=new Intl.DateTimeFormat(undefined,{dateStyle:'medium',timeStyle:'short'}).format(Date.parse(resetsAt));
  for(const [driver,label] of [['codex','Codex'],['claudeAgent','Claude'],['cursor','cursor'],['grok','grok']] as const){
    const providers=[{...nativeAccount({...limits,windows:[{...window('session',37.4),resetsAt}]},'detail@example.test'),driver}],before=JSON.stringify(providers);
    presentedLimits(providers,at,true);const detail=mobileUsageAccount('a:one','primary','session',at);
    expect([detail.driver,detail.label,detail.windowLabel,detail.remaining,detail.resets]).toEqual([driver,label,'Allowance','63% left',resetText]);
    expect(JSON.stringify(providers)).toBe(before);
  }
  presentedLimits([{...nativeAccount({...limits,windows:[{...window(),resetsAt:undefined}]}),driver:'claudeAgent',displayName:'Configured Claude'}],at,true);
  const named=mobileUsageAccount('a:one','primary','session',at);expect(named.label).toBe('Configured Claude');expect(named.resets).toBe('');
});
test('Account detail masks email as the primary does while preserving the existing per-key reveal state',()=>{
  const at=Date.parse('2026-10-07T12:00:00Z'),providers=[nativeAccount(limits,'first@example.test'),{...nativeAccount(limits,'second@example.test'),instanceId:'two'}],before=JSON.stringify(providers);
  presentedLimits(providers,at,true);
  const hidden=mobileUsageAccount('a:one','primary','session',at);expect([hidden.hasEmail,hidden.revealed,hidden.email]).toEqual([true,false,'••••••@••••••']);
  mobileUsageReveal('a:one');
  expect(mobileUsageAccount('a:one','primary','session',at).email).toBe('first@example.test');
  expect(mobileUsageAccount('a:two','primary','session',at).email).toBe('••••••@••••••');
  presentedLimits(providers,at,true);expect(mobileUsageAccount('a:one','primary','session',at).revealed).toBe(true);
  mobileUsageReveal('a:one');expect(mobileUsageAccount('a:one','primary','session',at).email).toBe('••••••@••••••');
  expect(JSON.stringify(providers)).toBe(before);
  presentedLimits([{...nativeAccount(limits),auth:{status:'authenticated'}}],at,true);
  const absent=mobileUsageAccount('a:one','primary','session',at);expect([absent.hasEmail,absent.email]).toEqual([false,'']);
});
test('explicit empty selection stays empty; diagnostic permissions distinguish old session scopes',()=>{
  expect(mobileUsageSelection('null')).toBeNull();expect(mobileUsageSelection('[]')).toEqual([]);expect(()=>mobileUsageSelection('{}')).toThrow();
  expect(usageGrant({authenticated:true,scopes:['orchestration:read']})).toBe(true);expect(usageGrant({authenticated:true,scopes:['orchestration:read'],auth:{serverUpdateScope:'environment:maintain'}})).toBe(false);expect(usageGrant({authenticated:true,scopes:['diagnostics:read'],permissions:[]})).toBe(false);
});
const original={origin:mobileClient.origin,environmentId:mobileClient.environmentId,connection:mobileClient.connection,config:mobileClient.config,generation:mobileClient.generation};
afterEach(async()=>{await mobileAutomationPrepare('settings','{}','','','',0,null);await mobileUsage('null','limits','{}','cost',0,null,false,false);Object.assign(mobileClient,original);});
let generation=201;
function transport(permissions=['diagnostics:read','providers:manage']){const calls:Obj[]=[];const id=`usage-${++generation}`,config={settings:{},providers:[nativeAccount(limits)]};Object.assign(mobileClient,{origin:'http://usage.test',environmentId:id,connection:'connected',generation,config});let ratesFail=false;let intercept:((request:Obj)=>Promise<void>)|null=null;
  const native:Native={available:true,watch(){},async later(input){const request=obj(input);calls.push(request);await intercept?.(request);if(request.method==='server.refreshUsageRates'&&ratesFail)return {ok:false,generation,error:{kind:'server',message:'Rates offline'}};
    const value=request.op==='environments'?{saved:[{origin:'http://usage.test',environmentId:id}]}:request.op==='http'?{authenticated:true,permissions}:request.method==='server.getConfig'?mobileClient.config:request.method==='server.getUsageSummary'?{contractVersion:6,readAt:'2026-10-07T00:00:00Z',sources:[],buckets:[]}:request.method==='provider.consumeResetCredit'?{outcome:'noCredit'}:{};return {ok:true,generation,value};}};
  return {id,native,calls,permissions,failRates:()=>{ratesFail=true;},intercept:(hook:((request:Obj)=>Promise<void>)|null)=>{intercept=hook;}};
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
  expect(mobileUsageAccount(key,'primary','session',now).email).toBe('••••••@••••••');expect((await mobileUsageCommand('redeem',id,key,now,native)).message).toContain('confirmation');expect(calls.some(call=>call.method==='provider.consumeResetCredit')).toBe(false);
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

test('Cursor prompts retain ready-provider eligibility, account suppression and selected-environment labels',()=>{
  const summary:Obj={contractVersion:6,sources:[{action:'enableCursorKeychain',fingerprint:{provider:'cursor',hostId:'mac'}}],buckets:[]};
  const a={id:'a',label:'Mac A',connected:true,summary,error:'',canRead:true,canWrite:true,canManage:false,config:{providers:[{driver:'cursor',status:'ready'}]}};
  const input={tab:'usage',window:mobileUsageWindow(30,now),metric:'cost',now,presentations:new Map(),external:false};
  const single=mobileUsagePresentation({...input,sources:[a]},false);
  expect(single.cursor).toEqual([{id:'a',label:'Mac A',usageLabel:'Cursor',buttonLabel:'Enable',disabled:false}]);
  expect(single.hasProviders).toBe(true);expect(single.providers).toEqual([]);expect(single.cursorInsertAt).toBe(0);
  const b={...a,id:'b',label:'Mac B'},multiple=mobileUsagePresentation({...input,sources:[a,b]},false);
  expect(multiple.cursor.map(item=>[item.usageLabel,item.buttonLabel])).toEqual([['Cursor · Mac A','Enable on Mac A'],['Cursor · Mac B','Enable on Mac B']]);
  for(const status of ['error','disabled','missing']){
    const data=mobileUsagePresentation({...input,sources:[{...a,config:{providers:[{driver:'cursor',status}]}}]},false);
    expect(data.cursor).toEqual([]);expect(data.hasProviders).toBe(false);
  }
  for(const source of [{...a,summary:null},{...a,config:{}},{...a,summary:{contractVersion:6,sources:[],buckets:[]}}])
    expect(mobileUsagePresentation({...input,sources:[source]},false).cursor).toEqual([]);
  const oneEligible=mobileUsagePresentation({...input,sources:[a,{...b,config:{}}]},false);
  expect(oneEligible.cursor[0]).toMatchObject({usageLabel:'Cursor · Mac A',buttonLabel:'Enable'});
  for(const source of [{...a,connected:false},{...a,canWrite:false}])expect(mobileUsagePresentation({...input,sources:[source]},false).cursor[0]?.disabled).toBe(true);
  const account={...b,summary:{contractVersion:6,sources:[{fingerprint:{provider:'cursor',hostId:'cursor.com'}}],buckets:[]}};
  expect(mobileUsagePresentation({...input,sources:[a,account]},false).cursor).toEqual([]);
  expect(mobileUsagePresentation({...input,sources:[]},false).hasProviders).toBe(false);
  expect(mobileUsagePresentation(null,false).hasProviders).toBe(false);
});
test('Cursor prompts remain available for incompatible summaries without admitting their usage totals',()=>{
  const config={providers:[{driver:'cursor',status:'ready'}]};
  for(const contractVersion of [3,7])for(const count of [1,2])for(const metric of ['cost','tokens']){
    const sources=Array.from({length:count},(_,index)=>({id:`qa-${index}`,label:`Mac ${index+1}`,connected:true,error:'',canRead:true,canWrite:true,canManage:false,config,
      summary:{contractVersion,sources:[{action:'enableCursorKeychain',fingerprint:{provider:'cursor',hostId:`mac-${index}`}}],
        buckets:[{provider:'cursor',model:'qa',day:'2026-10-07',costUsd:999,totals:{uncachedInputTokens:999}}]}}));
    const data=mobileUsagePresentation({tab:'usage',window:mobileUsageWindow(30,now),metric,now,presentations:new Map(),external:false,sources},false);
    expect(data.hasUsage).toBe(false);expect(data.hasProviders).toBe(true);expect(data.providers).toEqual([]);expect(data.cursorInsertAt).toBe(0);
    expect(data.cursor.map(item=>[item.id,item.usageLabel,item.buttonLabel,item.disabled])).toEqual(count===1
      ?[['qa-0','Cursor','Enable',false]]:[['qa-0','Cursor · Mac 1','Enable on Mac 1',false],['qa-1','Cursor · Mac 2','Enable on Mac 2',false]]);
    expect([data.total,data.headlineValue]).toEqual(['','']);expect(data.models).toEqual([]);expect(data.hasCost).toBe(false);
    expect(data.chartLegend).toEqual([]);expect(data.chartHasActivity).toBe(false);expect(data.totals.map(item=>item.value)).toEqual(['0','$0.00','0','0','0','0.0%']);
  }
  const empty=mobileUsagePresentation({tab:'usage',window:mobileUsageWindow(30,now),metric:'cost',now,presentations:new Map(),external:false,
    sources:[{id:'qa',label:'QA',connected:true,error:'',canRead:true,canWrite:true,canManage:false,config,summary:{contractVersion:6,sources:[],buckets:[]}}]},false);
  expect(empty.hasUsage).toBe(true);expect(empty.hasProviders).toBe(false);expect(empty.cursor).toEqual([]);expect(empty.providers).toEqual([]);
});
test('Cursor insertion follows Codex and Claude under both Usage metrics and pooled Limits order',()=>{
  const summary:Obj={contractVersion:6,sources:['grok','claude','codex'].map(provider=>({status:'ok',fingerprint:{provider,hostId:'mac',resolvedHomePath:`/qa/${provider}`}})),
    buckets:[['grok',9,5],['claude',1,100],['codex',4,10]].map(([provider,costUsd,tokens])=>({provider,model:'qa',day:'2026-10-07',costUsd,totals:{uncachedInputTokens:tokens}}))};
  summary.sources=[...summary.sources as Obj[],{action:'enableCursorKeychain',fingerprint:{provider:'cursor',hostId:'mac'}}];
  const providers=['grok','claudeAgent','codex'].map((driver,index)=>({...nativeAccount(limits,`${index}@qa.test`),instanceId:driver,driver}));
  const config={providers:[...providers,{driver:'cursor',status:'ready'}]},source={id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:true,canManage:false,config};
  const presentations:LimitPresentations=new Map([['qa',{entry:{target:{label:'QA'}},serverConfig:mobileUsageConfig(config)}]]);
  for(const metric of ['cost','tokens'])for(const tab of ['usage','limits']){
    const data=mobileUsagePresentation({tab,window:mobileUsageWindow(30,now),metric,now,presentations,external:false,sources:[source]},false);
    expect(data.providers.map(item=>item.id)).toEqual(metric==='cost'?['grok','codex','claude']:['claude','codex','grok']);
    expect(data.cursorInsertAt).toBe(metric==='cost'?3:2);
    expect(data.pools.map(pool=>pool.id)).toEqual(['grok','claudeAgent','codex']);expect(data.cursorPoolInsertAt).toBe(3);
    expect(data.cursor).toHaveLength(1);expect(data.limitsEmpty).toBe('');
  }
  const noCore={...summary,sources:[{status:'ok',fingerprint:{provider:'grok',hostId:'mac'}},{action:'enableCursorKeychain',fingerprint:{provider:'cursor',hostId:'mac'}}],
    buckets:[{provider:'grok',model:'qa',day:'2026-10-07',costUsd:1,totals:{uncachedInputTokens:1}}]};
  const first=mobileUsagePresentation({tab:'limits',window:mobileUsageWindow(30,now),metric:'cost',now,presentations:new Map(),external:false,sources:[{...source,summary:noCore}]},false);
  expect(first.providers.map(item=>item.id)).toEqual(['grok']);expect(first.cursorInsertAt).toBe(0);expect(first.cursorPoolInsertAt).toBe(0);expect(first.limitsEmpty).toBe('');
  const noPrompt=mobileUsagePresentation({tab:'limits',window:mobileUsageWindow(30,now),metric:'cost',now,presentations:new Map(),external:false,sources:[{...source,summary:{contractVersion:6,sources:[],buckets:[]}}]},false);
  expect(noPrompt.hasProviders).toBe(false);expect(noPrompt.cursor).toEqual([]);expect(noPrompt.limitsEmpty).toBe('No subscription limits reported.');
});
test('Cursor command pending disables its button, rejects duplicates and clears after existing refresh work',async()=>{
  const fixture=transport(['diagnostics:read','settings:write']);
  mobileClient.config={settings:{},providers:[{driver:'cursor',status:'ready'}]};
  let enabled=false;
  const prompt:Obj={contractVersion:6,readAt:'2026-10-07T00:00:00Z',sources:[{action:'enableCursorKeychain',fingerprint:{provider:'cursor',hostId:'mac'}}],buckets:[]};
  const native:Native={...fixture.native,async later(input){const request=obj(input),reply=await fixture.native.later(input);
    if(request.method==='server.updateSettings'){enabled=true;return {...reply,value:{cursorKeychainUsageEnabled:true}};}
    if(request.method==='server.getUsageSummary')return {...reply,value:enabled?{...prompt,sources:[{fingerprint:{provider:'cursor',hostId:'cursor.com'}}]}:prompt};return reply;}};
  await mobileUsage('null','usage',windowJSON,'cost',now,native);
  let release!:()=>void,entered!:()=>void;const gate=new Promise<void>(resolve=>{release=resolve;}),started=new Promise<void>(resolve=>{entered=resolve;});
  fixture.intercept(async request=>{if(request.method==='server.updateSettings'){entered();await gate;}});
  const command=mobileUsageCommand('enable-cursor',fixture.id,'',now,native);await started;
  try{
    const shown=mobileUsageSnapshot('null','usage','cost');expect(shown.cursor[0]).toMatchObject({disabled:true,buttonLabel:'Enable',usageLabel:'Cursor'});
    expect((await mobileUsageCommand('enable-cursor',fixture.id,'',now,native)).message).toBe('This usage action is already running.');
    expect(fixture.calls.filter(call=>call.method==='server.updateSettings')).toHaveLength(1);
    expect(fixture.calls.find(call=>call.method==='server.updateSettings')?.payload).toEqual({patch:{cursorKeychainUsageEnabled:true}});
  }finally{release();}
  expect((await command).message).toBe('');expect(mobileUsageSnapshot('null','usage','tokens').cursor[0]?.disabled).toBe(false);
  expect(fixture.calls.filter(call=>call.method==='server.refreshProviders')).toHaveLength(1);expect(fixture.calls.filter(call=>call.method==='server.getUsageSummary')).toHaveLength(2);
  await mobileUsage('null','usage',windowJSON,'cost',now,native);expect(mobileUsageSnapshot('null','usage','cost').cursor).toEqual([]);
});
test('Cursor enable rechecks write permission and preserves failure reporting without refresh on failed writes',async()=>{
  const fixture=transport(['diagnostics:read','settings:write']);mobileClient.config={settings:{},providers:[{driver:'cursor',status:'ready'}]};let failWrite=false;
  const prompt={contractVersion:6,sources:[{action:'enableCursorKeychain',fingerprint:{provider:'cursor',hostId:'mac'}}],buckets:[]};
  const native:Native={...fixture.native,async later(input){const request=obj(input),reply=await fixture.native.later(input);
    if(request.method==='server.getUsageSummary')return {...reply,value:prompt};
    if(request.method==='server.updateSettings'&&failWrite)return {ok:false,generation:mobileClient.generation,error:{kind:'server',message:'Keychain denied'}};return reply;}};
  await mobileUsage('null','usage',windowJSON,'cost',now,native);fixture.permissions.splice(fixture.permissions.indexOf('settings:write'),1);
  expect((await mobileUsageCommand('enable-cursor',fixture.id,'',now,native)).message).toContain('permission');
  expect(fixture.calls.some(call=>call.method==='server.updateSettings')).toBe(false);expect(mobileUsageSnapshot('null','usage','cost').cursor[0]?.disabled).toBe(false);
  fixture.permissions.push('settings:write');failWrite=true;
  expect((await mobileUsageCommand('enable-cursor',fixture.id,'',now,native)).message).toBe('Keychain denied');
  expect(mobileUsageSnapshot('null','usage','cost').cursor[0]?.disabled).toBe(false);expect(fixture.calls.filter(call=>call.method==='server.updateSettings')).toHaveLength(1);
  expect(fixture.calls.some(call=>call.method==='server.refreshProviders')).toBe(false);
});

test('nonempty usage provider rows project only declared UI fields',()=>{
  const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'claude',hostId:'host',resolvedHomePath:'/home'},distinctSessions:1}],
    buckets:[{provider:'claude',model:'model',day:'2026-10-07',costUsd:2,totals:{uncachedInputTokens:1000}}]};
  const contract=readFileSync(new URL('./settings-usage.contract',import.meta.url),'utf8');
  const declared=contract.split('shape UsageProvider\n')[1]?.split('shape UsageValue')[0]??'';
  const keys=[...declared.matchAll(/^  (\w+):/gm)].map(match=>match[1]).sort();
  expect(keys).toHaveLength(8);
  for(const metric of ['cost','tokens']){
    const data=mobileUsagePresentation({tab:'usage',window:mobileUsageWindow(30,now),metric,now,presentations:new Map(),external:false,
      sources:[{id:'a',label:'A',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]});
    expect(data.providers).toHaveLength(1);
    expect(Object.keys(data.providers[0]!).sort()).toEqual(keys);
    expect(data.providers[0]).toMatchObject({id:'claude',label:'Claude Code',value:metric==='cost'?'$2.00':'1K',fraction:1,share:`100.0% of ${metric}`});
  }
});

for(const [zone,expected] of [
  ['America/New_York',['1 AM EDT','1 AM EST']],['UTC',['5 AM','6 AM']],['Asia/Kathmandu',['11 AM','12 PM']],
] as const)test(`hour labels follow the requested ${zone} zone across repeated calls`,()=>{
  const instants=['2026-11-01T05:37:00.000Z','2026-11-01T06:37:00.000Z'];
  expect(instants.map(instant=>formatHourShort(instant,zone))).toEqual([...expected]);
  expect(instants.map(instant=>formatHourShort(instant,'UTC'))).toEqual(['5 AM','6 AM']);
  expect(instants.map(instant=>formatHourShort(instant,zone))).toEqual([...expected]);
});
test('hour labels preserve invalid inputs and reject an unknown requested zone',()=>{
  expect(formatHourShort('invalid','UTC')).toBe('invalid');
  expect(()=>formatHourShort('2026-11-01T05:37:00.000Z','Etc/Unknown')).toThrow(RangeError);
});
test('date/time labels keep local month day and hour without repeated-hour zone suffixes',()=>{
  const instants=['2026-11-01T05:37:00.000Z','2026-11-01T06:37:00.000Z','2026-08-11T17:37:00.000Z'];
  const day=new Intl.DateTimeFormat('en-US',{month:'short',day:'numeric'}),hour=new Intl.DateTimeFormat('en-US',{hour:'numeric'});
  expect(instants.map(formatDateTimeShort)).toEqual(instants.map(instant=>`${day.format(new Date(instant))}, ${hour.format(new Date(instant))}`));
  expect(formatDateTimeShort('invalid')).toBe('invalid');
});
for(const [zone,expected] of [
  ['America/New_York',['1 AM EDT','1 AM EST']],['UTC',['5 AM','6 AM']],
] as const)test(`hourly Usage chart labels retain the captured ${zone} window zone`,()=>{
  const instants=['2026-11-01T05:37:00.000Z','2026-11-01T06:37:00.000Z'];
  const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'codex',hostId:'qa',resolvedHomePath:'/qa'},distinctSessions:1}],
    buckets:instants.map(hourStart=>({provider:'codex',model:'qa',day:'2026-11-01',hourStart,costUsd:1,totals:{uncachedInputTokens:1}}))};
  const input={tab:'usage',metric:'cost',now:Date.parse('2026-11-01T07:37:00.000Z'),presentations:new Map(),external:false,
    sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]};
  const data=mobileUsagePresentation({...input,window:{sinceDay:'2026-11-01',untilDay:'2026-11-01',timeZone:zone,resolution:'hour',sinceTime:instants[0],untilTime:'2026-11-01T07:37:00.000Z'}},false);
  expect(data.hasUsage).toBe(true);expect(data.chart.map(period=>[period.id,period.label])).toEqual(instants.map((instant,index)=>[instant,expected[index]]));
  expect([data.chartStart,data.chartEnd]).toEqual([...expected]);
  const daily=mobileUsagePresentation({...input,window:{sinceDay:'2026-11-01',untilDay:'2026-11-01',timeZone:zone,resolution:'day'}},false);
  expect(daily.chart.map(period=>period.label)).toEqual(['Nov 1']);
  expect([daily.chartStart,daily.chartEnd]).toEqual(['Nov 1','Nov 1']);
});
test('Usage footer keeps daily endpoints and empty presentation separate from category labels',()=>{
  const data=mobileUsagePresentation({tab:'usage',window:{sinceDay:'2026-10-31',untilDay:'2026-11-02',timeZone:'Pacific/Kiritimati',resolution:'day'},
    metric:'cost',now,presentations:new Map(),external:false,sources:[{id:'qa',label:'QA',connected:true,summary:{contractVersion:6,sources:[],buckets:[]},error:'',canRead:true,canWrite:false,canManage:false,config:{}}]},false);
  expect([data.chartStart,data.chartEnd]).toEqual(['Oct 31','Nov 2']);
  expect(data.chart.map(period=>[period.id,period.label])).toEqual([['2026-10-31','Oct 31'],['2026-11-01','Nov 1'],['2026-11-02','Nov 2']]);
  expect(data.chartLegend).toEqual([]);expect(data.total).toBe('$0.00');expect(data.chartHasActivity).toBe(false);
  const empty=mobileUsagePresentation(null,false);
  expect([empty.chartStart,empty.chartEnd,empty.chartLegend,empty.chart,empty.chartHasActivity]).toEqual(['','',[],[],false]);
});
test('Usage footer legend follows merged cost order in both metrics and retains zero providers',()=>{
  const ids=['codex','claude','grok','cursor','opencode','antigravity','unknown'];
  const summary:Obj={contractVersion:6,sources:ids.map(provider=>({status:'ok',fingerprint:{provider,hostId:'qa',resolvedHomePath:`/qa/${provider}`},distinctSessions:provider==='cursor'?0:1})),
    buckets:[['codex',1,100],['claude',2,10],['cursor',0,0],['unknown',3,200]].map(([provider,costUsd,tokens])=>({provider,model:'qa',day:'2026-10-07',costUsd,totals:{uncachedInputTokens:tokens}}))};
  const input={tab:'usage',window:{sinceDay:'2026-10-07',untilDay:'2026-10-07',timeZone:'UTC',resolution:'day' as const},now,presentations:new Map(),external:false,
    sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]};
  const legend=[
    {id:'claude',label:'Claude Code',light:'#d97757',dark:'#d97757'},
    {id:'codex',label:'Codex',light:'#3c3c43',dark:'#e6e6e6'},
    {id:'grok',label:'Grok Build',light:'#52525b',dark:'#a1a1aa'},
    {id:'opencode',label:'OpenCode',light:'#5b9bbd',dark:'#5b9bbd'},
    {id:'antigravity',label:'Antigravity',light:'#8c7bd1',dark:'#8c7bd1'},
    {id:'cursor',label:'Cursor',light:'#8b8b8b',dark:'#8b8b8b'},
  ];
  const contract=readFileSync(new URL('./settings-usage.contract',import.meta.url),'utf8');
  const declared=contract.split('shape UsageLegend\n')[1]?.split('shape UsageProvider')[0]??'';
  const keys=[...declared.matchAll(/^  (\w+):/gm)].map(match=>match[1]).sort();
  expect(keys).toEqual(['dark','id','label','light']);
  for(const metric of ['cost','tokens']){
    const data=mobileUsagePresentation({...input,metric},false);
    expect(data.chartLegend).toEqual(legend);
    for(const item of data.chartLegend)expect(Object.keys(item).sort()).toEqual(keys);
    expect(data.providers.map(item=>item.id)).toEqual(metric==='cost'?['claude','codex','grok','opencode','antigravity','cursor']:['codex','claude','grok','opencode','antigravity','cursor']);
    expect(data.chart[0]?.bands.map(band=>band.id)).toEqual(['antigravity','opencode','cursor','grok','claude','codex']);
    expect(data.chart[0]?.bands.find(band=>band.id==='codex')?.height).toBeCloseTo(metric==='cost'?30:18000/310);
    expect(data.total).toBe(metric==='cost'?'$6.00':'310');
  }
});
test('Usage provider rows rank the selected metric with stable ties and preserve merged legend order',()=>{
  const ids=['grok','claude','codex','opencode','antigravity','cursor'];
  const summary:Obj={contractVersion:6,sources:ids.map(provider=>({status:'ok',fingerprint:{provider,hostId:'qa',resolvedHomePath:`/qa/${provider}`},distinctSessions:provider==='cursor'?0:1})),
    buckets:[['grok',2,10],['claude',2,20],['codex',1,100],['opencode',3,10],['cursor',0,0]].map(([provider,costUsd,tokens])=>({provider,model:'qa',day:'2026-10-07',costUsd,totals:{uncachedInputTokens:tokens}}))};
  const merged=mergeUsage([{id:'qa',label:'QA',summary}]),costOrder=['opencode','grok','claude','codex','antigravity','cursor'],tokenOrder=['codex','claude','opencode','grok','antigravity','cursor'];
  expect(merged.providers.map(provider=>provider.provider)).toEqual(costOrder);
  for(const provider of merged.providers)Object.freeze(provider);
  Object.freeze(merged.providers);
  const before=JSON.stringify(summary),input={tab:'usage',window:{sinceDay:'2026-10-07',untilDay:'2026-10-07',timeZone:'UTC',resolution:'day' as const},now,presentations:new Map(),external:false,
    sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]};
  for(const metric of ['cost','tokens','cost','tokens']){
    const rows=mobileUsageProviderRows(merged.providers,metric==='tokens'),data=mobileUsagePresentation({...input,metric},false);
    expect(rows.map(provider=>provider.id)).toEqual(metric==='cost'?costOrder:tokenOrder);
    expect(data.providers).toEqual(rows);expect(data.chartLegend.map(provider=>provider.id)).toEqual(costOrder);
    expect(merged.providers.map(provider=>provider.provider)).toEqual(costOrder);expect(JSON.stringify(summary)).toBe(before);
    expect(data.providers.find(provider=>provider.id==='antigravity')).toEqual({id:'antigravity',label:'Antigravity',light:'#8c7bd1',dark:'#8c7bd1',value:metric==='cost'?'$0.00':'0',fraction:0,share:`0.0% of ${metric}`,detail:metric==='cost'?'0 tokens':'$0.00'});
    expect(data.providers.find(provider=>provider.id==='cursor')).toEqual({id:'cursor',label:'Cursor',light:'#8b8b8b',dark:'#8b8b8b',value:metric==='cost'?'$0.00':'0',fraction:0,share:`0.0% of ${metric}`,detail:metric==='cost'?'0 tokens':'$0.00'});
  }
});
test('Usage provider presentation retains unrounded fractional shares and metric-specific detail',()=>{
  const summary:Obj={contractVersion:6,sources:['claude','codex','grok'].map(provider=>({status:'ok',fingerprint:{provider,hostId:'qa',resolvedHomePath:`/qa/${provider}`},distinctSessions:1})),
    buckets:[['claude',2,10],['codex',1,100]].map(([provider,costUsd,tokens])=>({provider,model:'qa',day:'2026-10-07',costUsd,totals:{uncachedInputTokens:tokens}}))};
  const merged=mergeUsage([{id:'qa',label:'QA',summary}]),before=JSON.stringify(summary);
  for(const provider of merged.providers)Object.freeze(provider);
  Object.freeze(merged.providers);
  const input={tab:'usage',window:{sinceDay:'2026-10-07',untilDay:'2026-10-07',timeZone:'UTC',resolution:'day' as const},now,presentations:new Map(),external:false,
    sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]};
  for(const metric of ['cost','tokens','cost']){
    const rows=mobileUsageProviderRows(merged.providers,metric==='tokens'),data=mobileUsagePresentation({...input,metric},false);
    expect(data.providers).toEqual(rows);
    expect(rows.map(provider=>[provider.id,provider.fraction,provider.share,provider.detail,provider.value])).toEqual(metric==='cost'?[
      ['claude',2/3,'66.7% of cost','10 tokens','$2.00'],['codex',1/3,'33.3% of cost','100 tokens','$1.00'],['grok',0,'0.0% of cost','0 tokens','$0.00'],
    ]:[
      ['codex',10/11,'90.9% of tokens','$1.00','100'],['claude',1/11,'9.1% of tokens','$2.00','10'],['grok',0,'0.0% of tokens','$0.00','0'],
    ]);
    expect(data.chartLegend.map(provider=>provider.id)).toEqual(['claude','codex','grok']);
    expect(merged.providers.map(provider=>provider.provider)).toEqual(['claude','codex','grok']);expect(JSON.stringify(summary)).toBe(before);
  }
});
test('Usage model presentation keeps provider identity, ranking and actual known-rate detail across metrics',()=>{
  const summary:Obj={contractVersion:6,sources:['claude','codex','grok','opencode'].map(provider=>({status:'ok',fingerprint:{provider,hostId:'qa',resolvedHomePath:`/qa/${provider}`},distinctSessions:1})),
    buckets:[['claude','shared',2,10,1,0],['codex','shared',1,100,1,0],['codex','cost-tie',1,30,1,0],['grok','mystery',0,20,1,1],
      ['opencode','free-a',0,5,1,0],['opencode','free-b',0,5,1,0],['claude','partial',0.5,5,2,1]].map(([provider,model,costUsd,tokens,records,unpricedRecords])=>
      ({provider,model,costUsd,records,unpricedRecords,day:'2026-10-07',sourcePath:`/qa/${provider}`,totals:{uncachedInputTokens:tokens}}))};
  const before=JSON.stringify(summary);
  const input={tab:'usage',window:{sinceDay:'2026-10-07',untilDay:'2026-10-07',timeZone:'UTC',resolution:'day' as const},now,presentations:new Map(),external:false,
    sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]};
  const empty=mobileUsagePresentation({...input,sources:[{...input.sources[0]!,summary:{...summary,buckets:[]}}]},false);
  expect(empty.hasUsage).toBe(true);expect(empty.models).toEqual([]);
  expect(empty.providers.map(provider=>provider.id)).toEqual(['claude','codex','grok','opencode']);
  expect(mobileUsagePresentation(null,false).models).toEqual([]);
  for(const metric of ['cost','tokens','cost']){
    const data=mobileUsagePresentation({...input,metric},false);
    for(const model of data.models)expect(Object.keys(model).sort()).toEqual(['dark','detail','id','label','light','value']);
    expect(data.models.map(model=>[model.id,model.label,model.light,model.dark,model.value,model.detail])).toEqual(metric==='cost'?[
      ['["claude","shared"]','shared','#d97757','#d97757','$2.00','44.4% of cost · 10 tokens'],
      ['["codex","shared"]','shared','#3c3c43','#e6e6e6','$1.00','22.2% of cost · 100 tokens'],
      ['["codex","cost-tie"]','cost-tie','#3c3c43','#e6e6e6','$1.00','22.2% of cost · 30 tokens'],
      ['["claude","partial"]','partial','#d97757','#d97757','$0.50','11.1% of cost · 5 tokens'],
      ['["grok","mystery"]','mystery','#52525b','#a1a1aa','Unpriced','no known rates · 20 tokens'],
      ['["opencode","free-a"]','free-a','#5b9bbd','#5b9bbd','$0.00','0.0% of cost · 5 tokens'],
      ['["opencode","free-b"]','free-b','#5b9bbd','#5b9bbd','$0.00','0.0% of cost · 5 tokens'],
    ]:[
      ['["codex","shared"]','shared','#3c3c43','#e6e6e6','100','57.1% of tokens · $1.00'],
      ['["codex","cost-tie"]','cost-tie','#3c3c43','#e6e6e6','30','17.1% of tokens · $1.00'],
      ['["grok","mystery"]','mystery','#52525b','#a1a1aa','20','11.4% of tokens · no known rates'],
      ['["claude","shared"]','shared','#d97757','#d97757','10','5.7% of tokens · $2.00'],
      ['["claude","partial"]','partial','#d97757','#d97757','5','2.9% of tokens · $0.50'],
      ['["opencode","free-a"]','free-a','#5b9bbd','#5b9bbd','5','2.9% of tokens · $0.00'],
      ['["opencode","free-b"]','free-b','#5b9bbd','#5b9bbd','5','2.9% of tokens · $0.00'],
    ]);
    expect(data.providers.map(provider=>provider.id)).toEqual(metric==='cost'?['claude','codex','grok','opencode']:['codex','grok','claude','opencode']);
    expect(data.chartLegend.map(provider=>provider.id)).toEqual(['claude','codex','grok','opencode']);
    expect(JSON.stringify(summary)).toBe(before);
  }
});
test('Usage headline follows the metric while raw totals and chart window labels remain separate',()=>{
  const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'claude',hostId:'qa',resolvedHomePath:'/qa'},distinctSessions:1234}],
    buckets:[{provider:'claude',model:'qa',day:'2026-10-07',hourStart:'2026-10-07T00:00:00.000Z',costUsd:0.825,totals:{uncachedInputTokens:1234567}}]};
  const input={tab:'usage',now,presentations:new Map(),external:false,sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]};
  for(const resolution of ['day','hour'] as const){
    const window={sinceDay:'2026-10-07',untilDay:resolution==='day'?'2026-10-08':'2026-10-07',timeZone:'UTC',resolution,sinceTime:'2026-10-07T00:00:00.000Z',untilTime:'2026-10-07T02:00:00.000Z'};
    const cost=mobileUsagePresentation({...input,window,metric:'cost'},false),tokens=mobileUsagePresentation({...input,window,metric:'tokens'},false);
    expect([cost.headlineLabel,cost.headlineValue,cost.headlineDetail]).toEqual(['Raw token cost','$0.83*','* if billed at full API rate']);
    expect([tokens.headlineLabel,tokens.headlineValue,tokens.headlineDetail]).toEqual(['Processed tokens','1.23M','Across 1,234 sessions']);
    expect([cost.total,tokens.total]).toEqual(['$0.83','1.23M']);
    expect([cost.chartTitle,tokens.chartTitle]).toEqual(resolution==='day'?['Daily cost','Daily processed tokens']:['Hourly cost','Hourly processed tokens']);
    expect(cost.chart.map(period=>period.id)).toEqual(resolution==='day'?['2026-10-07','2026-10-08']:['2026-10-07T00:00:00.000Z','2026-10-07T01:00:00.000Z']);
    expect(cost.chart.map(period=>({id:period.id,label:period.label}))).toEqual(tokens.chart.map(period=>({id:period.id,label:period.label})));
    expect(cost.chartLegend).toEqual(tokens.chartLegend);expect(cost.chartLegend[0]?.label).toBe('Claude Code');
    expect([cost.providers[0]?.value,cost.models[0]?.value,cost.costByType.find(item=>item.id==='other')?.value,tokens.providers[0]?.detail]).toEqual(['$0.83','$0.83','$0.83','$0.83']);
    expect(cost.chart[0]?.bands.find(band=>band.id==='claude')?.value).toBe('$0.83');
  }
});
test('compatible zero Usage keeps the reference cost note and literal session wording',()=>{
  for(const sessions of [0,1]){
    const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'codex',hostId:'qa',resolvedHomePath:'/qa'},distinctSessions:sessions}],buckets:[]};
    const input={tab:'usage',window:mobileUsageWindow(7,now),now,presentations:new Map(),external:false,
      sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]};
    const cost=mobileUsagePresentation({...input,metric:'cost'},false),tokens=mobileUsagePresentation({...input,metric:'tokens'},false);
    expect(cost.hasUsage).toBe(true);expect(tokens.hasUsage).toBe(true);
    expect([cost.chartHasActivity,tokens.chartHasActivity]).toEqual([false,false]);
    expect([cost.headlineLabel,cost.headlineValue,cost.headlineDetail,cost.total]).toEqual(['Raw token cost','$0.00*','* if billed at full API rate','$0.00']);
    expect([tokens.headlineLabel,tokens.headlineValue,tokens.headlineDetail,tokens.total]).toEqual(['Processed tokens','0',sessions===1?'Across 1 sessions':'Across 0 sessions','0']);
  }
});
test('missing and future Usage summaries cannot publish a headline',()=>{
  const input={tab:'usage',window:mobileUsageWindow(7,now),now,presentations:new Map(),external:false};
  for(const metric of ['cost','tokens'])for(const summary of [null,{contractVersion:7,sources:[{status:'ok',fingerprint:{provider:'codex',hostId:'qa',resolvedHomePath:'/qa'},distinctSessions:1}],
    buckets:[{provider:'codex',model:'qa',day:'2026-10-07',hourStart:'2026-10-07T00:00:00.000Z',costUsd:1,totals:{uncachedInputTokens:5}}]}]){
    const data=mobileUsagePresentation({...input,metric,sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]},false);
    expect([data.hasUsage,data.headlineLabel,data.headlineValue,data.headlineDetail,data.total,data.chartHasActivity]).toEqual([false,'','','','',false]);
  }
  const empty=mobileUsagePresentation(null,false);
  expect([empty.hasUsage,empty.headlineLabel,empty.headlineValue,empty.headlineDetail,empty.total,empty.chartHasActivity]).toEqual([false,'','','','',false]);
});
test('Usage chart activity follows token totals independently of the displayed metric',()=>{
  for(const row of [
    {cost:2,tokens:0,active:false,costText:'$2.00',tokenText:'0',costHeight:180,tokenHeight:0},
    {cost:0,tokens:4,active:true,costText:'$0.00',tokenText:'4',costHeight:0,tokenHeight:180},
  ])for(const resolution of ['day','hour'] as const){
    const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'codex',hostId:'qa',resolvedHomePath:'/qa'},distinctSessions:1}],
      buckets:[{provider:'codex',model:'qa',day:'2026-10-07',hourStart:'2026-10-07T00:00:00.000Z',costUsd:row.cost,totals:{uncachedInputTokens:row.tokens}}]};
    const input={tab:'usage',window:{sinceDay:'2026-10-07',untilDay:'2026-10-07',timeZone:'UTC',resolution,sinceTime:'2026-10-07T00:00:00.000Z',untilTime:'2026-10-07T01:00:00.000Z'},now,presentations:new Map(),external:false,
      sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]};
    for(const metric of ['cost','tokens']){
      const data=mobileUsagePresentation({...input,metric},false);
      expect(data.chartHasActivity).toBe(row.active);expect(data.total).toBe(metric==='cost'?row.costText:row.tokenText);
      expect(data.chart).toHaveLength(1);expect(data.chart[0]?.bands.find(band=>band.id==='codex')?.height).toBe(metric==='cost'?row.costHeight:row.tokenHeight);
      expect([data.chartStart,data.chartEnd]).toEqual(resolution==='day'?['Oct 7','Oct 7']:['12 AM','12 AM']);
      expect(data.chartLegend.map(provider=>provider.id)).toEqual(['codex']);
    }
  }
});
test('hourly Usage activity cannot borrow positive daily-only buckets',()=>{
  const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'codex',hostId:'qa',resolvedHomePath:'/qa'},distinctSessions:1}],
    buckets:[{provider:'codex',model:'qa',day:'2026-10-07',costUsd:1,totals:{uncachedInputTokens:5}}]};
  const input={tab:'usage',now,presentations:new Map(),external:false,sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]};
  for(const metric of ['cost','tokens']){
    const window={sinceDay:'2026-10-07',untilDay:'2026-10-07',timeZone:'UTC',sinceTime:'2026-10-07T00:00:00.000Z',untilTime:'2026-10-07T01:00:00.000Z'};
    const daily=mobileUsagePresentation({...input,metric,window:{...window,resolution:'day'}},false),hourly=mobileUsagePresentation({...input,metric,window:{...window,resolution:'hour'}},false);
    expect([daily.chartHasActivity,hourly.chartHasActivity]).toEqual([true,false]);
    expect(hourly.total).toBe(daily.total);expect(hourly.headlineValue).toBe(daily.headlineValue);expect(hourly.chartLegend).toEqual(daily.chartLegend);
    expect(hourly.chart[0]?.bands.every(band=>band.height===0)).toBe(true);
  }
});
test('Usage Totals keep six metric-independent cells and average only active selected periods',()=>{
  const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'codex',hostId:'qa',resolvedHomePath:'/qa'},distinctSessions:3}],
    buckets:[
      {provider:'codex',model:'qa',day:'2026-10-06',hourStart:'2026-10-06T23:00:00.000Z',costUsd:2,cacheSavingsUsd:2,records:4,unpricedRecords:1,
        totals:{uncachedInputTokens:1000,cachedInputTokens:1000,cacheCreationTokens:200,outputTokens:300,reasoningTokens:120}},
      {provider:'codex',model:'qa',day:'2026-10-07',hourStart:'2026-10-07T00:00:00.000Z',costUsd:1,cacheSavingsUsd:2,records:2,unpricedRecords:1,
        totals:{uncachedInputTokens:0,cachedInputTokens:500,cacheCreationTokens:100,outputTokens:100,reasoningTokens:40}},
      {provider:'codex',model:'qa',day:'2026-10-07',hourStart:'2026-10-07T01:00:00.000Z',costUsd:1,cacheSavingsUsd:2,records:2,unpricedRecords:0,
        totals:{uncachedInputTokens:0,cachedInputTokens:500,cacheCreationTokens:200,outputTokens:100,reasoningTokens:40}},
      {provider:'codex',model:'qa',day:'2026-10-07',hourStart:'2026-10-07T02:00:00.000Z',costUsd:0,totals:{}},
    ]};
  const before=JSON.stringify(summary),sources=[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}];
  const merged=mergeUsage(sources.map(source=>({id:source.id,label:source.label,summary:source.summary})));
  expect([merged.totalTokens,merged.outputTokens,merged.reasoningTokens,merged.records]).toEqual([4000,500,200,8]);
  expect(merged.daily.filter(period=>period.totalTokens>0)).toHaveLength(2);
  expect(merged.hourly.filter(period=>period.totalTokens>0)).toHaveLength(3);
  for(const resolution of ['day','hour'] as const)for(const metric of ['cost','tokens','cost']){
    const data=mobileUsagePresentation({tab:'usage',window:{sinceDay:'2026-10-06',untilDay:'2026-10-07',timeZone:'UTC',resolution,
      sinceTime:'2026-10-06T12:00:00.000Z',untilTime:'2026-10-07T12:00:00.000Z'},metric,now,presentations:new Map(),external:false,sources},false);
    expect(data.totals).toEqual([
      {id:'tokens',label:'Processed tokens',value:'4K',detail:resolution==='hour'?'1.33K per active hour':'2K per active day'},
      {id:'cache-savings',label:'Cache savings',value:'$6.00',detail:'1.5x the raw cost'},
      {id:'cache-read',label:'Cached input',value:'2K',detail:'66.7% of observed input'},
      {id:'input',label:'Uncached input',value:'1K',detail:'500 cache writes'},
      {id:'output',label:'Output',value:'500',detail:'incl. 200 reasoning'},
      {id:'unpriced',label:'Unpriced',value:'25.0%',detail:'of records, excluded from cost'},
    ]);
    expect(data.total).toBe(metric==='cost'?'$4.00':'4K');expect(data.headlineDetail).toBe(metric==='cost'?'* if billed at full API rate':'Across 3 sessions');
    expect(data.chartHasActivity).toBe(true);expect(data.chartLegend.map(item=>item.id)).toEqual(['codex']);
    expect(data.providers[0]?.value).toBe(data.total);expect(data.models[0]?.value).toBe(data.total);
    expect(data.hasCost).toBe(true);expect(data.costByType.map(item=>[item.id,item.weight])).toEqual([['other',4]]);
    expect(data.costBySpeed).toEqual([]);expect(JSON.stringify(summary)).toBe(before);
  }
});
test('Usage Totals handle zero observed input and nonpositive cost without changing cache-write tokens',()=>{
  for(const costUsd of [0,-1])for(const metric of ['cost','tokens']){
    const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'codex',hostId:'qa',resolvedHomePath:'/qa'}}],
      buckets:[{provider:'codex',model:'qa',day:'2026-10-07',costUsd,cacheSavingsUsd:2,records:0,unpricedRecords:0,totals:{cacheCreationTokens:20}}]};
    const data=mobileUsagePresentation({tab:'usage',window:mobileUsageWindow(7,now),metric,now,presentations:new Map(),external:false,
      sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]},false);
    expect(data.totals.map(item=>[item.label,item.value,item.detail])).toEqual([
      ['Processed tokens','20','20 per active day'],['Cache savings','$2.00','vs full input rates'],
      ['Cached input','0','0.0% of observed input'],['Uncached input','0','20 cache writes'],
      ['Output','0','incl. 0 reasoning'],['Unpriced','0.0%','of records, excluded from cost'],
    ]);
  }
});
test('Usage Totals exclude duplicate, unowned and future reasoning without double-counting output',()=>{
  const source={status:'ok',fingerprint:{provider:'codex',hostId:'shared',resolvedHomePath:'/qa'},distinctSessions:2};
  const bucket={provider:'codex',sourcePath:'/qa',model:'qa',day:'2026-10-07',hourStart:'2026-10-07T01:00:00.000Z',costUsd:1,records:2,unpricedRecords:1,
    totals:{uncachedInputTokens:20,outputTokens:80,reasoningTokens:30}};
  const owned:Obj={contractVersion:6,readAt:'2026-10-07T02:00:00Z',sources:[source],buckets:[bucket,
    {...bucket,sourcePath:'/unowned',totals:{outputTokens:900,reasoningTokens:900}},
    {...bucket,provider:'claude',totals:{outputTokens:900,reasoningTokens:900}}]};
  const duplicate:Obj={contractVersion:6,readAt:'2026-10-07T01:00:00Z',sources:[source],buckets:[{...bucket,totals:{outputTokens:900,reasoningTokens:900}}]};
  const future:Obj={...owned,contractVersion:7,readAt:'2026-10-07T03:00:00Z'};
  const environments=[{id:'old',label:'Old',summary:duplicate},{id:'qa',label:'QA',summary:owned},{id:'future',label:'Future',summary:future}],before=JSON.stringify(environments);
  const merged=mergeUsage(environments);
  expect([merged.totalTokens,merged.outputTokens,merged.reasoningTokens,merged.records,merged.sessions]).toEqual([100,80,30,2,2]);
  expect(merged.duplicates).toEqual(['Old: /qa']);expect(merged.mismatches).toEqual([{environment:'Future',direction:'clientBehind'}]);
  for(const metric of ['cost','tokens']){
    const data=mobileUsagePresentation({tab:'usage',window:mobileUsageWindow(7,now),metric,now,presentations:new Map(),external:false,
      sources:environments.map(item=>({...item,connected:true,error:'',canRead:true,canWrite:false,canManage:false,config:{}}))},false);
    expect(data.totals[0]).toMatchObject({value:'100',detail:'100 per active day'});
    expect(data.totals[4]).toMatchObject({value:'80',detail:'incl. 30 reasoning'});
    expect(data.totals[5]).toMatchObject({value:'50.0%',detail:'of records, excluded from cost'});
    expect(data.providers.map(item=>item.id)).toEqual(['codex']);expect(data.models.map(item=>item.id)).toEqual(['["codex","qa"]']);
  }
  expect(JSON.stringify(environments)).toBe(before);
});
test('Usage Totals keep zero average and reasoning for inactive, absent and incompatible summaries',()=>{
  const compatible:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'codex',hostId:'qa',resolvedHomePath:'/qa'},distinctSessions:1}],buckets:[]};
  for(const resolution of ['day','hour'] as const)for(const summary of [compatible,{...compatible,contractVersion:7}]){
    const data=mobileUsagePresentation({tab:'usage',window:{sinceDay:'2026-10-06',untilDay:'2026-10-07',timeZone:'UTC',resolution,
      sinceTime:'2026-10-06T12:00:00.000Z',untilTime:'2026-10-07T12:00:00.000Z'},metric:'tokens',now,presentations:new Map(),external:false,
      sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]},false);
    expect(data.hasUsage).toBe(summary.contractVersion===6);expect(data.totals.map(item=>item.value)).toEqual(['0','$0.00','0','0','0','0.0%']);
    expect(data.totals[0]?.detail).toBe(`0 per active ${resolution==='hour'?'hour':'day'}`);
    expect(data.totals[1]?.detail).toBe('vs full input rates');expect(data.totals[4]?.detail).toBe('incl. 0 reasoning');
  }
  const empty=mobileUsagePresentation(null,false);
  expect(empty.hasUsage).toBe(false);expect(empty.totals.map(item=>item.value)).toEqual(['0','$0.00','0','0','0','0.0%']);
});

test('Usage Cost bars retain raw dollar weights, neutral colors and premium across metrics',()=>{
  const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'claude',hostId:'qa',resolvedHomePath:'/qa'},distinctSessions:1}],
    buckets:[{provider:'claude',model:'qa',day:'2026-10-07',costUsd:10,records:1,unpricedRecords:0,totals:{uncachedInputTokens:100},
      categoryCostUsd:{input:1,cacheRead:2,cacheWrite:3,output:1},fastCostUsd:2,ultrafastCostUsd:1,speedPremiumUsd:0.825}]};
  const before=JSON.stringify(summary),input={tab:'usage',window:{sinceDay:'2026-10-07',untilDay:'2026-10-07',timeZone:'UTC',resolution:'day' as const},now,
    presentations:new Map(),external:false,sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]};
  for(const metric of ['cost','tokens','cost']){
    const data=mobileUsagePresentation({...input,metric},false);
    expect(data.hasCost).toBe(true);expect(data.costPremium).toBe('$0.83 premium');
    for(const item of [...data.costByType,...data.costBySpeed])expect(Object.keys(item).sort()).toEqual(['dark','id','label','light','value','weight']);
    expect(data.costByType.map(item=>[item.id,item.label,item.weight,item.value,item.light,item.dark])).toEqual([
      ['input','Input',1,'$1.00','#848484','#737373'],['cacheRead','Cache read',2,'$2.00','#c0c0c0','#282828'],
      ['cacheWrite','Cache write',3,'$3.00','#6d6d6d','#949494'],['output','Output',1,'$1.00','#3c3c43','#e6e6e6'],['other','Other',3,'$3.00','#a3a3a3','#494949'],
    ]);
    expect(data.costBySpeed.map(item=>[item.id,item.label,item.weight,item.value,item.light,item.dark])).toEqual([
      ['standard','Standard',7,'$7.00','#b8b8b8','#313131'],['fast','Fast',2,'$2.00','#797979','#838383'],['ultrafast','Ultrafast',1,'$1.00','#3c3c43','#e6e6e6'],
    ]);
    expect([data.total,data.providers[0]?.value,data.models[0]?.value]).toEqual(Array(3).fill(metric==='cost'?'$10.00':'100'));
    expect(data.models[0]?.id).toBe('["claude","qa"]');expect(data.chartLegend.map(item=>item.id)).toEqual(['claude']);
    expect(data.models[0]?.detail).toBe(metric==='cost'?'100.0% of cost · 100 tokens':'100.0% of tokens · $10.00');
    expect(JSON.stringify(summary)).toBe(before);
  }
});
test('Usage Cost visibility keeps tiny positive sections, filters empty segments and gates speed independently',()=>{
  const cases:[string,Obj,boolean,[string,number,string][],[string,number,string][]][]=[
    ['zero',{costUsd:0},false,[],[]],['negative',{costUsd:-1},false,[],[]],
    ['Other below threshold',{costUsd:0.0049},true,[],[]],['Other at threshold',{costUsd:0.005},true,[['other',0.005,'$0.01']],[]],
    ['tiny split weights',{costUsd:0.003,categoryCostUsd:{input:0.001,cacheRead:0.002,cacheWrite:0,output:0}},true,[['input',0.001,'$0.00'],['cacheRead',0.002,'$0.00']],[]],
    ['speed without type',{costUsd:0.0049,fastCostUsd:0.0049},true,[],[['fast',0.0049,'$0.00']]],
    ['standard only',{costUsd:1,categoryCostUsd:{input:1}},true,[['input',1,'$1.00']],[]],
    ['ultrafast only',{costUsd:1,categoryCostUsd:{input:1},ultrafastCostUsd:0.5},true,[['input',1,'$1.00']],[['standard',0.5,'$0.50'],['ultrafast',0.5,'$0.50']]],
  ];
  for(const [label,bucket,hasCost,types,speeds] of cases)for(const metric of ['cost','tokens']){
    const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'claude',hostId:'qa',resolvedHomePath:'/qa'}}],
      buckets:[{provider:'claude',model:'qa',day:'2026-10-07',totals:{uncachedInputTokens:1},...bucket}]};
    const data=mobileUsagePresentation({tab:'usage',window:mobileUsageWindow(7,now),metric,now,presentations:new Map(),external:false,
      sources:[{id:'qa',label:'QA',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]},false);
    expect({case:label,visible:data.hasCost,types:data.costByType.map(item=>[item.id,item.weight,item.value]),speeds:data.costBySpeed.map(item=>[item.id,item.weight,item.value])})
      .toEqual({case:label,visible:hasCost,types,speeds});
    expect(data.costPremium).toBe('$0.00 premium');
  }
  expect(mobileUsagePresentation(null,false).hasCost).toBe(false);
  const future=mobileUsagePresentation({tab:'usage',window:mobileUsageWindow(7,now),metric:'cost',now,presentations:new Map(),external:false,
    sources:[{id:'qa',label:'QA',connected:true,summary:{contractVersion:7,costUsd:10,buckets:[]},error:'',canRead:true,canWrite:false,canManage:false,config:{}}]},false);
  expect([future.hasCost,future.costByType,future.costBySpeed]).toEqual([false,[],[]]);
});
test('mobile USD formatting keeps reference rounding, signed zero, grouping and nonfinite output',()=>{
  expect([0.825,1.275,-0.825,-0.004,-0,0,1_234_567.891,NaN,Infinity,-Infinity].map(formatUsd))
    .toEqual(['$0.83','$1.28','-$0.83','-$0.00','-$0.00','$0.00','$1,234,567.89','$NaN','$∞','-$∞']);
});
for(const [label,value,expected,hasCost] of [
  ['half-cent',0.825,'$0.83',true],['second half-cent',1.275,'$1.28',true],['negative half-cent',-0.825,'-$0.83',false],
  ['negative rounds to zero',-0.004,'-$0.00',false],['zero',0,'$0.00',false],['grouping',1_234_567.891,'$1,234,567.89',true],
  ['NaN',NaN,'$0.00',false],['positive infinity',Infinity,'$0.00',false],['negative infinity',-Infinity,'$0.00',false],
] as const)test(`Usage projection formats ${label} across cost fields and token details`,()=>{
  const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'claude',hostId:'host',resolvedHomePath:'/home'},distinctSessions:1}],
    buckets:[{provider:'claude',model:'model',day:'2026-10-07',costUsd:value,cacheSavingsUsd:value,categoryCostUsd:{input:value},fastCostUsd:value,totals:{uncachedInputTokens:1000}}]};
  const input={tab:'usage',window:mobileUsageWindow(30,now),now,presentations:new Map(),external:false,
    sources:[{id:'a',label:'A',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]};
  const cost=mobileUsagePresentation({...input,metric:'cost'},false),tokens=mobileUsagePresentation({...input,metric:'tokens'},false);
  expect([cost.total,cost.providers[0]?.value,cost.models[0]?.value,cost.chart.find(period=>period.id==='2026-10-07')?.bands.find(band=>band.id==='claude')?.value,
    tokens.providers[0]?.detail]).toEqual(Array(5).fill(expected));
  expect(cost.hasCost).toBe(hasCost);expect(tokens.hasCost).toBe(hasCost);
  expect(cost.costByType.map(item=>item.value)).toEqual(hasCost?[expected]:[]);
  expect(cost.costBySpeed.map(item=>item.value)).toEqual(hasCost?[expected]:[]);
});

function refreshBarrier(){
  let enter!:()=>void,release!:()=>void;
  const entered=new Promise<void>(resolve=>{enter=resolve;}),closed=new Promise<void>(resolve=>{release=resolve;});
  return {entered,release,wait:async()=>{enter();await closed;}};
}
async function openUsage(native:Native,tab='usage',days='1'){
  await mobileAutomationPrepare('settings','{}','','','',now,native);
  await mobileAutomationPrepare('settingsUsage','{}','','','',now,native);
  await mobileAutomationCommand('usage-local','tab',tab,'',now,native);
  await mobileAutomationCommand('usage-local','days',days,'',now,native);
  await mobileAutomationPrepare('settingsUsage','{}','','','',now,native);
}
test('pull refresh availability follows diagnostics permission rather than summary availability',async()=>{
  const fixture=transport([]);await openUsage(fixture.native);
  expect(mobileAutomationSnapshot().usage.canRefresh).toBe(false);
  const count=fixture.calls.length,result=await mobileAutomationCommand('usage-refresh','','','',now+60_000,fixture.native);
  expect(result.message).toContain('cannot read usage');expect(fixture.calls).toHaveLength(count);
  await mobileAutomationCommand('usage-local','tab','limits','',now,fixture.native);
  expect(mobileAutomationSnapshot().usage.canRefresh).toBe(true);
  const allowed=mobileUsagePresentation({tab:'usage',window:mobileUsageWindow(1,now),metric:'cost',now,presentations:new Map(),external:false,
    sources:[{id:'allowed',label:'Allowed',connected:true,summary:null,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]});
  expect(allowed.hasUsage).toBe(false);expect(allowed.canRefresh).toBe(true);
});
test('Usage pull uses the new hourly window, deduplicates pulls and stays busy through the final read',async()=>{
  const fixture=transport();await openUsage(fixture.native);fixture.calls.length=0;
  const gate=refreshBarrier();let summaryRead=false;
  fixture.intercept(async request=>{if(request.method==='server.getUsageSummary')summaryRead=true;else if(request.op==='environments'&&summaryRead)await gate.wait();});
  const refreshedAt=now+90_000,pending=mobileAutomationCommand('usage-refresh','','','',refreshedAt,fixture.native);await gate.entered;
  try{
    expect(mobileAutomationSnapshot(true).usage.refreshing).toBe(true);
    expect(mobileAutomationSnapshot(false).usage.refreshing).toBe(false);
    const count=fixture.calls.length,duplicate=await mobileAutomationCommand('usage-refresh','','','',refreshedAt+1,fixture.native);
    expect(duplicate.refresh).toBe(false);expect(fixture.calls).toHaveLength(count);
    expect(fixture.calls.filter(call=>call.method==='server.refreshUsageRates')).toHaveLength(1);
    expect(fixture.calls.filter(call=>call.method==='server.getUsageSummary').map(call=>call.payload)).toEqual([mobileUsageWindow(1,refreshedAt)]);
  }finally{gate.release();}
  const result=await pending;expect(result.message).toBe('');expect(result.refresh).toBe(false);
  expect(mobileAutomationSnapshot().now).toBe(refreshedAt);expect(mobileAutomationSnapshot(true).usage.refreshing).toBe(false);
});
test('Limits pull probes providers and keeps the selected Usage window',async()=>{
  const fixture=transport([]);await openUsage(fixture.native,'limits','7');fixture.calls.length=0;
  const result=await mobileAutomationCommand('usage-refresh','','','',now+3_600_000,fixture.native);
  expect(result.message).toBe('');expect(result.refresh).toBe(false);expect(mobileAutomationSnapshot().now).toBe(now);
  expect(fixture.calls.filter(call=>call.method==='server.refreshProviders')).toHaveLength(1);
  expect(fixture.calls.some(call=>call.method==='server.refreshUsageRates'||call.method==='server.getUsageSummary')).toBe(false);
});
test('failed rate refresh still reads the selected window and settles the pull',async()=>{
  const fixture=transport();await openUsage(fixture.native);fixture.failRates();fixture.calls.length=0;
  const result=await mobileAutomationCommand('usage-refresh','','','',now+60_000,fixture.native);
  expect(result.message).toContain('Rates offline');expect(mobileAutomationSnapshot(true).usage.refreshing).toBe(false);
  expect(fixture.calls.filter(call=>call.method==='server.getUsageSummary')).toHaveLength(1);
});
for(const change of ['departure','tab','days','environment'] as const)test(`held pull retires its result after ${change}`,async()=>{
  const fixture=transport();await openUsage(fixture.native);fixture.calls.length=0;
  const gate=refreshBarrier();fixture.intercept(async request=>{if(request.method==='server.refreshUsageRates')await gate.wait();});
  const pending=mobileAutomationCommand('usage-refresh','','','',now+60_000,fixture.native);await gate.entered;
  try{
    if(change==='departure')await mobileAutomationPrepare('settings','{}','','','',now+120_000,fixture.native);
    else if(change==='tab')await mobileAutomationCommand('usage-local','tab','limits','',now+120_000,fixture.native);
    else if(change==='days')await mobileAutomationCommand('usage-local','days','7','',now+120_000,fixture.native);
    else{
      const chooser:Native={...fixture.native,async later(input){if(obj(input).op==='mobileScheduledMenu')return {ok:true,generation:mobileClient.generation,value:{choice:JSON.stringify(['environment',fixture.id])}};return fixture.native.later(input);}};
      await mobileAutomationCommand('usage-environments','','','',now+120_000,chooser);
    }
    expect(mobileAutomationSnapshot(true).usage.refreshing).toBe(false);
  }finally{gate.release();}
  const result=await pending;expect(result.message).toBe('');expect(result.refresh).toBe(false);
  expect(fixture.calls.some(call=>call.method==='server.getUsageSummary')).toBe(false);
  const snapshot=mobileAutomationSnapshot();
  if(change==='departure'){expect(snapshot.route).toBe('settings');expect(snapshot.usage.ready).toBe(false);await mobileAutomationPrepare('settingsUsage','{}','','','',now+180_000,fixture.native);expect(mobileAutomationSnapshot().now).toBe(now+180_000);}
  else if(change==='tab')expect(snapshot.tab).toBe('limits');
  else if(change==='days'){expect(snapshot.days).toBe(7);expect(snapshot.now).toBe(now+120_000);}
  else expect(snapshot.selection).toBe('[]');
});
test('native cancellation releases the pull owner without a follow-up read',async()=>{
  const fixture=transport();await openUsage(fixture.native);fixture.calls.length=0;
  fixture.intercept(async request=>{if(request.method==='server.refreshUsageRates')throw new ClientError('cancelled','superseded');});
  await expect(mobileAutomationCommand('usage-refresh','','','',now+60_000,fixture.native)).rejects.toMatchObject({kind:'superseded'});
  expect(mobileAutomationSnapshot(true).usage.refreshing).toBe(false);expect(fixture.calls.some(call=>call.method==='server.getUsageSummary')).toBe(false);
});

test('a held final refresh read cannot replace a newer selected window',async()=>{
  const fixture=transport();await openUsage(fixture.native);fixture.calls.length=0;
  const gate=refreshBarrier();let summaryRead=false,held=false;
  fixture.intercept(async request=>{if(request.method==='server.getUsageSummary')summaryRead=true;else if(request.op==='environments'&&summaryRead&&!held){held=true;await gate.wait();}});
  const pending=mobileAutomationCommand('usage-refresh','','','',now+60_000,fixture.native);await gate.entered;
  try{
    await mobileAutomationCommand('usage-local','days','7','',now+120_000,fixture.native);
    await mobileAutomationPrepare('settingsUsage','{}','','','',now+120_000,fixture.native);
    expect(mobileAutomationSnapshot().days).toBe(7);expect(mobileAutomationSnapshot().usage.chartTitle).toBe('Daily cost');
  }finally{gate.release();}
  const result=await pending;expect(result.refresh).toBe(false);expect(result.message).toBe('');
  expect(mobileAutomationSnapshot().now).toBe(now+120_000);expect(mobileAutomationSnapshot().usage.chartTitle).toBe('Daily cost');
});

for(const transition of ['same-minute range','tab roundtrip'] as const)test(`current summary waiter survives a retired writer after ${transition}`,async()=>{
  const fixture=transport();await openUsage(fixture.native);fixture.calls.length=0;
  const gate=refreshBarrier();let held=false;
  fixture.intercept(async request=>{if(request.method==='server.getUsageSummary'&&!held){held=true;await gate.wait();}});
  const refreshedAt=now+90_000,refresh=mobileAutomationCommand('usage-refresh','','','',refreshedAt,fixture.native);await gate.entered;
  let load:Promise<unknown>|undefined,settled=false;
  try{
    if(transition==='same-minute range')await mobileAutomationCommand('usage-local','days','1','',refreshedAt+5_000,fixture.native);
    else{
      await mobileAutomationCommand('usage-local','tab','limits','',refreshedAt+5_000,fixture.native);
      await mobileAutomationCommand('usage-local','tab','usage','',refreshedAt+5_000,fixture.native);
    }
    load=mobileAutomationPrepare('settingsUsage','{}','','','',refreshedAt+5_000,fixture.native).then(value=>{settled=true;return value;});
    await new Promise<void>(resolve=>setImmediate(resolve));
    expect(settled).toBe(false);
    expect(fixture.calls.filter(call=>call.method==='server.getUsageSummary')).toHaveLength(1);
  }finally{gate.release();}
  await Promise.all([refresh,load]);
  const snapshot=mobileAutomationSnapshot();expect(snapshot.usage.ready).toBe(true);expect(snapshot.usage.hasUsage).toBe(true);
  expect(snapshot.usage.environments[0]?.status).toBe('');expect(snapshot.tab).toBe('usage');
  expect(fixture.calls.filter(call=>call.method==='server.getUsageSummary')).toHaveLength(1);
});
for(const rejectRetry of [false,true])test(`current summary waiter retries a cancelled producer only once, retry rejects=${rejectRetry}`,async()=>{
  const fixture=transport();await openUsage(fixture.native);fixture.calls.length=0;
  const gate=refreshBarrier();let summaries=0;
  fixture.intercept(async request=>{if(request.method==='server.getUsageSummary'){summaries++;if(summaries===1){await gate.wait();throw new ClientError('old answer cancelled','superseded');}if(rejectRetry)throw new ClientError('new answer cancelled','superseded');}});
  const refreshedAt=now+90_000,refresh=mobileAutomationCommand('usage-refresh','','','',refreshedAt,fixture.native).catch(error=>error);await gate.entered;
  let load:Promise<unknown>|undefined;
  try{
    await mobileAutomationCommand('usage-local','days','1','',refreshedAt+5_000,fixture.native);
    load=mobileAutomationPrepare('settingsUsage','{}','','','',refreshedAt+5_000,fixture.native).catch(error=>error);
    await new Promise<void>(resolve=>setImmediate(resolve));expect(summaries).toBe(1);
  }finally{gate.release();}
  const [oldResult,newResult]=await Promise.all([refresh,load]);
  expect(oldResult).toMatchObject({kind:'superseded'});expect(summaries).toBe(2);
  if(rejectRetry)expect(newResult).toMatchObject({kind:'superseded'});
  else{expect(newResult).not.toBeInstanceOf(Error);expect(mobileAutomationSnapshot().usage.hasUsage).toBe(true);}
});
test('a current summary waiter reports a shared network failure without retrying it',async()=>{
  const fixture=transport();await openUsage(fixture.native);fixture.calls.length=0;
  const gate=refreshBarrier();let summaries=0;
  fixture.intercept(async request=>{if(request.method==='server.getUsageSummary'){summaries++;await gate.wait();throw new Error('Summary offline');}});
  const refreshedAt=now+90_000,refresh=mobileAutomationCommand('usage-refresh','','','',refreshedAt,fixture.native);await gate.entered;
  let load:Promise<unknown>|undefined;
  try{
    await mobileAutomationCommand('usage-local','days','1','',refreshedAt+5_000,fixture.native);
    load=mobileAutomationPrepare('settingsUsage','{}','','','',refreshedAt+5_000,fixture.native);
    await new Promise<void>(resolve=>setImmediate(resolve));
  }finally{gate.release();}
  await Promise.all([refresh,load]);expect(summaries).toBe(1);
  expect(mobileAutomationSnapshot().usage.environments[0]?.status).toContain('Summary offline');
});
test('a new environment selection cannot refresh the previous prepared sources while loading',async()=>{
  const fixture=transport();await openUsage(fixture.native);
  const chooser:Native={...fixture.native,async later(input){if(obj(input).op==='mobileScheduledMenu')return {ok:true,generation:mobileClient.generation,value:{choice:JSON.stringify(['environment',fixture.id])}};return fixture.native.later(input);}};
  await mobileAutomationCommand('usage-environments','','','',now,chooser);fixture.calls.length=0;
  const gate=refreshBarrier();let held=false;
  fixture.intercept(async request=>{if(request.op==='environments'&&!held){held=true;await gate.wait();}});
  const load=mobileAutomationPrepare('settingsUsage','{}','','','',now,fixture.native);await gate.entered;
  try{
    const snapshot=mobileAutomationSnapshot();expect(snapshot.selection).toBe('[]');expect(snapshot.usage.tab).toBe('usage');
    expect(snapshot.usage.ready).toBe(false);expect(snapshot.usage.canRefresh).toBe(false);
    const count=fixture.calls.length;
    const pull=await mobileAutomationCommand('usage-refresh','','','',now+60_000,fixture.native);expect(pull.refresh).toBe(false);expect(pull.message).toBe('');
    const guarded=await mobileUsageCommand('refresh-usage','','',now+60_000,fixture.native,null,{selection:'[]',window:mobileUsageWindow(1,now),current:()=>true});
    expect(guarded.message).toContain('selection is still loading');expect(fixture.calls).toHaveLength(count);
  }finally{gate.release();}
  await load;expect(mobileAutomationSnapshot().usage.canRefresh).toBe(false);
  await mobileAutomationCommand('usage-local','tab','limits','',now,fixture.native);expect(mobileAutomationSnapshot().usage.canRefresh).toBe(true);
  fixture.calls.length=0;await mobileAutomationCommand('usage-refresh','','','',now+60_000,fixture.native);
  expect(fixture.calls.some(call=>call.method==='server.refreshProviders'||call.method==='server.refreshUsageRates'||call.method==='server.getUsageSummary')).toBe(false);
});
