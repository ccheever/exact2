// @ref llp/1109.002-design-system-parity.spec.md#typography-and-font-assets
// Pinned UsageRouteScreen/UsageLimitsPooled projection. Clock supplied by route owner.
import { arr, obj, str, type Obj } from './shared/domain';
import { formatCount, formatTokens, formatUsd, formatPercent, formatDayShort, formatHourShort, mergeUsage, windowPeriods, isModelCostUnknown, type UsageWindow } from './shared/pages-usage';
import { collectLimitAccounts, collectLimitPools, collectLimitNotices, collectExternalUsageLinks, cursorUsageWindowDetails, displayLimitWindows, remainingPercent, formatDuration, type LimitPresentations } from './settings-usage-limits';
export interface UsageSourceView {id:string;label:string;connected:boolean;summary:Obj|null;error:string;canRead:boolean;canWrite:boolean;canManage:boolean;config:Obj}
interface Prepared {tab:string;window:UsageWindow;metric:string;now:number;sources:UsageSourceView[];presentations:LimitPresentations;external:boolean}
export const MOBILE_USAGE_PROVIDERS=[
  {id:'codex',driver:'codex',label:'Codex',light:'#3c3c43',dark:'#e6e6e6'}, {id:'claude',driver:'claudeAgent',label:'Claude Code',light:'#d97757',dark:'#d97757'},
  {id:'grok',driver:'grok',label:'Grok Build',light:'#52525b',dark:'#a1a1aa'}, {id:'cursor',driver:'cursor',label:'Cursor',light:'#8b8b8b',dark:'#8b8b8b'},
  {id:'opencode',driver:'opencode',label:'OpenCode',light:'#5b9bbd',dark:'#5b9bbd'}, {id:'antigravity',driver:'antigravity',label:'Antigravity',light:'#8c7bd1',dark:'#8c7bd1'}];
const USAGE_MIX_COLORS={input:{light:'#848484',dark:'#737373'},cacheRead:{light:'#c0c0c0',dark:'#282828'},cacheWrite:{light:'#6d6d6d',dark:'#949494'},
  output:{light:'#3c3c43',dark:'#e6e6e6'},other:{light:'#a3a3a3',dark:'#494949'},standard:{light:'#b8b8b8',dark:'#313131'},fast:{light:'#797979',dark:'#838383'},ultrafast:{light:'#3c3c43',dark:'#e6e6e6'}};
const costSegments=(items:{id:keyof typeof USAGE_MIX_COLORS;label:string;value:number}[])=>items.filter(item=>item.value>0)
  .map(item=>({id:item.id,label:item.label,weight:item.value,value:formatUsd(item.value),...USAGE_MIX_COLORS[item.id]}));
