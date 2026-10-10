// Controlled editor revision helpers, T3 Code (MIT); see LICENSE-T3.
// Source365aa87982 src/native/composerEditorRevision.ts; helper bodies unchanged.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
export interface ComposerNativeEventSnapshot {
  readonly eventCount: number;
  readonly value: string;
  readonly selection: ComposerEditorSelection | null;
}

export interface ComposerEditorSelection {
  readonly start: number;
  readonly end: number;
}

export function acknowledgeComposerNativeEvent(
  mostRecentEventCount: number,
  incomingEventCount: number,
): number | null {
  if (!Number.isSafeInteger(incomingEventCount) || incomingEventCount < mostRecentEventCount) {
    return null;
  }
  return incomingEventCount;
}

export function resolveComposerControlledEventCount(
  value: string,
  selection: ComposerEditorSelection | null,
  mostRecentEventCount: number,
  snapshots: ReadonlyArray<ComposerNativeEventSnapshot>,
): number {
  let newestValueEventCount: number | null = null;
  for (let index = snapshots.length - 1; index >= 0; index -= 1) {
    const snapshot = snapshots[index];
    if (snapshot?.value !== value) continue;

    newestValueEventCount ??= snapshot.eventCount;
    if (selection === null || snapshotSelectionMatches(snapshot, selection)) {
      return snapshot.eventCount;
    }
  }

  // A value emitted by native paired with a different selection is an
  // intermediate React render. Keep it behind the native revision so it
  // cannot move the caret while newer keystrokes are being processed.
  if (newestValueEventCount !== null && mostRecentEventCount > 0) {
    return Math.min(newestValueEventCount, mostRecentEventCount - 1);
  }

  return mostRecentEventCount;
}

// A snapshot without a selection describes a state the editor applied itself
// (an assumed controlled document, where the native side may have bounded the
// caret). Revision stamping treats it as matching any controlled selection so
// a parent caret move on the assumed value stays at the assumed revision and
// passes the editor's staleness guard. Echo detection must not reuse this
// wildcard: an echo payload serializes `selection: null`, which would drop
// that caret move instead of applying it.
function snapshotSelectionMatches(
  snapshot: ComposerNativeEventSnapshot,
  selection: ComposerEditorSelection,
): boolean {
  if (snapshot.selection === null) return true;
  return snapshot.selection.start === selection.start && snapshot.selection.end === selection.end;
}

export function isComposerNativeEcho(
  value: string,
  selection: ComposerEditorSelection | null,
  eventCount: number,
  snapshots: ReadonlyArray<ComposerNativeEventSnapshot>,
): boolean {
  for (let index = snapshots.length - 1; index >= 0; index -= 1) {
    const snapshot = snapshots[index];
    if (
      snapshot !== undefined &&
      snapshot.eventCount === eventCount &&
      snapshot.value === value &&
      (selection === null ||
        (snapshot.selection !== null &&
          snapshot.selection.start === selection.start &&
          snapshot.selection.end === selection.end))
    ) {
      return true;
    }
  }
  return false;
}

/**
 * Records that a parent-driven controlled document was handed to the native
 * editor. From that point the acknowledged snapshot history describes a
 * superseded native state, so it is replaced with the assumed applied state;
 * a later parent update back to a previously acknowledged value must classify
 * as a fresh edit, not as a native echo the editor would drop. Native events
 * that raced past the controlled revision stay authoritative and are kept.
 */
export function assumeComposerControlledState(
  snapshots: ReadonlyArray<ComposerNativeEventSnapshot>,
  eventCount: number,
  value: string,
): ComposerNativeEventSnapshot[] {
  return [
    { eventCount, value, selection: null },
    ...snapshots.filter((snapshot) => snapshot.eventCount > eventCount),
  ];
}

