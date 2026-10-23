// Live grammar identity belongs to the composer, independently of its serialized source.
import { codeAt, composerLines, composerMarkdown, emptyFenceAt, parseOpeningFence, type ComposerBlockOrigin } from './composer-markdown';
import { composerBlockInputRule } from './composer-input-rules';
import { applyComposerEdit, composerCodeInput, composerMarkdownKey, type ComposerEdit, type ComposerKey, type ComposerKeyResult } from './composer-markdown-edit';
import { listContinuationForEnter, listIndentForTab } from './composer-list-continuation';
import { composerSurround } from './composer-surround';
import type { ComposerRange } from './composer-markdown';
import { composerCharacterDeletion, composerChipDeletion, composerChipWordDeletion, composerChips, composerChipsInBlocks, composerWordDeletion, outsideComposerChips, reconcileComposerChips, type ComposerChip, type SkillOffer } from './composer-source-chips';
import type { ComposerBlock } from './composer-markdown';

type FenceOrigins = Map<number, string>;
type SourceOrigins = { chips: readonly ComposerChip[]; blocks: readonly ComposerBlockOrigin[]; fences: ReadonlyMap<number, string> };
type SourceChange = ComposerRange & { text: string };
type Pending = { value: string; origins: FenceOrigins; blocks: ComposerBlockOrigin[] };
export type ComposerMode = 'rich' | 'plain' | 'literal';
export type ComposerSessionResult = ComposerKeyResult | { kind: 'stale' };

/** Stores grammar metadata, never an undo stack or a second editable value. */
export class ComposerSourceSession {
  private owner = '';
  private currentMode: ComposerMode = 'rich';
  private value = '';
  private origins: FenceOrigins = new Map();
  private pending: Pending | null = null;
  private currentBlocks: ComposerBlockOrigin[] = [];
  private currentChips: ComposerChip[] = [];
  private offer: SkillOffer = null;
  private offerKey: string | null = null;
  // Exact source signatures retain block and reference origins for native history, not edit commands.
  private historyOrigins = new Map<string, SourceOrigins>();

  matches(owner: string, value: string): boolean { return owner === this.owner && value === this.value; }
  get chips(): readonly ComposerChip[] { return this.currentChips; }
  get mode(): ComposerMode { return this.currentMode; }
  get blockOrigins(): readonly ComposerBlockOrigin[] { return this.currentBlocks; }