export function mobileUsageProviderRows(providers:Readonly<ReturnType<typeof mergeUsage>['providers']>,tokens:boolean){
  return [...providers].sort((a,b)=>tokens?b.totalTokens-a.totalTokens:b.costUsd-a.costUsd).flatMap(total=>{
    const item=MOBILE_USAGE_PROVIDERS.find(item=>item.id===total.provider),fraction=tokens?total.tokenShare:total.costShare;
    return item?[{id:item.id,label:item.label,light:item.light,dark:item.dark,value:tokens?formatTokens(total.totalTokens):formatUsd(total.costUsd),fraction,share:`${formatPercent(fraction)} of ${tokens?'tokens':'cost'}`,detail:tokens?formatUsd(total.costUsd):`${formatTokens(total.totalTokens)} tokens`}]:[];
  });
}
const provider=(id:string)=>MOBILE_USAGE_PROVIDERS.find(provider=>provider.id===id||provider.driver===id)??{id,driver:id,label:id,light:'#737373',dark:'#a3a3a3'};
const reveals=new Set<string>();let detailPrepared:Prepared|null=null;
export function mobileUsageReveal(key:string){if(reveals.has(key))reveals.delete(key);else reveals.add(key);}
const masked=(value:string|undefined,key:string)=>!value?'':reveals.has(key)?value:'••••••••';
const paceLabel=(pace:string|null)=>pace==='ahead'?'Ahead of pace':pace==='under'?'Under pace':pace==='on'?'On pace':'';
const LIMIT_DRIVER_LABEL:Readonly<Record<string,string>>={codex:'Codex',claudeAgent:'Claude'};
export function mobileUsagePresentation(input:Prepared|null,publish=true,pendingCursor:(id:string)=>boolean=()=>false){
  if(publish){detailPrepared=input;if(!input)reveals.clear();}
  const sources=input?.sources??[],summaries=sources.flatMap(source=>source.summary?[{id:source.id,label:source.label,summary:source.summary}]:[]),merged=mergeUsage(summaries),tokens=input?.metric==='tokens',compatible=summaries.filter(item=>Number(item.summary.contractVersion)>=4&&Number(item.summary.contractVersion)<=6),actual=compatible.length>0;
  const accounts=collectLimitAccounts(input?.presentations??new Map()),pools=collectLimitPools(accounts,input?.now??0),periods=input?windowPeriods(input.window):[],byPeriod=new Map((input?.window.resolution==='hour'?merged.hourly:merged.daily).map(period=>[period.key,period]));
  const activePeriods=(input?.window.resolution==='hour'?merged.hourly:merged.daily).filter(period=>period.totalTokens>0).length;
  const periodAverage=activePeriods===0?0:merged.totalTokens/activePeriods,observedInput=merged.uncachedInputTokens+merged.cachedInputTokens;
  const peak=Math.max(0,...periods.map(key=>{const period=byPeriod.get(key);return period?(tokens?period.totalTokens:period.costUsd):0;}));
  const hasCursorAccount=sources.some(source=>arr(source.summary?.sources).some(item=>obj(item.fingerprint).provider==='cursor'&&obj(item.fingerprint).hostId==='cursor.com'));
  const cursorSources=hasCursorAccount?[]:sources.filter(source=>arr(source.summary?.sources).some(item=>item.action==='enableCursorKeychain')&&arr(source.config.providers).some(provider=>provider.driver==='cursor'&&provider.status==='ready'));
  const cursor=cursorSources.map(source=>({id:source.id,label:source.label,usageLabel:sources.length>1?`Cursor · ${source.label}`:'Cursor',
    buttonLabel:cursorSources.length>1?`Enable on ${source.label}`:'Enable',disabled:!source.connected||!source.canWrite||pendingCursor(source.id)}));
  const providers=mobileUsageProviderRows(merged.providers,tokens),afterCore=(ids:string[],claude:string)=>Math.max(ids.indexOf('codex'),ids.indexOf(claude))+1;
  return {ready:!!input,error:'',tab:input?.tab??'limits',metric:tokens?'tokens':'cost',hasUsage:actual,canRefresh:(input?.tab??'limits')==='limits'||sources.some(source=>source.canRead),refreshing:false,
    emptyMessage:!input?'':!sources.length?'Select an environment to see usage.':!actual&&input.tab==='usage'?'No usage summary is available for these environments.':'',
    environments:sources.map(source=>({id:source.id,label:source.label,status:source.error||(!source.connected?'Waiting for connection…':''),connected:source.connected})),
    notices:[...collectLimitNotices(input?.presentations??new Map()),...merged.mismatches.map(item=>`${item.environment}: ${item.direction==='serverBehind'?'Update the server to read usage.':'Update this app to read usage.'}`),...sources.flatMap(source=>arr(source.summary?.sources).flatMap(item=>str(item.message)?[`${source.label}: ${str(item.message)}`]:[]))].map((text,index)=>({id:String(index),text})),
    duplicateMessage:merged.duplicates.length?`Counted once across environments sharing a transcript directory: ${merged.duplicates.join(', ')}`:'',
    links:collectExternalUsageLinks(input?.presentations??new Map()).map(link=>({...link,accounts:link.accounts.join(', '),message:link.message??'',disabled:!input?.external})),
    cursor,cursorInsertAt:afterCore(providers.map(item=>item.id),'claude'),cursorPoolInsertAt:afterCore(pools.map(pool=>pool.driver),'claudeAgent'),
    pools:pools.map(pool=>({id:pool.driver,label:LIMIT_DRIVER_LABEL[pool.driver]??pool.driver,light:provider(pool.driver).light,dark:provider(pool.driver).dark,
      windows:displayLimitWindows(pool).map(window=>{
        const details=pool.driver==='cursor'?cursorUsageWindowDetails(window.id):undefined;
        const refill=window.resets.find(reset=>reset.restoresPercent>0),now=input?.now??0;
        return {id:`${window.kind}:${window.id}`,windowId:window.id,kind:window.kind,label:details?.label??window.label,description:details?.description??'',
          remaining:window.remainingPercent,pace:paceLabel(window.pace),nextRefill:refill?`↻ +${refill.restoresPercent}% ${refill.at<=now?'now':`in ${formatDuration(refill.at-now)}`}`:'',
          columns:window.columns.map(({account,window:own})=>({key:account.key,label:account.displayName||provider(account.driver).label,present:!!own,remaining:own?remainingPercent(own):0,email:masked(account.email,account.key)}))};
      })})),
    limitsEmpty:input?.tab==='limits'&&pools.length===0&&cursor.length===0?'No subscription limits reported.':'',
    total:actual?(tokens?formatTokens(merged.totalTokens):formatUsd(merged.costUsd)):'',chartTitle:`${input?.window.resolution==='hour'?'Hourly':'Daily'} ${tokens?'processed tokens':'cost'}`,
    chartHasActivity:(input?.window.resolution==='hour'?merged.hourly:merged.daily).some(period=>period.totalTokens>0),
    headlineLabel:actual?(tokens?'Processed tokens':'Raw token cost'):'',
    headlineValue:actual?(tokens?formatTokens(merged.totalTokens):`${formatUsd(merged.costUsd)}*`):'',
    headlineDetail:actual?(tokens?`Across ${formatCount(merged.sessions)} sessions`:'* if billed at full API rate'):'',
    chartStart:input?(input.window.resolution==='hour'?formatHourShort(periods[0]??'',input.window.timeZone):formatDayShort(input.window.sinceDay)):'',
    chartEnd:input?(input.window.resolution==='hour'?formatHourShort(periods[periods.length-1]??'',input.window.timeZone):formatDayShort(input.window.untilDay)):'',
    chartLegend:merged.providers.flatMap(total=>{const item=MOBILE_USAGE_PROVIDERS.find(item=>item.id===total.provider);return item?[{id:item.id,label:item.label,light:item.light,dark:item.dark}]:[];}),
    chart:periods.map(key=>{const period=byPeriod.get(key);return {id:key,label:input?.window.resolution==='hour'?formatHourShort(key,input.window.timeZone):formatDayShort(key),
      bands:[...MOBILE_USAGE_PROVIDERS].reverse().map(provider=>{const band=period?.byProvider.get(provider.id),value=band?(tokens?band.totalTokens:band.costUsd):0;return {id:provider.id,light:provider.light,dark:provider.dark,height:peak?value/peak*180:0,value:tokens?formatTokens(value):formatUsd(value)};})};}),
    providers,hasProviders:providers.length>0||cursor.length>0,
    totals:[{id:'tokens',label:'Processed tokens',value:formatTokens(merged.totalTokens),detail:`${formatTokens(periodAverage)} per active ${input?.window.resolution==='hour'?'hour':'day'}`},
      {id:'cache-savings',label:'Cache savings',value:formatUsd(merged.cacheSavingsUsd),detail:merged.costUsd>0?`${(merged.cacheSavingsUsd/merged.costUsd).toFixed(1)}x the raw cost`:'vs full input rates'},
      {id:'cache-read',label:'Cached input',value:formatTokens(merged.cachedInputTokens),detail:`${formatPercent(observedInput===0?0:merged.cachedInputTokens/observedInput)} of observed input`},
      {id:'input',label:'Uncached input',value:formatTokens(merged.uncachedInputTokens),detail:`${formatTokens(merged.cacheCreationTokens)} cache writes`},
      {id:'output',label:'Output',value:formatTokens(merged.outputTokens),detail:`incl. ${formatTokens(merged.reasoningTokens)} reasoning`},
      {id:'unpriced',label:'Unpriced',value:formatPercent(merged.unpricedShare),detail:'of records, excluded from cost'}],
    hasCost:merged.costUsd>0,
    costByType:costSegments([{id:'input',label:'Input',value:merged.categoryCost.input},{id:'cacheRead',label:'Cache read',value:merged.categoryCost.cacheRead},
      {id:'cacheWrite',label:'Cache write',value:merged.categoryCost.cacheWrite},{id:'output',label:'Output',value:merged.categoryCost.output},
      {id:'other',label:'Other',value:merged.categoryCost.unsplit>=0.005?merged.categoryCost.unsplit:0}]),
    costBySpeed:merged.speedCost.fast+merged.speedCost.ultrafast>0?costSegments([{id:'standard',label:'Standard',value:merged.speedCost.standard},
      {id:'fast',label:'Fast',value:merged.speedCost.fast},{id:'ultrafast',label:'Ultrafast',value:merged.speedCost.ultrafast}]):[],
    costPremium:`${formatUsd(merged.speedCost.premium)} premium`,
    models:[...merged.models].sort((a,b)=>tokens?b.totalTokens-a.totalTokens:b.costUsd-a.costUsd).map(model=>{
      const palette=provider(model.provider),unknown=isModelCostUnknown(model);
      return {id:JSON.stringify([model.provider,model.model]),label:model.model,light:palette.light,dark:palette.dark,
        value:tokens?formatTokens(model.totalTokens):unknown?'Unpriced':formatUsd(model.costUsd),
        detail:tokens?`${formatPercent(model.tokenShare)} of tokens · ${unknown?'no known rates':formatUsd(model.costUsd)}`:unknown?`no known rates · ${formatTokens(model.totalTokens)} tokens`:`${formatPercent(model.costShare)} of cost · ${formatTokens(model.totalTokens)} tokens`};
    })};
}
export function mobileUsageAccount(key:string,windowId:string,kind:string,now:number){
  const input=detailPrepared,account=collectLimitAccounts(input?.presentations??new Map()).find(account=>account.key===key),pool=collectLimitPools(account?[account]:[],now)[0],window=account?.limits.windows.find(window=>window.id===windowId&&window.kind===kind),credits=account?.limits.resetCredits;
  const pooled=collectLimitPools(collectLimitAccounts(input?.presentations??new Map()),now).find(pool=>pool.driver===account?.driver)?.windows.find(window=>window.id===windowId&&window.kind===kind);
  return {present:!!account,error:account?'':'This account is no longer reporting limits on the selected environments.',key,label:account?.displayName||provider(account?.driver??'').label,email:masked(account?.email,key),hasEmail:!!account?.email,revealed:reveals.has(key),plan:account?.plan??'',
    windowLabel:window?.label??'',remaining:window?`${remainingPercent(window)}% remaining`:'',resets:window?.resetsAt?new Date(window.resetsAt).toLocaleString():'',restores:window&&pooled?`Restores ${Math.round(window.usedPercent/pooled.members.length)}% of the pool`:'',
    source:account?.environments.length?`Signed in on ${account.environments.map(env=>env.label).join(', ')}`:account?.sourceLabel?`Source: ${account.sourceLabel}`:'',
    credits:credits?credits.availableCount===0?'No reset credits banked':`${credits.availableCount} ${credits.availableCount===1?'reset credit':'reset credits'} banked${credits.nextExpiresAt?` · next expires in ${formatDuration(Date.parse(credits.nextExpiresAt)-now)}`:''}`:'',
    canRedeem:!!account?.redeem&&!!credits?.availableCount&&!!input?.sources.find(source=>source.id===account.redeem?.environmentId&&source.connected&&source.canManage),environmentId:account?.redeem?.environmentId??'',
    light:provider(pool?.driver??'').light,dark:provider(pool?.driver??'').dark};
}