export function pruneAcknowledgedComposerNativeEvents(
  snapshots: ReadonlyArray<ComposerNativeEventSnapshot>,
  acknowledgedEventCount: number,
): ComposerNativeEventSnapshot[] {
  // The newest acknowledged snapshot must survive pruning: it is what lets a
  // later, unrelated re-render classify the settled composer state as a native
  // echo instead of a parent-driven edit that would re-control the caret (and
  // reset the keyboard's autocorrect context on iOS).
  let latestAcknowledgedIndex = -1;
  for (let index = snapshots.length - 1; index >= 0; index -= 1) {
    const snapshot = snapshots[index];
    if (snapshot !== undefined && snapshot.eventCount <= acknowledgedEventCount) {
      latestAcknowledgedIndex = index;
      break;
    }
  }
  return snapshots.filter(
    (snapshot, index) =>
      index === latestAcknowledgedIndex || snapshot.eventCount > acknowledgedEventCount,
  );
}

/** App-owned envelope around the unchanged source revision helpers. Values only:
 * pure observation never persists drafts or performs native side effects. */
export interface ComposerEditorIdentity {
  owner: string; editorId: string; routeVisit: string; renderEpoch: string;
}
export interface ComposerMountedIdentity extends ComposerEditorIdentity { mountId: string }
export type ComposerEditorEventKind = 'ready' | 'text' | 'selection' | 'focus' | 'blur' | 'submit' | 'commandApplied' | 'commandRejected';
export interface ComposerEditorEvent extends ComposerMountedIdentity {
  pendingCommand?: ComposerCommandTerminal;
  kind: ComposerEditorEventKind; eventCount: number; value: string; selection: ComposerEditorSelection;
  composing: boolean; focused: boolean; alternate?: boolean; commandId?: string; commandRevision?: number; reason?: string;
}
export interface ComposerCommandTerminal {
  kind: 'commandApplied' | 'commandRejected'; commandId: string; commandRevision: number; eventCount: number;
  value: string; selection: ComposerEditorSelection; composing: boolean; focused: boolean; reason: string;
}
export interface ComposerEditorDocument { value: string; selection: ComposerEditorSelection }
export interface ComposerEditorCommand extends ComposerMountedIdentity {
  commandId: string; commandRevision: number;
  expected: ComposerEditorDocument & {eventCount:number};
  next: ComposerEditorDocument & {tokensJson:string};
}
export interface ComposerEditorEffect {
  id: string; event: ComposerEditorEvent; writeText: boolean;
  /** Matching command identity is needed to commit the runtime's staged context/settings. */
  command: ComposerEditorCommand | null;
}
export interface ComposerEditorState {
  identity: ComposerEditorIdentity; mountId: string; eventCount: number; revision: number;
  value: string; selection: ComposerEditorSelection; composing: boolean; focused: boolean;
  snapshots: ComposerNativeEventSnapshot[]; lastEvent: ComposerEditorEvent | null;
  latestEffect: ComposerEditorEffect | null; commandEffect: ComposerEditorEffect | null; stagedEventCount: number; committedEventCount: number;
  committedValue: string; stagedValue: string; pendingCommand: ComposerEditorCommand | null; lastCommandRevision: number;
  ackCommandId: string; issuedCommandIds: string[];
}
export interface ComposerEditorControl extends ComposerMountedIdentity {
  acknowledgedEventCount: number; ackCommandId: string;
  document: ComposerEditorDocument & {tokensJson:string;isNativeEcho:boolean};
  command: ComposerEditorCommand | null;
}
const MAX_DOCUMENT = 1_000_000;
const eventKinds = new Set<ComposerEditorEventKind>(['ready','text','selection','focus','blur','submit','commandApplied','commandRejected']);
const copyRange = (range:ComposerEditorSelection):ComposerEditorSelection => ({start:range.start,end:range.end});
const record = (value:unknown):Record<string,unknown> | null => typeof value==='object' && value!==null && !Array.isArray(value) ? value as Record<string,unknown> : null;
const token = (value:unknown):value is string => typeof value==='string' && value.length>0 && value.length<=4096;
const count = (value:unknown):value is number => typeof value==='number' && Number.isSafeInteger(value) && value>=0;
const rangeValid = (range:unknown,value:string):range is ComposerEditorSelection => {
  const object=record(range);
  return !!object && count(object.start) && count(object.end) && object.start<=object.end && object.end<=value.length;
};
const documentValid = (value:unknown,selection:unknown):value is string => typeof value==='string' && value.length<=MAX_DOCUMENT && rangeValid(selection,value);
const sameRange = (a:ComposerEditorSelection,b:ComposerEditorSelection) => a.start===b.start && a.end===b.end;
const sameIdentity = (a:ComposerEditorIdentity,b:ComposerEditorIdentity) => a.owner===b.owner && a.editorId===b.editorId && a.routeVisit===b.routeVisit && a.renderEpoch===b.renderEpoch;
const identityValid = (value:ComposerEditorIdentity) => token(value.owner) && token(value.editorId) && token(value.routeVisit) && token(value.renderEpoch);
const mounted = (state:ComposerEditorState):ComposerMountedIdentity => ({...state.identity,mountId:state.mountId});
const terminal = (event:ComposerEditorEvent) => event.kind==='commandApplied' || event.kind==='commandRejected';
const effectId = (event:ComposerEditorEvent) => JSON.stringify([event.renderEpoch,event.mountId,event.eventCount]);
const copyEvent = (event:ComposerEditorEvent):ComposerEditorEvent => ({...event,selection:copyRange(event.selection),
  ...(event.pendingCommand?{pendingCommand:{...event.pendingCommand,selection:copyRange(event.pendingCommand.selection)}}:{})});
