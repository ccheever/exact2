import { afterEach, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { mobileClient } from './client';
import { obj, type Obj } from './shared/domain';
import type { Native } from './shared/protocol';
import { collectLimitAccounts, collectLimitPools, displayLimitWindows, type LimitPresentations } from './settings-usage-limits';
import { mobileUsageConfig } from './settings-usage-types';
import { mobileUsage, mobileUsageCommand, mobileUsageSelection, mobileUsageSnapshot, mobileUsageWindow, usageGrant } from './settings-usage';
import { mobileUsagePresentation, mobileUsageAccount } from './settings-usage-view';
import { formatDateTimeShort, formatHourShort, formatUsd } from './shared/pages-usage';
import { mobileAutomationPrepare, mobileAutomationSnapshot, mobileAutomationCommand } from './settings-scheduled-flow';
import { ClientError } from './shared/protocol';
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
  const daily=mobileUsagePresentation({...input,window:{sinceDay:'2026-11-01',untilDay:'2026-11-01',timeZone:zone,resolution:'day'}},false);
  expect(daily.chart.map(period=>period.label)).toEqual(['Nov 1']);
});
test('mobile USD formatting keeps reference rounding, signed zero, grouping and nonfinite output',()=>{
  expect([0.825,1.275,-0.825,-0.004,-0,0,1_234_567.891,NaN,Infinity,-Infinity].map(formatUsd))
    .toEqual(['$0.83','$1.28','-$0.83','-$0.00','-$0.00','$0.00','$1,234,567.89','$NaN','$∞','-$∞']);
});
for(const [label,value,expected] of [
  ['half-cent',0.825,'$0.83'],['second half-cent',1.275,'$1.28'],['negative half-cent',-0.825,'-$0.83'],
  ['negative rounds to zero',-0.004,'-$0.00'],['zero',0,'$0.00'],['grouping',1_234_567.891,'$1,234,567.89'],
  ['NaN',NaN,'$0.00'],['positive infinity',Infinity,'$0.00'],['negative infinity',-Infinity,'$0.00'],
] as const)test(`Usage projection formats ${label} across cost fields and token details`,()=>{
  const summary:Obj={contractVersion:6,sources:[{status:'ok',fingerprint:{provider:'claude',hostId:'host',resolvedHomePath:'/home'},distinctSessions:1}],
    buckets:[{provider:'claude',model:'model',day:'2026-10-07',costUsd:value,cacheSavingsUsd:value,categoryCostUsd:{input:value},fastCostUsd:value,totals:{uncachedInputTokens:1000}}]};
  const input={tab:'usage',window:mobileUsageWindow(30,now),now,presentations:new Map(),external:false,
    sources:[{id:'a',label:'A',connected:true,summary,error:'',canRead:true,canWrite:false,canManage:false,config:{}}]};
  const cost=mobileUsagePresentation({...input,metric:'cost'},false),tokens=mobileUsagePresentation({...input,metric:'tokens'},false);
  expect([cost.total,cost.providers[0]?.value,cost.models[0]?.value,cost.chart.find(period=>period.id==='2026-10-07')?.bands.find(band=>band.id==='claude')?.value,
    ...['cost','savings','category-input','speed-fast'].map(id=>cost.costs.find(item=>item.id===id)?.value),tokens.providers[0]?.detail])
    .toEqual(Array(9).fill(expected));
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
