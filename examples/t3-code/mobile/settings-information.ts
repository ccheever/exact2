// @ref llp/1107.009-mobile-settings.decision.md#information-sources
// Mobile365aa87982 About, Legal, licenses, Diagnostics and Client Storage routes.
import { obj, str, type Obj } from './shared/domain';
import { decodeNotices } from './shared/settings-data';
import { bridgeReply, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
export const MOBILE_INFORMATION_ROUTES=['settingsAbout','settingsClientStorage','settingsDiagnostics','settingsOpenSourceLicenses','settingsOpenSourceLicense','settingsLegal'];
export const MOBILE_LEGAL_URL='https://t3.codes/legal';
export const MOBILE_LEGAL_DOCUMENTS=['legal','privacy-policy','terms-of-service','security-policy'].map(path=>`https://t3.codes/${path}`);
export function mobileLegalDocument(value:string){try{const url=new URL(value);return ['http:','https:'].includes(url.protocol)&&MOBILE_LEGAL_DOCUMENTS.includes(`${url.origin}${url.pathname.replace(/\/+$/,'')||'/'}`);}catch{return false;}}
const bundleNames:Record<string,string>={mobile:'Mobile',ios:'iOS',assets:'Assets',web:'Web',server:'Server',desktop:'Desktop','device-tools':'Device tools'};
let version='',build='',identityError='',noticeError='',coverage='',entries:Obj[]|null=null,revision=0,epoch=0;
/** Reads local bundle identity/notices only. No connected server or account scope applies. */
export async function mobileInformationPrepare(route:string,native:Native|null|undefined){
  const token=++epoch;if(!MOBILE_INFORMATION_ROUTES.includes(route))return {revision};
  try{
    if(!native?.available)throw new Error('Open in the native app to read this build’s information.');
    const reply=await bridgeReply(native,{op:'mobileInformation'});if(!reply.ok)throw new Error(str(obj(reply.error).message,'Build information is unavailable.'));
    if(token!==epoch)return {revision};const data=obj(reply.value);version=str(data.version);build=str(data.build);identityError='';coverage=str(data.noticeCoverage);noticeError='';
    try{entries=decodeNotices(obj(data.notices));}catch{entries=null;noticeError='License notices are unavailable in this build.';}
  }catch(error){if(letGo(error))throw error;if(token===epoch){version='';build='';identityError=error instanceof Error?error.message:'Build information is unavailable.';entries=null;coverage='';noticeError='License notices are unavailable in this build.';}}
  return {revision:++revision};
}
export function mobileInformationSnapshot(query='',entryKey=''){
  const terms=query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const rows=(entries??[]).filter(entry=>terms.every(term=>[entry.name,entry.version,entry.license,...entry.bundles as string[]].join(' ').toLowerCase().includes(term))).map(entry=>({id:str(entry.id),name:str(entry.name),metadata:[str(entry.version),str(entry.license)].filter(Boolean).join(' · ')}));
  const found=entries?.find(entry=>entry.id===entryKey),detail=found?{available:true,id:str(found.id),name:str(found.name),metadata:[str(found.version),str(found.license),(found.bundles as string[]).map(bundle=>bundleNames[bundle]??bundle).join(', ')].filter(Boolean).join(' · '),sourceURL:str(found.sourceUrl),notice:str(found.noticeText)}:{available:false,id:'',name:'',metadata:'',sourceURL:'',notice:''};
  return {revision,version:version||'Unavailable',build,identityError,licensesReady:entries!==null,licenseMessage:noticeError||'No licenses match that search.',coverage,licenses:rows,detail,
    // GAP 003: no offline client-cache owner; never clear preferences/drafts instead.
    storage:{available:false,title:'Storage unavailable',detail:'Offline client cache storage is unavailable in this build.',action:'Clear caches',footer:'Clearing caches never removes environment connections, credentials, account data, or appearance preferences.'},
    // GAP 004: no startup crash journal; unavailable is not a zero-crash result.
    diagnostics:{available:false,title:'Crash log unavailable',detail:'Startup crash logging is unavailable in this build.',action:'Copy crash report',footer:'Error messages can quote values from the app. Read any crash report before sharing it.'}};
}
export async function mobileInformationCommand(op:string,value:string,native:Native|null|undefined){
  let message='';try{
    // GAP 003/004: unsupported destructive/report actions remain refused.
    if(op==='clear-cache')throw new Error('Offline client cache storage is unavailable in this build.');
    if(op==='copy-crashes')throw new Error('Startup crash logging is unavailable in this build.');
    if(op!=='license-source')throw new Error('This information action is unavailable.');
    const entry=entries?.find(entry=>entry.id===value);if(!entry||!entry.sourceUrl)throw new Error('This license source is unavailable.');
    if(!native?.available)throw new Error('Open in the native app to follow this link.');
    const url=new URL(str(entry.sourceUrl));if(!['https:','http:'].includes(url.protocol))throw new Error('This source URL cannot be opened.');
    const reply=await bridgeReply(native,{op:'mobileOpenURL',url:url.href});if(!reply.ok||obj(reply.value).opened!==true)throw new Error('The project source could not be opened.');
  }catch(error){if(letGo(error))throw error;message=error instanceof Error?error.message:'Could not complete the information action.';}
  return {revision:++revision,message};
}
/** Native view owns the WKWebView/loading UI; routes and close remain root-owned. */
export function mobileInformationLegalConfiguration(routeKey:string,closeActionID:string,colors:unknown){return JSON.stringify({routeKey,closeActionID,url:MOBILE_LEGAL_URL,allowed:MOBILE_LEGAL_DOCUMENTS,colors});}
