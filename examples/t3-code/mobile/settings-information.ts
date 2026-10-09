// @ref llp/1109.009-mobile-settings.decision.md#information-sources
// Mobile365aa87982 About, Legal, licenses, Diagnostics and Client Storage routes.
import { obj, str, type Obj } from './shared/domain';
import { decodeNotices } from './shared/settings-data';
import { bridgeReply, type Native } from './shared/protocol';
import { letGo } from './shared/let-go';
import { mobileCacheClear, mobileCacheInspect, type MobileCacheSummary, type MobileCacheKind } from './mobile-client-cache';
import { mobileTheme } from './design';

export interface MobileStorageEnvironment { environmentId: string; label: string; machine: string }
export interface MobileInformationCommandOptions {
  routeKey?: string;
  environments?: readonly MobileStorageEnvironment[];
  current?: () => boolean;
  /** Invocation-owned integration clears runtime owners and the dedicated disk cache. */
  clearCache?: (native: Native, environmentId?: string) => Promise<unknown>;
}
export const MOBILE_STORAGE_ERROR = 'Client storage is temporarily unavailable. Try again after restarting the app.';
const storageFooter = 'Clearing caches never removes environment connections, credentials, account data, or appearance preferences.';
const machineSymbols: Record<string, string> = { server: 'server.rack', cloud: 'cloud', linux: 'terminal', desktop: 'desktopcomputer', laptop: 'laptopcomputer', 'mac-mini': 'macmini', 'mac-studio': 'macstudio' };
const machineLabels: Record<string, string> = { server: 'Server', cloud: 'Cloud VM', linux: 'Linux/WSL', desktop: 'Desktop', laptop: 'Laptop', 'mac-mini': 'Mini PC', 'mac-studio': 'Workstation' };
export function mobileStorageBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10 * 1024 ? 1 : 0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(bytes < 10 * 1024 * 1024 ? 1 : 0)} MB`;
}
export function mobileStorageAggregate(summary: MobileCacheSummary) {
  const environments = new Map<string, { environmentId: string; recordCount: number; payloadBytes: number; kinds: Partial<Record<MobileCacheKind, number>> }>();
  for (const row of summary.rows) {
    const prior = environments.get(row.environmentId) ?? { environmentId: row.environmentId, recordCount: 0, payloadBytes: 0, kinds: {} };
    environments.set(row.environmentId, { environmentId: row.environmentId,
      recordCount: prior.recordCount + row.recordCount, payloadBytes: prior.payloadBytes + row.payloadBytes,
      kinds: { ...prior.kinds, [row.kind]: row.recordCount } });
  }
  return { recordCount: summary.recordCount, payloadBytes: summary.payloadBytes, environments: [...environments.values()] };
}

/** Local read/action state only; no native handle, callback or promise is retained. */
export class MobileClientStorageState {
  private summary: ReturnType<typeof mobileStorageAggregate> | null = null;
  private readSerial = 0;
  private visit = 0;
  private routeKey = '';
  private active = false;
  private readFailed = false;
  private clearFailed = false;
  private commandBusy = false;
  private clearing = false;
  revision = 0;

  enter(route: string, routeKey = route): void {
    const active = route === 'settingsClientStorage';
    if (this.routeKey !== routeKey || this.active !== active) {
      this.routeKey = routeKey; this.active = active; this.visit++; this.readSerial++;
    }
  }
  async prepare(native: Native | null | undefined): Promise<void> {
    const serial = ++this.readSerial, visit = this.visit;
    try {
      if (!native?.available) throw new Error('Client cache unavailable');
      const summary = await mobileCacheInspect(native);
      if (!this.active || visit !== this.visit || serial !== this.readSerial) return;
      this.summary = mobileStorageAggregate(summary); this.readFailed = false; this.revision++;
    } catch (error) {
      if (letGo(error)) throw error;
      if (this.active && visit === this.visit && serial === this.readSerial) {
        this.summary = null; this.readFailed = true; this.revision++;
      }
    }
  }
  snapshot(environments: readonly MobileStorageEnvironment[] = [], pending = false, danger = mobileTheme('light').colors.dangerForeground) {
    const facts = new Map(environments.map(row => [row.environmentId, row]));
    const clearing = this.clearing || pending, available = this.summary !== null;
    const rows = [...(this.summary?.environments ?? [])].sort((left, right) =>
      (facts.get(left.environmentId)?.label ?? '').localeCompare(facts.get(right.environmentId)?.label ?? ''));
    return { available, loading: !available && !this.readFailed, clearing, danger,
      title: this.readFailed ? 'Storage unavailable' : available ? 'No cached data' : '',
      detail: this.readFailed ? 'Restart the app and try again.' : available
        ? 'Offline cache records will appear here after environments are used.' : 'Inspecting cached data…',
      action: available ? `Clear ${mobileStorageBytes(this.summary!.payloadBytes)}` : 'Clear caches',
      disabled: clearing || !available || this.summary!.recordCount === 0,
      footer: storageFooter, error: this.readFailed || this.clearFailed ? MOBILE_STORAGE_ERROR : '',
      recordCount: this.summary?.recordCount ?? 0, payloadBytes: this.summary?.payloadBytes ?? 0,
      environments: rows.map((row, index) => {
        const fact = facts.get(row.environmentId), machine = fact?.machine ?? 'server';
        return { id: row.environmentId, label: fact?.label ?? row.environmentId,
          machine: machineSymbols[machine] ?? machineSymbols.server!, machineLabel: machineLabels[machine] ?? machineLabels.server!,
          recordCount: row.recordCount, payloadBytes: row.payloadBytes, first: index === 0,
          action: `Clear ${mobileStorageBytes(row.payloadBytes)}` };
      }) };
  }
  async command(op: string, value: string, native: Native | null | undefined, options: MobileInformationCommandOptions = {}) {
    if (!this.active || options.routeKey !== undefined && options.routeKey !== this.routeKey || this.commandBusy
      || !this.summary || this.summary.recordCount === 0 || !native?.available || options.current?.() === false) return;
    const environmentId = op === 'clear-cache-environment' ? value : undefined;
    if (op !== 'clear-cache-all' && op !== 'clear-cache-environment'
      || environmentId !== undefined && !this.summary.environments.some(row => row.environmentId === environmentId)) return;
    const visit = this.visit, current = () => this.active && this.visit === visit && options.current?.() !== false;
    const label = options.environments?.find(row => row.environmentId === environmentId)?.label ?? environmentId;
    this.commandBusy = true;
    try {
      const confirmation = await bridgeReply(native, { op: 'mobileAlert',
        kind: environmentId === undefined ? 'clear-client-caches' : 'clear-client-cache',
        title: environmentId === undefined ? 'Clear all client caches?' : `Clear cache for ${label}?`,
        message: environmentId === undefined
          ? 'This removes offline data for every environment. Connections, credentials, account data, and app preferences stay intact.'
          : 'This removes offline threads, server metadata, and cached branches for this environment. The saved connection and credentials stay intact.' });
      if (!current()) return;
      if (!confirmation.ok) throw new Error('Cache confirmation unavailable');
      if (obj(confirmation.value).choice !== 'clear') return;
      this.clearing = true; this.clearFailed = false; this.readSerial++; this.revision++;
      if (options.clearCache) await options.clearCache(native, environmentId);
      else await mobileCacheClear(native, environmentId === undefined ? {} : { environmentId });
      // A later route visit may already have inspected before deletion finished.
      // Its next owned prepare must read the committed store, never those counts.
      this.summary = null; this.readFailed = false; this.readSerial++; this.revision++;
      if (!current()) return;
      await this.prepare(native);
    } catch (error) {
      if (letGo(error)) throw error;
      if (current()) { this.clearFailed = true; this.revision++; }
    } finally { this.commandBusy = false; this.clearing = false; }
  }
}
const clientStorage = new MobileClientStorageState();
export const MOBILE_INFORMATION_ROUTES=['settingsAbout','settingsClientStorage','settingsDiagnostics','settingsOpenSourceLicenses','settingsOpenSourceLicense','settingsLegal'];
export const MOBILE_LEGAL_URL='https://t3.codes/legal';
export const MOBILE_LEGAL_DOCUMENTS=['legal','privacy-policy','terms-of-service','security-policy'].map(path=>`https://t3.codes/${path}`);
export function mobileLegalDocument(value:string){try{const url=new URL(value);return ['http:','https:'].includes(url.protocol)&&MOBILE_LEGAL_DOCUMENTS.includes(`${url.origin}${url.pathname.replace(/\/+$/,'')||'/'}`);}catch{return false;}}
const bundleNames:Record<string,string>={mobile:'Mobile',ios:'iOS',assets:'Assets',web:'Web',server:'Server',desktop:'Desktop','device-tools':'Device tools'};
let version='',build='',identityError='',noticeError='',coverage='',entries:Obj[]|null=null,revision=0,epoch=0;
/** Reads local bundle identity/notices only. No connected server or account scope applies. */
export async function mobileInformationPrepare(route:string,native:Native|null|undefined,routeKey=route){
  clientStorage.enter(route,routeKey);
  const token=++epoch;if(!MOBILE_INFORMATION_ROUTES.includes(route))return {revision};
  if(route==='settingsClientStorage'){await clientStorage.prepare(native);return {revision:++revision};}
  try{
    if(!native?.available)throw new Error('Open in the native app to read this build’s information.');
    const reply=await bridgeReply(native,{op:'mobileInformation'});if(!reply.ok)throw new Error(str(obj(reply.error).message,'Build information is unavailable.'));
    if(token!==epoch)return {revision};const data=obj(reply.value);version=str(data.version);build=str(data.build);identityError='';coverage=str(data.noticeCoverage);noticeError='';
    try{entries=decodeNotices(obj(data.notices));}catch{entries=null;noticeError='License notices are unavailable in this build.';}
  }catch(error){if(letGo(error))throw error;if(token===epoch){version='';build='';identityError=error instanceof Error?error.message:'Build information is unavailable.';entries=null;coverage='';noticeError='License notices are unavailable in this build.';}}
  return {revision:++revision};
}
export function mobileInformationSnapshot(query='',entryKey='',environments:readonly MobileStorageEnvironment[]=[],clearing=false,danger=mobileTheme('light').colors.dangerForeground){
  const terms=query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const rows=(entries??[]).filter(entry=>terms.every(term=>[entry.name,entry.version,entry.license,...entry.bundles as string[]].join(' ').toLowerCase().includes(term))).map(entry=>({id:str(entry.id),name:str(entry.name),metadata:[str(entry.version),str(entry.license)].filter(Boolean).join(' · ')}));
  const found=entries?.find(entry=>entry.id===entryKey),detail=found?{available:true,id:str(found.id),name:str(found.name),metadata:[str(found.version),str(found.license),(found.bundles as string[]).map(bundle=>bundleNames[bundle]??bundle).join(', ')].filter(Boolean).join(' · '),sourceURL:str(found.sourceUrl),notice:str(found.noticeText)}:{available:false,id:'',name:'',metadata:'',sourceURL:'',notice:''};
  return {revision,version:version||'Unavailable',build,identityError,licensesReady:entries!==null,licenseMessage:noticeError||'No licenses match that search.',coverage,licenses:rows,detail,
    storage:clientStorage.snapshot(environments,clearing,danger),
    // GAP 004: no startup crash journal; unavailable is not a zero-crash result.
    diagnostics:{available:false,title:'Crash log unavailable',detail:'Startup crash logging is unavailable in this build.',action:'Copy crash report',footer:'Error messages can quote values from the app. Read any crash report before sharing it.'}};
}
export async function mobileInformationCommand(op:string,value:string,native:Native|null|undefined,options:MobileInformationCommandOptions={}){
  if(op==='clear-cache-all'||op==='clear-cache-environment'){
    await clientStorage.command(op,value,native,options);return {revision:++revision,message:''};
  }
  let message='';try{
    // Legacy unspecific cache action has no valid confirmation scope.
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