const copyCommand = (command:ComposerEditorCommand):ComposerEditorCommand => ({...command,
  expected:{...command.expected,selection:copyRange(command.expected.selection)}, next:{...command.next,selection:copyRange(command.next.selection)}});

/** Decode untrusted native JSON. Ranges are source UTF16 units, not characters.
 * Do not clamp malformed coordinates into a different edit. */
export function mobileComposerEditorDecodeEvent(input:unknown):ComposerEditorEvent | null {
  let value=input;
  if(typeof value==='string') { try { value=JSON.parse(value); } catch { return null; } }
  const object=record(value);
  if(!object || !token(object.owner) || !token(object.editorId) || !token(object.routeVisit) || !token(object.renderEpoch)
    || !token(object.mountId) || !count(object.eventCount) || !eventKinds.has(object.kind as ComposerEditorEventKind)
    || !documentValid(object.value,object.selection) || typeof object.composing!=='boolean' || typeof object.focused!=='boolean') return null;
  const event:ComposerEditorEvent={owner:object.owner,editorId:object.editorId,routeVisit:object.routeVisit,renderEpoch:object.renderEpoch,
    mountId:object.mountId,eventCount:object.eventCount,kind:object.kind as ComposerEditorEventKind,value:object.value,
    selection:copyRange(object.selection as ComposerEditorSelection),composing:object.composing,focused:object.focused};
  if(event.kind==='submit') {
    if(typeof object.alternate!=='boolean') return null;
    event.alternate=object.alternate;
  }
  if(terminal(event)) {
    if(!token(object.commandId) || !count(object.commandRevision) || object.commandRevision===0 || typeof object.reason!=='string'
      || object.reason.length>4096 || (event.kind==='commandApplied' && object.reason!=='')) return null;
    event.commandId=object.commandId; event.commandRevision=object.commandRevision; event.reason=object.reason;
  }
  if(object.pendingCommand!==undefined) {
    const pending=record(object.pendingCommand);
    if(!pending || !['commandApplied','commandRejected'].includes(String(pending.kind)) || !token(pending.commandId)
      || !count(pending.commandRevision) || pending.commandRevision===0 || !count(pending.eventCount) || pending.eventCount>event.eventCount
      || !documentValid(pending.value,pending.selection) || typeof pending.composing!=='boolean' || typeof pending.focused!=='boolean'
      || typeof pending.reason!=='string' || pending.reason.length>4096 || pending.kind==='commandApplied' && pending.reason!=='') return null;
    event.pendingCommand={kind:pending.kind as ComposerCommandTerminal['kind'],commandId:pending.commandId,commandRevision:pending.commandRevision,
      eventCount:pending.eventCount,value:pending.value,selection:copyRange(pending.selection as ComposerEditorSelection),composing:pending.composing,focused:pending.focused,reason:pending.reason};
    if(event.pendingCommand.eventCount===event.eventCount && !terminal(event)) return null;
    if(terminal(event) && JSON.stringify(event.pendingCommand)!==JSON.stringify(commandTerminal(event))) return null;
  }
  return event;
}
function commandTerminal(event:ComposerEditorEvent):ComposerCommandTerminal {
  return {kind:event.kind as ComposerCommandTerminal['kind'],commandId:event.commandId!,commandRevision:event.commandRevision!,
    eventCount:event.eventCount,value:event.value,selection:copyRange(event.selection),composing:event.composing,focused:event.focused,reason:event.reason!};
}