  /** `skills`: the names the provider offers users, which alone become skill chips; null: every `$name`. */
  observe(owner: string, value: string, inputType = '', mode: ComposerMode = 'rich', skills: readonly string[] | null = null): void {
    const offerKey = skills ? skills.join('\n') : null;
    if (offerKey !== this.offerKey) {
      const names = skills ? new Set(skills) : null;
      this.offerKey = offerKey;
      this.offer = names ? name => names.has(name) : null;
      // The provider changed what it offers: chips follow it without touching the source or its history.
      if (this.currentMode !== 'literal') this.currentChips = this.offered(this.value, this.currentChips);
    }
    const enteredRichText = this.currentMode === 'literal' && mode !== 'literal';
    if (mode !== this.currentMode) { this.origins = new Map(); this.pending = null; this.currentBlocks = []; }
    this.currentMode = mode;
    if (owner !== this.owner) {
      this.owner = owner;
      this.value = value;
      this.origins = new Map();
      this.currentBlocks = [];
      this.historyOrigins.clear();
      this.currentChips = mode === 'literal' ? [] : composerChips(value, this.offer);
      this.pending = null;
      return;
    }
    if (value === this.value) {
      if (mode === 'literal') this.currentChips = [];
      else if (enteredRichText) this.currentChips = composerChips(value, this.offer);
      return;
    }
    this.rememberOrigins(this.value, this.currentChips, this.currentBlocks, this.origins);
    if (this.pending?.value === value) {
      this.origins = this.pending.origins;
      this.currentBlocks = this.pending.blocks;
    } else {
      const old = this.value;
      let prefix = 0, suffix = 0;
      while (prefix < old.length && prefix < value.length && old[prefix] === value[prefix]) prefix++;
      while (suffix < old.length - prefix && suffix < value.length - prefix && old[old.length - suffix - 1] === value[value.length - suffix - 1]) suffix++;
      const oldEnd = old.length - suffix, delta = value.length - old.length;
      const change = { start: prefix, end: oldEnd, text: value.slice(prefix, value.length - suffix) };
      const live = mode === 'rich' && inputType === 'insertText' && change.text.length > 0
        && !change.text.includes('\n') && !old.slice(prefix, oldEnd).includes('\n');
      const blocksAfter = this.blocksAfter(change, value, live || inputType.startsWith('delete'));
      this.currentBlocks = live ? this.typedBlocks(change, value, blocksAfter) : blocksAfter;
      const next: FenceOrigins = new Map(), blocks = composerMarkdown(value).blocks;
      for (const block of composerMarkdown(old).blocks) {
        if (block.kind !== 'code') continue;
        const start = block.start >= oldEnd ? block.start + delta : block.start;
        const candidate = blocks.find(item => item.kind === 'code' && item.start === start);
        if (candidate?.kind !== 'code' || candidate.info !== block.info || candidate.fence.charAt(0) !== block.fence.charAt(0)) continue;
        const history = inputType === 'historyUndo' || inputType === 'historyRedo';
        if (block.open.end <= prefix || block.start >= oldEnd || history) next.set(start, this.origins.get(block.start) ?? block.fence);
      }
      this.origins = next;
    }
    this.currentChips = mode === 'literal' ? [] : enteredRichText ? composerChips(value, this.offer)
      : reconcileComposerChips(this.value, value, this.currentChips, this.offer);
    if (inputType === 'historyUndo' || inputType === 'historyRedo') {
      const remembered = this.historyOrigins.get(value);
      this.currentBlocks = mode === 'rich' ? remembered?.blocks.map(block => ({ ...block })) ?? [] : [];
      this.origins = mode === 'rich' ? new Map(remembered?.fences) : new Map();
      if (remembered && mode !== 'literal') this.currentChips = this.offered(value, remembered.chips.map(chip => ({ ...chip })));
    }
    this.rememberOrigins(value, this.currentChips, this.currentBlocks, this.origins);
    this.value = value;
    this.pending = null;
  }

  input(owner: string, value: string, selection: ComposerRange, text: string, paste = false): ComposerSessionResult {
    if (!this.matches(owner, value)) return { kind: 'stale' };
    const blocks = composerMarkdown(value, { styling: this.mode === 'rich', blockOrigins: this.currentBlocks }).blocks;
    const surround = paste || this.mode === 'literal' ? null : composerSurround(value, selection, text, this.sourceChips(value, blocks), this.mode === 'rich' ? blocks : []);
    if (surround) {
      this.pending = { value: applyComposerEdit(value, surround), origins: this.after(surround), blocks: this.blocksAfter(surround, applyComposerEdit(value, surround), true) };
      return { kind: 'edit', edit: surround };
    }
    // Text typed, pasted or dropped inside a reference goes after it.
    selection = outsideComposerChips(selection, this.sourceChips(value, blocks));
    const code = codeAt(blocks, selection) ?? emptyFenceAt(blocks, selection);
    const outcome = composerCodeInput(value, selection, text, blocks);
    if (outcome.kind === 'edit') {
      const origins = this.after(outcome.edit);
      if (code) origins.set(code.start, this.origins.get(code.start) ?? code.fence);
      const next = applyComposerEdit(value, outcome.edit);
      this.pending = { value: next, origins, blocks: this.blocksAfter(outcome.edit, next, !paste) };
    }
    return outcome;
  }

