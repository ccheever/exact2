// @ref llp/1109.005-composer-and-transcript.decision.md#ordinary-send-handoff-and-pending-feed
import {expect,test} from 'bun:test';
import {mobileThreadLocalDock as project,mobileThreadLocalResetKey as resetKey,type ThreadLocalDockInput} from './thread-local-dock';
import {collectProviderUsageLimits,type UsageLimitsReport,type UsageLimitsAccount} from './thread-local-usage-model';
import type {ThreadLocalFeedbackNotice} from './thread-send-local-commands';
const now=Date.parse('2026-10-09T12:00:00Z'),iso=(ms:number)=>new Date(ms).toISOString();
const input:ThreadLocalDockInput={usageKey:'home:env:thread:provider:run',viewportHeight:844,dark:false,canManageProviders:true};
const account=(patch:Partial<UsageLimitsAccount>={}):UsageLimitsAccount=>({id:'a',driver:'codex',label:'Account',instanceId:'codex',
  limits:{windows:[{id:'session',label:'Session',usedPercent:25,resetsAt:iso(now+3600000),windowDurationMins:120}]},...patch});
const report=(accounts:UsageLimitsAccount[],notices:string[]=[]):UsageLimitsReport=>({createdAt:iso(now),accounts,notices});
const view=(usage:UsageLimitsReport|null,options:Partial<ThreadLocalDockInput>={})=>project({usage,feedback:[]},{...input,...options});
test('report projection anchors pace and countdown to captured report time, with source 40% height',()=>{
  const data=view(report([account()]));expect(data.maxHeight).toBe(338);
  expect(data.accounts[0]!.windows[0]).toMatchObject({remaining:75,timeLeft:50,hasTime:true,pace:'under pace',resets:'resets in 1h 0m',color:'#3c3c43'});
  expect(view(report([account()]),{viewportHeight:1024,dark:true}).accounts[0]!.windows[0]!.color).toBe('#e6e6e6');
  expect(view(report([account()]),{viewportHeight:1024}).maxHeight).toBe(410);
});
test('source threshold colors include exact boundaries and unknown driver uses foreground',()=>{
  const windows=[90,70,69].map((usedPercent,i)=>({id:String(i),label:'Quota',usedPercent}));
  const rows=view(report([account({driver:'other',limits:{windows}})])).accounts[0]!.windows;
  expect(rows.map(row=>row.color)).toEqual(['oklch(63.7% 0.237 25.331)','oklch(76.9% 0.188 70.08)','']);
  expect(rows.every(row=>!row.hasTime&&row.pace===''&&row.resets==='')).toBe(true);
  expect(view(report([account({driver:'claudeAgent'})])).accounts[0]!.windows[0]!.color).toBe('#d97757');
});
test('individual account identity follows source label precedence and reveal state key changes with label',()=>{
  const first=view(report([account({displayName:'  work@example.com  ',plan:'Pro'}),account({id:'b',driver:'claudeAgent',instanceId:'custom',displayName:' '})]));
  expect(first.accounts.map(row=>[row.label,row.instanceLabel,row.first])).toEqual([['Codex','work@example.com',true],['Claude','custom',false]]);
  expect(first.accounts[0]!.detail).toBe('Pro');
  const next=view(report([account({displayName:'other@example.com'})]));expect(next.accounts[0]!.key).not.toBe(first.accounts[0]!.key);
  expect(view(report([account({instanceId:undefined,label:'Hub / pooled'})])).accounts[0]!.instanceLabel).toBe('Hub / pooled');
  expect(view(report([account()])).accounts[0]!.instanceLabel).toBe('');
});
test('limits notices suppress bars, external links survive and report notices retain source border policy',()=>{
  const data=view(report([account({limits:{windows:[{id:'a',label:'a',usedPercent:0}],unavailable:{reason:'probeFailed',message:'Offline'},externalUsage:{url:'https://usage.example'}}})],['Hub unavailable']));
  expect(data.accounts[0]).toMatchObject({notice:'Offline',windows:[],externalURL:'https://usage.example'});
  expect(data.notices[0]).toMatchObject({text:'Hub unavailable',border:true});
  const empty=view(report([],['One','Two']));expect(empty.hasUsage).toBe(true);expect(empty.accounts).toEqual([]);expect(empty.notices.map(row=>row.border)).toEqual([false,false]);
  expect(view(null)).toMatchObject({hasUsage:false,accounts:[],notices:[]});
});
test('dense reset footer hides zero credits until status and binds exact pooled input',()=>{
  const pooled={sourceId:'hub',accountId:'account',creditId:'credit'},a=account({resetCreditInput:pooled,limits:{windows:[],resetCredits:{availableCount:2,nextExpiresAt:iso(now+3600000)}}});
  const key=resetKey(input.usageKey,a.id,pooled);
  const row=view(report([a]),{canManageProviders:false,resetStates:[{key,busy:true,status:'Retained warning'}]}).accounts[0]!;
  expect(row).toMatchObject({creditsVisible:true,creditsSummary:'2 reset credits banked · next expires in 1h 0m',resetKey:key,
    resetInput:JSON.stringify(pooled),resetVisible:true,resetDisabled:true,resetLabel:'Using…',resetStatus:'Retained warning',permissionMessage:'This connection cannot manage provider accounts.'});
  a.limits.resetCredits={availableCount:0};expect(view(report([a])).accounts[0]!.creditsVisible).toBe(false);
  expect(view(report([a]),{resetStates:[{key,busy:false,status:'Nothing to reset right now.'}]}).accounts[0]).toMatchObject({creditsVisible:true,resetVisible:false,creditsSummary:'No reset credits banked'});
  expect(view(report([a]),{usageKey:'other',resetStates:[{key,busy:true,status:'old'}]}).accounts[0]).toMatchObject({creditsVisible:false,resetStatus:''});
});
test('native reset fallback uses instance ID and missing reset authority cannot render action',()=>{
  const a=account({limits:{windows:[],resetCredits:{availableCount:1}}});
  expect(view(report([a])).accounts[0]).toMatchObject({resetInput:'{"instanceId":"codex"}',creditsSummary:'1 reset credit banked',resetDisabled:false});
  delete a.instanceId;expect(view(report([a])).accounts[0]).toMatchObject({creditsVisible:false,resetVisible:false,resetInput:''});
});
test('actual local usage report keeps deduped hub reset identity through visual projection',()=>{
  const limits={checkedAt:iso(now),windows:[{id:'weekly',label:'Weekly',kind:'weekly',usedPercent:10}]};
  const result=collectProviderUsageLimits('codex',[{instanceId:'codex',driver:'codex',enabled:true,installed:true,auth:{email:'same@example.com'},usageLimits:limits}],
    [{id:'hub',label:'Hub',accounts:[{id:'acct',driver:'codex',email:' SAME@EXAMPLE.COM ',usageLimits:{...limits,resetCredits:{availableCount:1,nextCreditId:'next'}}}]}],now)!;
  const data=view(result);expect(data.accounts).toHaveLength(1);expect(JSON.parse(data.accounts[0]!.resetInput)).toEqual({sourceId:'hub',accountId:'acct',creditId:'next'});
  expect(data.accounts[0]!.windows[0]!.remaining).toBe(90);
});
test('feedback status and Copy ID payload remain exact detached notices',()=>{
  const feedback:ThreadLocalFeedbackNotice[]=[{id:'one',status:'uploading',title:'Sending feedback to OpenAI...',description:'',feedbackId:'',dismissible:false,createdAt:iso(now)},
    {id:'two',status:'sent',title:'Feedback sent to OpenAI',description:'Thread ID: exact',feedbackId:'exact',dismissible:true,createdAt:iso(now)}];
  const data=project({usage:null,feedback},input);expect(data.feedback).toEqual(feedback);
  data.feedback[1]!.feedbackId='changed';expect(feedback[1]!.feedbackId).toBe('exact');
});