/** A new renderEpoch is required for each native remount. Repeated admission
 * cannot reseed from lagging client.draft, even when native text equals an older value. */
export function mobileComposerEditorAdmit(prior:ComposerEditorState | null, identity:ComposerEditorIdentity,
  initial:ComposerEditorDocument):ComposerEditorState {
  if(!identityValid(identity) || !documentValid(initial.value,initial.selection)) throw new Error('Invalid composer admission.');
  if(prior && sameIdentity(prior.identity,identity)) return prior;
  return {identity:{...identity},mountId:'',eventCount:-1,revision:(prior?.revision??0)+1,value:initial.value,selection:copyRange(initial.selection),
    composing:false,focused:false,snapshots:[],lastEvent:null,latestEffect:null,commandEffect:null,stagedEventCount:-1,committedEventCount:-1,
    committedValue:initial.value,stagedValue:initial.value,pendingCommand:null,lastCommandRevision:0,ackCommandId:'',issuedCommandIds:[]};
}
export function mobileComposerEditorRetire(_state:ComposerEditorState | null):null { return null; }

export interface ComposerEditorAcceptance {
  state:ComposerEditorState; accepted:boolean; duplicate:boolean;
  effect:ComposerEditorEffect | null; commandEffect:ComposerEditorEffect | null;
}
/** Observe native text immediately. Retained command outcomes are independent of
 * later text/selection/IME events, so the raw event latch cannot lose a terminal. */
export function mobileComposerEditorAccept(state:ComposerEditorState,input:unknown):ComposerEditorAcceptance {
  const refuse=():ComposerEditorAcceptance=>({state,accepted:false,duplicate:false,effect:null,commandEffect:null});
  const event=mobileComposerEditorDecodeEvent(input);
  if(!event || !sameIdentity(state.identity,event)) return refuse();
  if(!state.mountId) {
    if(event.kind!=='ready' || event.value!==state.value || !sameRange(event.selection,state.selection) || event.composing) return refuse();
  } else if(event.mountId!==state.mountId || event.kind==='ready' && event.eventCount!==state.eventCount) return refuse();
  if(event.eventCount<state.eventCount) return refuse();
  if(event.eventCount===state.eventCount) {
    if(!state.lastEvent || JSON.stringify(event)!==JSON.stringify(state.lastEvent)) return refuse();
    return {state,accepted:true,duplicate:true,effect:state.stagedEventCount<event.eventCount?state.latestEffect:null,commandEffect:state.commandEffect};
  }
  let commandEffect=state.commandEffect;
  const outcome=event.pendingCommand ?? (terminal(event)?commandTerminal(event):null);
  if(outcome && outcome.commandId!==state.ackCommandId) {
    const command=state.pendingCommand;
    if(!command || outcome.commandId!==command.commandId || outcome.commandRevision!==command.commandRevision
      || outcome.eventCount<=command.expected.eventCount) return refuse();
    if(outcome.kind==='commandApplied' && (outcome.value!==command.next.value || !sameRange(outcome.selection,command.next.selection) || outcome.composing)) return refuse();
    const terminalEvent:ComposerEditorEvent={...mounted(state),...outcome};
    const id=JSON.stringify([event.renderEpoch,event.mountId,'command',outcome.commandId,outcome.commandRevision]);
    if(commandEffect && (commandEffect.id!==id || JSON.stringify(commandEffect.event)!==JSON.stringify(terminalEvent))) return refuse();
    commandEffect ??= {id,event:terminalEvent,writeText:false,command:copyCommand(command)};
  }
  const effect:ComposerEditorEffect={id:effectId(event),event:copyEvent(event),writeText:event.value!==state.stagedValue,command:null};
  const snapshots=pruneAcknowledgedComposerNativeEvents([...state.snapshots,
    {eventCount:event.eventCount,value:event.value,selection:copyRange(event.selection)}],event.eventCount);
  const next:ComposerEditorState={...state,mountId:event.mountId,eventCount:event.eventCount,revision:state.revision+1,value:event.value,
    selection:copyRange(event.selection),composing:event.composing,focused:event.focused,snapshots,lastEvent:copyEvent(event),latestEffect:effect,commandEffect};
  return {state:next,accepted:true,duplicate:false,effect,commandEffect};
}