  key(owner: string, value: string, selection: ComposerRange, key: ComposerKey): ComposerSessionResult {
    if (!this.matches(owner, value)) return { kind: 'stale' };
    const parsed = composerMarkdown(value, { styling: this.mode === 'rich', blockOrigins: this.currentBlocks }).blocks;
    // Backspace and Delete (`Word…`: with Option) reach here at a reference. One that was
    // admitted at a reference and waited while an earlier edit removed it deletes as the field
    // would have, so a queued second press is never lost.
    const deletes = /^(Word)?(Backspace|Delete)$/.exec(key.name);
    if (deletes) {
      const word = deletes[1] !== undefined, backward = deletes[2] === 'Backspace', chips = this.sourceChips(value, parsed);
      const chip = word ? composerChipWordDeletion(value, selection, chips, backward) : composerChipDeletion(selection, chips, backward);
      if (chip) return this.deleted(value, chip);
      if (word) return this.deleted(value, composerWordDeletion(value, selection, backward));
      if (this.mode !== 'rich') return this.deleted(value, composerCharacterDeletion(value, selection, backward));
    }
    if (this.mode !== 'rich') {
      // The plain composer continues and indents Markdown lists typed as text; a literal answer does not.
      const list = this.mode === 'plain' && !key.command
        ? key.name === 'Enter' && key.shift ? listContinuationForEnter(value, selection.start, this.sourceChips(value, parsed))
          : key.name === 'Tab' && !key.shift ? listIndentForTab(value, selection.start, selection.end, this.sourceChips(value, parsed)) : null
        : null;
      if (list && (key.name === 'Tab' || selection.start === selection.end)) return this.deleted(value, list);
      if (key.name !== 'Enter') return { kind: 'default' };
      if (key.command || !key.shift) return { kind: 'send' };
      return this.input(owner, value, selection, '\n');
    }
    const blocks = parsed, code = codeAt(blocks, selection) ?? emptyFenceAt(blocks, selection);
    const outcome = composerMarkdownKey(value, selection, key, code ? this.origins.get(code.start) : undefined, this.currentBlocks, this.sourceChips(value, parsed));
    if (outcome.kind === 'default' && deletes) return this.deleted(value, composerCharacterDeletion(value, selection, deletes[2] === 'Backspace'));
    if (outcome.kind === 'edit') {
      const next = applyComposerEdit(value, outcome.edit), origins = this.after(outcome.edit);
      if (code && composerMarkdown(next).blocks.some(block => block.kind === 'code' && block.start === code.start)) {
        origins.set(code.start, this.origins.get(code.start) ?? code.fence);
      }
      let blocks = this.blocksAfter(outcome.edit, next, true);
      // The admitted Enter gesture replaces exactly the opening line. Release
      // its live suppression only after the shared edit has been accepted.
      if (key.name === 'Enter' && parseOpeningFence(value.slice(outcome.edit.start, outcome.edit.end))) {
        blocks = blocks.filter(origin => origin.start !== outcome.edit.start);
      }
      this.pending = { value: next, origins, blocks };
    }
    return outcome;
  }

  /** `chips` with the skills the provider no longer offers dropped and the ones it now offers added. */
  private offered(value: string, chips: readonly ComposerChip[]): ComposerChip[] {
    const kept = chips.filter(chip => chip.kind !== 'skill' || !this.offer || this.offer(chip.name));
    const added = composerChips(value, this.offer).filter(chip => chip.kind === 'skill' && !kept.some(old => chip.start < old.end && chip.end > old.start));
    return [...kept, ...added].sort((a, b) => a.start - b.start);
  }

  /** The references the projection draws: none in code or a rule. */
  private sourceChips(value: string, blocks: readonly ComposerBlock[]): ComposerChip[] {
    return this.mode === 'literal' ? [] : composerChipsInBlocks(value, blocks, this.currentChips);
  }

  /** A deletion or other app edit as one guarded edit; `handled` when there is nothing to do. */
  private deleted(value: string, edit: ComposerEdit | null): ComposerSessionResult {
    if (!edit) return { kind: 'handled' };
    const next = applyComposerEdit(value, edit);
    this.pending = { value: next, origins: this.after(edit), blocks: this.blocksAfter(edit, next, true) };
    return { kind: 'edit', edit };
  }

  private after(edit: ComposerEdit): FenceOrigins {
    const next: FenceOrigins = new Map(), delta = edit.text.length - (edit.end - edit.start);
    for (const [start, fence] of this.origins) {
      if (start < edit.start) next.set(start, fence);
      else if (start >= edit.end && edit.start !== edit.end) next.set(start + delta, fence);
      else if (start >= edit.start && edit.start === edit.end) next.set(start + delta, fence);
    }
    return next;
  }

