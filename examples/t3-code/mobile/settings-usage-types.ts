// @ref llp/1109.003-pairing-and-transport.decision.md#mobile-adaptations
// Structural boundary for pinned providerUsageLimits schemas; not a second protocol client.
import { arr, obj, str, type Obj } from './shared/domain';
export type EnvironmentId = string;
export type ProviderConsumeResetCreditInput = { instanceId: string } | { sourceId: string; accountId: string; creditId: string };
export interface ServerProviderUsageWindow { id:string; kind:'session'|'weekly'|'monthly'|'other'; label:string; usedPercent:number; resetsAt?:string; windowDurationMins?:number }
export interface ServerProviderUsageLimits { checkedAt:string; windows:readonly ServerProviderUsageWindow[]; credentialFingerprint?:string;
  resetCredits?:{availableCount:number;nextExpiresAt?:string;nextCreditId?:string}; externalUsage?:{label:string;url:string}; unavailable?:{reason:'unsupported'|'probeFailed';message?:string} }
export interface ServerProvider { instanceId:string; driver:string; enabled:boolean; installed:boolean; availability?:unknown; displayName?:string; accentColor?:string;
  auth:{status:string;email?:string;label?:string;subscriptionSharing?:boolean}; usageLimits?:ServerProviderUsageLimits }
export type UsageLimitSourceSnapshots = readonly {id:string;label:string;error?:string;accounts:readonly {id:string;driver:string;email?:string;plan?:string;usageLimits:ServerProviderUsageLimits}[]}[];
export const isProviderAvailable = (provider:ServerProvider) => provider.availability !== 'unavailable';
const optional = (value:unknown) => typeof value==='string'&&value?value:undefined;
export function mobileUsageLimits(value:unknown):ServerProviderUsageLimits|undefined {
  const data=obj(value);if(!str(data.checkedAt)||!Array.isArray(data.windows))return undefined;
  const windows=arr(data.windows).flatMap(window=> {
    if(!str(window.id)||!str(window.label)||!['session','weekly','monthly','other'].includes(str(window.kind))||typeof window.usedPercent!=='number'||!Number.isFinite(window.usedPercent)||window.usedPercent<0||window.usedPercent>100)return [];
    return [{id:str(window.id),label:str(window.label),kind:window.kind as ServerProviderUsageWindow['kind'],usedPercent:window.usedPercent,resetsAt:optional(window.resetsAt),
      windowDurationMins:typeof window.windowDurationMins==='number'&&Number.isInteger(window.windowDurationMins)&&window.windowDurationMins>=0?window.windowDurationMins:undefined}];
  });
  const credit=obj(data.resetCredits),external=obj(data.externalUsage),unavailable=obj(data.unavailable);
  return {checkedAt:str(data.checkedAt),windows,credentialFingerprint:optional(data.credentialFingerprint),
    ...(typeof credit.availableCount==='number'&&Number.isInteger(credit.availableCount)&&credit.availableCount>=0?{resetCredits:{availableCount:credit.availableCount,nextExpiresAt:optional(credit.nextExpiresAt),nextCreditId:optional(credit.nextCreditId)}}:{}),
    ...(str(external.label)&&str(external.url)?{externalUsage:{label:str(external.label),url:str(external.url)}}:{}),
    ...(['unsupported','probeFailed'].includes(str(unavailable.reason))?{unavailable:{reason:unavailable.reason as 'unsupported'|'probeFailed',message:optional(unavailable.message)}}:{})};
}
export function mobileUsageConfig(config:Obj):{providers:ServerProvider[];usageLimitSources:UsageLimitSourceSnapshots} {
  return {providers:arr(config.providers).map(provider=>({instanceId:str(provider.instanceId),driver:str(provider.driver),enabled:provider.enabled===true,installed:provider.installed===true,availability:provider.availability,
    displayName:optional(provider.displayName),accentColor:optional(provider.accentColor),auth:{status:str(obj(provider.auth).status),email:optional(obj(provider.auth).email),label:optional(obj(provider.auth).label),subscriptionSharing:obj(provider.auth).subscriptionSharing===true},usageLimits:mobileUsageLimits(provider.usageLimits)})),
    usageLimitSources:arr(config.usageLimitSources).map(source=>({id:str(source.id),label:str(source.label),error:optional(source.error),accounts:arr(source.accounts).flatMap(account=>{
      const limits=mobileUsageLimits(account.usageLimits);return limits?[{id:str(account.id),driver:str(account.driver),email:optional(account.email),plan:optional(account.plan),usageLimits:limits}]:[];})}))};
}