/** Claim effects once. Runtime reduces the latest snapshot synchronously first.
 * It then commits command context/settings against that current text, preserving
 * undo context, before publishing the returned command acknowledgment. */
export function mobileComposerEditorStageEffect(state:ComposerEditorState,id:string):{state:ComposerEditorState;effect:ComposerEditorEffect | null} {
  if(state.commandEffect?.id===id) {
    if(state.stagedEventCount!==state.eventCount) return {state,effect:null};
    return {state:{...state,ackCommandId:state.commandEffect.event.commandId!,pendingCommand:null,commandEffect:null,revision:state.revision+1},effect:state.commandEffect};
  }
  const effect=state.latestEffect;
  if(!effect || effect.id!==id || state.stagedEventCount>=effect.event.eventCount) return {state,effect:null};
  return {state:{...state,stagedEventCount:effect.event.eventCount,stagedValue:effect.event.value,revision:state.revision+1},effect};
}
/** A durable answer may acknowledge older saved text but never replace observed
 * text/caret. Wrong-mount completions and regressing completions are ignored. */
export function mobileComposerEditorCommitted(state:ComposerEditorState,effect:ComposerEditorEffect):ComposerEditorState {
  const event=effect.event;
  if(!sameIdentity(state.identity,event) || state.mountId!==event.mountId || effect.id!==effectId(event)
    || event.eventCount>state.stagedEventCount || event.eventCount<=state.committedEventCount) return state;
  return {...state,committedEventCount:event.eventCount,committedValue:event.value};
}

/** Plain CAS intent. Staging it performs no edit and records no context. A native
 * terminal event is required before the runtime may commit associated effects. */
export function mobileComposerEditorCommand(state:ComposerEditorState,commandId:string,commandRevision:number,
  next:ComposerEditorDocument & {tokensJson:string}):{state:ComposerEditorState;command:ComposerEditorCommand | null} {
  if(!state.mountId || state.composing || state.pendingCommand || state.stagedEventCount!==state.eventCount
    || !token(commandId) || state.issuedCommandIds.includes(commandId) || !count(commandRevision) || commandRevision<=state.lastCommandRevision
    || !documentValid(next.value,next.selection) || typeof next.tokensJson!=='string') return {state,command:null};
  const command:ComposerEditorCommand={...mounted(state),commandId,commandRevision,
    expected:{eventCount:state.eventCount,value:state.value,selection:copyRange(state.selection)},
    next:{value:next.value,selection:copyRange(next.selection),tokensJson:next.tokensJson}};
  return {state:{...state,pendingCommand:command,lastCommandRevision:commandRevision,issuedCommandIds:[...state.issuedCommandIds,commandId],revision:state.revision+1},command:copyCommand(command)};
}

/** Current observed document only. Callers cannot stamp an old client snapshot
 * with a newly acknowledged eventCount. Initial props are deliberately non-echo. */
export function mobileComposerEditorControlled(state:ComposerEditorState,tokensJson='[]'):ComposerEditorControl {
  const recent=Math.max(0,state.eventCount);
  return {...mounted(state),acknowledgedEventCount:resolveComposerControlledEventCount(state.value,state.selection,recent,state.snapshots),
    document:{value:state.value,selection:copyRange(state.selection),tokensJson,
      isNativeEcho:isComposerNativeEcho(state.value,state.selection,recent,state.snapshots)},
    ackCommandId:state.ackCommandId,command:state.pendingCommand?copyCommand(state.pendingCommand):null};
}