  private blocksAfter(edit: SourceChange, value: string, preserveTouched: boolean): ComposerBlockOrigin[] {
    if (value.length > 262_144) return [];
    const delta = edit.text.length - (edit.end - edit.start), next: ComposerBlockOrigin[] = [];
    const lines = new Map(composerLines(this.value).map(line => [line.start, line]));
    const nextLines = new Map(composerLines(value).map(line => [line.start, line]));
    const rawBlocks = new Map(composerMarkdown(value).blocks.map(block => [block.start, block]));
    for (const origin of this.currentBlocks) {
      const line = lines.get(origin.start);
      if (!line) continue;
      let start = origin.start;
      if (edit.end <= start && (edit.start < start || edit.text.endsWith('\n'))) start += delta;
      else if (edit.start <= line.end) {
        const appendedLine = edit.start === line.end && edit.end === line.end && edit.text.startsWith('\n');
        if (!preserveTouched && !appendedLine || edit.start < start) continue;
      }
      if (start !== 0 && value.charAt(start - 1) !== '\n') continue;
      const nextLine = nextLines.get(start);
      if (!nextLine) continue;
      if (origin.kind === 'paragraph') {
        // The full source matters: a fence before an existing newline already
        // parses as code, although its live Enter gesture has not happened.
        if (rawBlocks.get(start)?.kind === 'paragraph') continue;
      } else {
        const block = composerMarkdown(nextLine.text, { blockOrigins: [{ start: 0, kind: origin.kind }] }).blocks[0];
        if (block?.kind !== 'list' || block.list !== origin.kind) continue;
      }
      next.push({ start, kind: origin.kind });
    }
    return next;
  }

  private typedBlocks(edit: SourceChange, value: string, rebased: ComposerBlockOrigin[]): ComposerBlockOrigin[] {
    if (value.length > 262_144) return [];
    const previous = composerMarkdown(this.value, { blockOrigins: this.currentBlocks }).blocks
      .find(block => block.start <= edit.start && block.end >= edit.end);
    const end = edit.start + edit.text.length;
    const line = composerLines(value).find(line => line.start <= end && line.end >= end);
    if (!previous || !line) return rebased;
    let kind: ComposerBlockOrigin['kind'] | undefined;
    if (previous.kind === 'paragraph') {
      const rule = composerBlockInputRule(value.slice(line.start, end), { kind: 'paragraph' });
      if (rule?.kind === 'task') kind = 'task';
      else if (!rule && composerMarkdown(value).blocks.find(block => block.start === line.start)?.kind !== 'paragraph') kind = 'paragraph';
    } else if (previous.kind === 'list' && previous.list === 'bullet') {
      const prefix = /^([ \t]*)([-*+])([ \t]+)/.exec(line.text);
      if (!prefix || prefix[2] !== previous.marker) return rebased;
      const rule = composerBlockInputRule(value.slice(line.start + prefix[0].length, end), { kind: 'list', marker: previous.marker });
      // A checkbox remains the bullet's literal body until its final space.
      kind = rule?.kind === 'task' ? 'task' : 'bullet';
    } else return rebased;
    const next = rebased.filter(origin => origin.start !== line.start);
    if (kind) next.push({ start: line.start, kind });
    return next.slice(-1024);
  }

  private rememberOrigins(value: string, chips: readonly ComposerChip[], blocks: readonly ComposerBlockOrigin[], fences: ReadonlyMap<number, string>): void {
    this.historyOrigins.delete(value);
    if (!chips.length && !blocks.length && !fences.size || value.length > 262_144 || chips.length + blocks.length + fences.size > 1024) return;
    this.historyOrigins.set(value, { chips: chips.map(chip => ({ ...chip })), blocks: blocks.map(block => ({ ...block })), fences: new Map(fences) });
    let characters = 0, count = 0;
    for (const [source, retained] of this.historyOrigins) { characters += source.length; count += retained.chips.length + retained.blocks.length + retained.fences.size; }
    for (const [source, retained] of this.historyOrigins) {
      if (this.historyOrigins.size <= 32 && characters <= 262_144 && count <= 1024) break;
      this.historyOrigins.delete(source);
      characters -= source.length; count -= retained.chips.length + retained.blocks.length + retained.fences.size;
    }
  }
}
