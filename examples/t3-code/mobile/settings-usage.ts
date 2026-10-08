// @ref llp/1109.009-mobile-settings.decision.md#root-and-native-lifetime
// @ref llp/1109.003-pairing-and-transport.decision.md#mobile-adaptations
// Mobile Usage route ownership over the shared transport, config and usage merge.
import { mobileClient } from './client';
import { arr, obj, str, type Obj } from './shared/domain';
import { letGo } from './shared/let-go';
import { bridgeReply, ClientError, type Native } from './shared/protocol';
import { settingsSources, settingsEndpoint, settingsCall, settingsNative, settingsGrants, settingsAdoptConfig, type MobileSettingsEndpoint } from './settings-server-source';
import { makeWindow, type UsageWindow } from './shared/pages-usage';
import { collectLimitAccounts, type LimitPresentations } from './settings-usage-limits';
import { mobileUsageConfig } from './settings-usage-types';
import { mobileUsagePresentation, type UsageSourceView } from './settings-usage-view';
export { mobileUsageAccount, mobileUsageReveal } from './settings-usage-view';
export const USAGE_ROUTES=['SettingsUsage','SettingsUsageAccount'];
export function mobileUsageSelection(json:string):string[]|null {
  let value:unknown;try{value=JSON.parse(json);}catch{throw new ClientError('The usage environment selection is invalid.');}
  if(value===null)return null;if(!Array.isArray(value)||!value.every(item=>typeof item==='string'))throw new ClientError('The usage environment selection is invalid.');return [...new Set(value)];
}
export function usageGrant(session:Obj):boolean {
  if(session.authenticated!==true)return false;
  if(session.permissions!==undefined)return Array.isArray(session.permissions)&&session.permissions.includes('diagnostics:read');
  const scopes=Array.isArray(session.scopes)?session.scopes:[];
  return scopes.includes('diagnostics:read')||obj(session.auth).serverUpdateScope===undefined&&scopes.includes('orchestration:read');
}
export const mobileUsageWindow=(days:number,now:number)=>makeWindow([1,7,30,90].includes(days)?days:30,now);
interface CachedSummary { generation:number; origin:string|null; windowKey:string; summary:Obj|null; error:string; pending:Promise<void>|null }
const summaries=new Map<string,CachedSummary>(),refreshAfter=new Map<string,number>(),refreshes=new Map<string,Promise<void>>(),commands=new Set<string>();
let prepared:{selection:string;tab:string;window:UsageWindow;metric:string;now:number;sources:UsageSourceView[];presentations:LimitPresentations;external:boolean;choices:{id:string;label:string;selected:boolean}[]}|null=null;
let epoch=0;
const failure=(error:unknown,fallback:string)=>error instanceof Error?error.message:fallback;
const result=(message='')=>({revision:++mobileClient.revision,message});
function parseWindow(json:string):UsageWindow {
  const value=obj(JSON.parse(json));
  if(!/^\d{4}-\d{2}-\d{2}$/.test(str(value.sinceDay))||!/^\d{4}-\d{2}-\d{2}$/.test(str(value.untilDay))||!str(value.timeZone)||!['day','hour'].includes(str(value.resolution)))throw new ClientError('Choose a valid usage window.');
  if(value.resolution==='hour'&&(!Number.isFinite(Date.parse(str(value.sinceTime)))||!Number.isFinite(Date.parse(str(value.untilTime)))))throw new ClientError('Choose a valid hourly window.');
  return {sinceDay:str(value.sinceDay),untilDay:str(value.untilDay),timeZone:str(value.timeZone),resolution:value.resolution as 'day'|'hour',...(value.resolution==='hour'?{sinceTime:str(value.sinceTime),untilTime:str(value.untilTime)}:{})};
}
async function readSummary(endpoint:MobileSettingsEndpoint,window:UsageWindow,force=false) {
  const id=endpoint.source.environmentId,key=JSON.stringify(window);let cached=summaries.get(id);
  if(!cached||cached.generation!==endpoint.generation||cached.origin!==endpoint.source.origin||cached.windowKey!==key){cached={generation:endpoint.generation,origin:endpoint.source.origin,windowKey:key,summary:null,error:'',pending:null};summaries.set(id,cached);}
  if(cached.pending){await cached.pending;return cached;}
  if(!force&&cached.summary)return cached;
  const entry=cached;
  entry.pending=(async()=>{try{entry.summary=await settingsCall(endpoint,{op:'request',method:'server.getUsageSummary',payload:window});entry.error='';}catch(error){if(letGo(error))throw error;entry.error=failure(error,'Could not read usage.');}finally{entry.pending=null;}})();
  await entry.pending;return entry;
}
async function probe(endpoint:MobileSettingsEndpoint,now:number,automatic:boolean,afterPending=false):Promise<void> {
  const key=JSON.stringify([endpoint.source.key,endpoint.generation]),pending=refreshes.get(key);
  if(pending){if(!afterPending)return automatic?undefined:pending;try{await pending;}catch{}return probe(endpoint,now,false,true);}
  if(automatic&&now<(refreshAfter.get(key)??0))return;
  const promise=(async()=>{await settingsCall(endpoint,{op:'request',method:'server.refreshProviders',payload:{}});
    const config=await settingsCall(endpoint,{op:'request',method:'server.getConfig',payload:{}});settingsAdoptConfig(endpoint,config);
  })().finally(()=>{refreshes.delete(key);refreshAfter.set(key,now+300000);});refreshes.set(key,promise);return promise;
}
/** Async only at route activation, source/revision/window changes, or refresh; local metric/reveal uses snapshot. */
export async function mobileUsage(selectionJSON:string,tab:string,windowJSON:string,metric:string,now:number,nativeInput:Native|null|undefined,externalLinksAvailable=false,active=true) {
  const run=++epoch;
  if(!active){prepared=null;return mobileUsagePresentation(null);}
  if(!nativeInput?.available)return mobileUsagePresentation(null);
  try{
    const selection=mobileUsageSelection(selectionJSON),window=parseWindow(windowJSON),native=settingsNative(nativeInput),all=await settingsSources(native);
    const selected=all.filter(source=>source.enabled&&(selection===null||selection.includes(source.environmentId))),presentations=new Map() as Map<string,LimitPresentations extends ReadonlyMap<string,infer V>?V:never>;
    const sources=await Promise.all(selected.map(async source=>{
      const endpoint=settingsEndpoint(source,native);let error='',summary:Obj|null=null,canRead=false,canWrite=false,canManage=false;
      if(source.phase==='connected'){
        try{const session=await settingsCall(endpoint,{op:'http',path:'/api/auth/session'});canRead=usageGrant(session);canWrite=settingsGrants(session,'settings:write');canManage=settingsGrants(session,'providers:manage');
          if(tab==='limits')try{await probe(endpoint,now,true);}catch(cause){if(letGo(cause))throw cause;error=failure(cause,'Could not refresh limits.');}
          if(canRead){const cached=await readSummary(endpoint,window);summary=cached.summary;error=error||cached.error;}else if(tab!=='limits')error='This connection cannot read usage diagnostics.';
        }catch(cause){if(letGo(cause))throw cause;error=failure(cause,'Could not verify access.');}
        presentations.set(source.environmentId,{entry:{target:{label:source.label}},serverConfig:mobileUsageConfig(source.config)});
      }else{const cached=summaries.get(source.environmentId);summary=cached?.windowKey===JSON.stringify(window)&&cached.origin===source.origin?cached.summary:null;error=summary?'Disconnected · showing saved usage':'Waiting for connection…';}
      return {id:source.environmentId,label:source.label,connected:source.phase==='connected',summary,error,canRead,canWrite,canManage,config:source.config};
    }));
    if(run===epoch)prepared={selection:selectionJSON,tab:tab==='usage'?'usage':'limits',window,metric:metric==='tokens'?'tokens':'cost',now,sources,presentations,external:externalLinksAvailable,choices:all.filter(source=>source.enabled).map(source=>({id:source.environmentId,label:source.label,selected:selection===null||selection.includes(source.environmentId)}))};
    return mobileUsageSnapshot(selectionJSON,tab,metric);
  }catch(error){if(letGo(error))throw error;return {...mobileUsagePresentation(null),error:failure(error,'Could not load usage.')};}
}
/** No network/watch activity for a metric or email-reveal change. Scope mismatch never shows another selection. */
export function mobileUsageSnapshot(selectionJSON:string,tab:string,metric:string){return mobileUsagePresentation(prepared?.selection===selectionJSON?{...prepared,tab:tab==='usage'?'usage':'limits',metric:metric==='tokens'?'tokens':'cost'}:null);}
async function current(environmentId:string,native:Native,permission:'diagnostics:read'|'settings:write'|'providers:manage'|null) {
  if(!prepared?.sources.some(source=>source.id===environmentId))throw new ClientError('Choose an environment in the current usage selection.');
  const source=(await settingsSources(native)).find(source=>source.environmentId===environmentId&&source.enabled&&source.phase==='connected');if(!source)throw new ClientError('This environment is disconnected.');
  const endpoint=settingsEndpoint(source,native),session=await settingsCall(endpoint,{op:'http',path:'/api/auth/session'});
  if(permission&&!(permission==='diagnostics:read'?usageGrant(session):settingsGrants(session,permission)))throw new ClientError('This connection does not have permission for that action.');
  return endpoint;
}
/** Root confirms redeem before sending redeem-confirmed; accountKey binds displayed credit identity. */
export async function mobileUsageCommand(op:string,environmentId:string,accountKey:string,now:number,nativeInput:Native|null|undefined,confirmedRedemption:string|null=null){
  const key=JSON.stringify([op,environmentId,accountKey]);if(commands.has(key))return result('This usage action is already running.');commands.add(key);
  try{
    if(!prepared||!nativeInput?.available)throw new ClientError('Open Usage in the native app first.');const native=settingsNative(nativeInput);
    if(op==='open-link'){
      const links=mobileUsagePresentation(prepared).links;if(!prepared.external||!links.some(link=>link.url===accountKey))throw new ClientError('This usage link is unavailable.');
      const url=new URL(accountKey);if(!['http:','https:'].includes(url.protocol))throw new ClientError('This usage URL cannot be opened.');
      const reply=await bridgeReply(nativeInput,{op:'mobileOpenURL',url:url.href});if(!reply.ok||obj(reply.value).opened!==true)throw new ClientError('The usage URL was not opened.');return result();
    }
    if(op==='refresh-usage'||op==='refresh-limits'){
      const failures=await Promise.all(prepared.sources.filter(source=>source.connected).map(async source=>{try{const endpoint=await current(source.id,native,op==='refresh-usage'?'diagnostics:read':null);
        if(op==='refresh-limits')await probe(endpoint,now,false);else{let ratesError='';try{await settingsCall(endpoint,{op:'request',method:'server.refreshUsageRates',payload:{}});}catch(error){if(letGo(error))throw error;ratesError=failure(error,'Could not refresh rates.');}
          const read=await readSummary(endpoint,prepared!.window,true);if(ratesError||read.error)throw new ClientError([ratesError,read.error].filter(Boolean).join(' '));}return '';
      }catch(error){if(letGo(error))throw error;return `${source.label}: ${failure(error,'Could not refresh usage.')}`;}}));return result(failures.filter(Boolean).join('\n'));
    }
    if(op==='enable-cursor'){
      const source=prepared.sources.find(source=>source.id===environmentId),hasAccount=prepared.sources.some(source=>arr(source.summary?.sources).some(item=>obj(item.fingerprint).provider==='cursor'&&obj(item.fingerprint).hostId==='cursor.com'));
      if(hasAccount||!source||!arr(source.summary?.sources).some(item=>item.action==='enableCursorKeychain')||!arr(source.config.providers).some(provider=>provider.driver==='cursor'&&provider.status==='ready'))throw new ClientError('Cursor access is not requested by this environment.');
      const endpoint=await current(environmentId,native,'settings:write'),settings=await settingsCall(endpoint,{op:'request',method:'server.updateSettings',payload:{patch:{cursorKeychainUsageEnabled:true}}});settingsAdoptConfig(endpoint,{...endpoint.source.config,settings});
      await Promise.all([probe(endpoint,now,false,true),readSummary(endpoint,prepared.window,true)]);return result();
    }
    if(op==='redeem-confirmed'){
      const shown=collectLimitAccounts(prepared.presentations).find(account=>account.key===accountKey);if(!shown?.redeem||!shown.limits.resetCredits?.availableCount)throw new ClientError('No reset credit is available for this account.');
      if(confirmedRedemption!==null&&JSON.stringify(shown.redeem)!==confirmedRedemption)throw new ClientError('This reset credit changed. Reopen the account before using it.');
      if(environmentId!==shown.redeem.environmentId)throw new ClientError('The reset-credit environment changed.');const endpoint=await current(environmentId,native,'providers:manage');
      const latest=new Map(prepared.presentations);latest.set(environmentId,{entry:{target:{label:endpoint.source.label}},serverConfig:mobileUsageConfig(endpoint.source.config)});
      const account=collectLimitAccounts(latest).find(account=>account.key===accountKey);
      if(!account?.redeem||!account.limits.resetCredits?.availableCount||JSON.stringify(account.redeem)!==JSON.stringify(shown.redeem))throw new ClientError('This reset credit changed. Reopen the account before using it.');
      const reply=await settingsCall(endpoint,{op:'request',method:'provider.consumeResetCredit',payload:account.redeem.input});
      const outcomes:Record<string,string>={reset:'Reset applied. Your windows have cleared.',nothingToReset:'Nothing to reset right now.',noCredit:'No reset credit left.',alreadyRedeemed:'That credit was already redeemed.'};
      return result(str(reply.warning)||outcomes[str(reply.outcome)]||'The server returned an unknown reset-credit outcome.');
    }
    throw new ClientError('That usage action is unavailable or requires confirmation.');
  }catch(error){if(letGo(error))throw error;return result(failure(error,'Could not update usage.'));}finally{commands.delete(key);}
}

/** The independent Usage selection has no project scope. Null tracks newly connected environments. */
export function mobileUsageEnvironmentChoices(selectionJSON:string){return prepared?.selection===selectionJSON?prepared.choices:[];}
export function mobileUsageToggleEnvironment(selectionJSON:string,environmentId:string):string {
  const choices=mobileUsageEnvironmentChoices(selectionJSON);if(!choices.some(choice=>choice.id===environmentId))throw new ClientError('This environment is unavailable.');
  const current=mobileUsageSelection(selectionJSON),ids=new Set(current??choices.map(choice=>choice.id));if(ids.has(environmentId))ids.delete(environmentId);else ids.add(environmentId);return JSON.stringify([...ids]);
}

/** Immutable identity of the credit displayed before a native confirmation opens. */
export function mobileUsageRedemption(accountKey:string):string|null {const account=collectLimitAccounts(prepared?.presentations??new Map()).find(account=>account.key===accountKey);return account?.redeem&&account.limits.resetCredits?.availableCount?JSON.stringify(account.redeem):null;}
