import type { ComposerFieldSnapshot } from './composer-field-commands';
import { cancelFreshCitation, moveFreshCitation, type ComposerChip, type FreshCitation } from './composer-source-chips';

type Surface = 'composer' | 'preview';
type Frame = readonly [number, number, number, number];
export type ComposerChipPress = {
  seq: number; open: boolean; surface: Surface; owner: string; fieldOwner: string;
  kind: string; act: string; start: number; end: number; link: string; label: string;
  name: string; path: string; context: string; frame: number[];
  anchor: 'chip' | 'source'; anchorFrame: number[]; removeOnCancel: boolean;
};
type Active = { snapshot: ComposerFieldSnapshot; press: ComposerChipPress; citation: FreshCitation | null };
const sameOwner = (a: ComposerFieldSnapshot, b: ComposerFieldSnapshot) => a.generation === b.generation && a.snapshotOwner === b.snapshotOwner && a.owner === b.owner;
const validFrame = (frame: Frame) => frame.every(Number.isFinite) && frame[2] > 0 && frame[3] > 0;
const copy = (press: ComposerChipPress): ComposerChipPress => ({ ...press, frame: [...press.frame], anchorFrame: [...press.anchorFrame] });
const actions = new Set(['mention', 'thread', 'citation', 'pull-request', 'image', 'file', 'video', 'pr-open', 'pr-draft', 'pr-merged', 'pr-closed']);

/** Product presses retain source identity and measured geometry. The shared
 *  field still owns selection, source edits and undo. `numbers` issues the press
 *  sequence; the window's composer and its side chats' share one, so a press in
 *  either field is newer than every press before it in both. */
export class ComposerChipPresses {
  private sequence = 0;
  constructor(private readonly numbers: { last: number } = { last: 0 }) {}
  private interaction = 0;
  private active: Active | null = null;
  get latest(): number { return this.sequence; }
  get interactionRevision(): number { return this.interaction; }

  observe(snapshot: ComposerFieldSnapshot, surface: Surface): void {
    const active = this.active;
    if (!active || active.press.surface !== surface) return;
    if (!snapshot.live || !sameOwner(active.snapshot, snapshot)) { this.active = null; return; }
    if (snapshot.value !== active.snapshot.value) {
      if (!active.citation) { this.active = null; return; }
      const moved = moveFreshCitation(active.citation, snapshot.value);
      if (!moved) { this.active = null; return; }
      active.citation = moved;
      active.press.start = moved.start; active.press.end = moved.end;
    }
    active.snapshot = { ...snapshot };
  }

  open(snapshot: ComposerFieldSnapshot, surface: Surface, chips: readonly ComposerChip[], target: { start: number; end: number; source: string }, frame: Frame,
    options: { comment?: boolean; fresh?: FreshCitation; sourceFrame?: Frame } = {}): ComposerChipPress | null {
    this.observe(snapshot, surface);
    if (!snapshot.live || snapshot.composing || !validFrame(frame) || options.sourceFrame && !validFrame(options.sourceFrame)) return null;
    const chip = chips.find(candidate => candidate.start === target.start && candidate.end === target.end && candidate.source === target.source);
    if (!chip || !Number.isSafeInteger(chip.start) || !Number.isSafeInteger(chip.end) || chip.start < 0 || chip.end > snapshot.value.length
      || snapshot.value.slice(chip.start, chip.end) !== chip.source) return null;
    const fresh = options.fresh;
    if (fresh && (chip.kind !== 'citation' || fresh.value !== snapshot.value || fresh.start !== chip.start || fresh.end !== chip.end || fresh.source !== chip.source)) return null;
    const kind = chip.kind === 'context' ? chip.contextKind : chip.kind;
    const details = chip.kind === 'skill' || chip.kind === 'citation' && options.comment === true
      || chip.kind === 'context' && ['terminal', 'review-comment', 'preview-annotation'].includes(kind);
    const action = surface === 'preview' || details ? '' : chip.kind === 'context' ? kind === 'folder' ? 'file' : kind : chip.kind;
    if (!details && !actions.has(action)) return null;
    this.interaction++;
    const previous = this.active;
    if (details && previous?.press.open && previous.press.surface === surface && sameOwner(previous.snapshot, snapshot)
      && previous.press.start === chip.start && previous.press.end === chip.end && previous.press.link === chip.source) {
      if (kind === 'citation') return copy(previous.press);
      this.active = null;
      return null;
    }
    if (!Number.isSafeInteger(this.numbers.last + 1)) return null;
    this.sequence = ++this.numbers.last;
    const press: ComposerChipPress = { seq: this.sequence, open: details, surface, owner: snapshot.snapshotOwner, fieldOwner: snapshot.owner,
      kind, act: action, start: chip.start, end: chip.end, link: chip.source, label: chip.label,
      name: chip.kind === 'skill' ? chip.name : chip.label, path: chip.kind === 'mention' ? chip.path : '', context: chip.kind === 'context' ? chip.contextId : '',
      frame: [...frame], anchor: options.sourceFrame ? 'source' : 'chip', anchorFrame: [...(options.sourceFrame ?? frame)], removeOnCancel: fresh !== undefined };
    const citation = chip.kind === 'citation' && details ? fresh ?? { start: chip.start, end: chip.end, source: chip.source, value: snapshot.value, spaces: { before: false, after: false } } : null;
    this.active = { snapshot: { ...snapshot }, press, citation };
    return copy(press);
  }

  read(snapshot: ComposerFieldSnapshot, surface: Surface): ComposerChipPress | null {
    this.observe(snapshot, surface);
    return this.active ? copy(this.active.press) : null;
  }
  close(seq: number): boolean {
    if (seq !== this.sequence) return false;
    // Explicit dismissal also cancels a pending effect after its source edit
    // invalidated the old press. Source observation itself is not dismissal.
    this.interaction++;
    return this.finish(seq);
  }
  finish(seq: number): boolean {
    if (this.active?.press.seq !== seq) return false;
    this.active = null;
    return true;
  }
  take(snapshot: ComposerFieldSnapshot, seq: number): ComposerChipPress | null {
    this.observe(snapshot, 'composer');
    const press = this.active?.press;
    if (!press || press.seq !== seq || press.surface !== 'composer' || press.open || !press.act) return null;
    this.active = null;
    return copy(press);
  }
  cancel(snapshot: ComposerFieldSnapshot, seq: number) {
    this.observe(snapshot, 'composer');
    const active = this.active;
    return active?.press.seq === seq && active.press.surface === 'composer' && active.press.removeOnCancel && active.citation
      ? cancelFreshCitation(active.citation, snapshot.value) : null;
  }
}
