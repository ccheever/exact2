// Live fence identity belongs to the composer, independently of its serialized source.
import { codeAt, composerMarkdown } from './composer-markdown';
import { applyComposerEdit, composerCodeInput, composerMarkdownKey, type ComposerEdit, type ComposerKey, type ComposerKeyResult } from './composer-markdown-edit';
import type { ComposerRange } from './composer-markdown-inline';
import { composerChips, pastedPathQueryLength, reconcileComposerChips, type ComposerChip } from './composer-source-chips';

type FenceOrigins = Map<number, string>;
type SourceOrigins = { literal: readonly ComposerRange[]; chips: readonly ComposerChip[] };
type Pending = { value: string; origins: FenceOrigins; literal: ComposerRange[] };
export type ComposerMode = 'rich' | 'plain' | 'literal';
export type ComposerSessionResult = ComposerKeyResult | { kind: 'stale' };

/** Stores grammar metadata, never an undo stack or a second editable value. */
export class ComposerSourceSession {
  private owner = '';
  private currentMode: ComposerMode = 'rich';
  private value = '';
  private origins: FenceOrigins = new Map();
  private pending: Pending | null = null;
  private literal: ComposerRange[] = [];
  private currentChips: ComposerChip[] = [];
  // Exact source signatures retain decoration origins for native history, not edit commands.
  private historyOrigins = new Map<string, SourceOrigins>();

  matches(owner: string, value: string): boolean { return owner === this.owner && value === this.value; }
  get chips(): readonly ComposerChip[] { return this.currentChips; }
  get mode(): ComposerMode { return this.currentMode; }
  get literalRanges(): readonly ComposerRange[] { return this.literal; }

  observe(owner: string, value: string, inputType = '', mode: ComposerMode = 'rich'): void {
    const enteredRichText = this.currentMode === 'literal' && mode !== 'literal';
    if (mode !== this.currentMode) { this.origins = new Map(); this.pending = null; }
    this.currentMode = mode;
    if (owner !== this.owner) {
      this.owner = owner;
      this.value = value;
      this.origins = new Map();
      this.literal = [];
      this.historyOrigins.clear();
      this.currentChips = mode === 'literal' ? [] : composerChips(value);
      this.pending = null;
      return;
    }
    if (value === this.value) {
      if (mode === 'literal') this.currentChips = [];
      else if (enteredRichText) this.currentChips = composerChips(value);
      return;
    }
    this.rememberOrigins(this.value, this.literal, this.currentChips);
    if (this.pending?.value === value) {
      this.origins = this.pending.origins;
      this.literal = this.pending.literal;
    } else {
      const old = this.value;
      let prefix = 0, suffix = 0;
      while (prefix < old.length && prefix < value.length && old[prefix] === value[prefix]) prefix++;
      while (suffix < old.length - prefix && suffix < value.length - prefix && old[old.length - suffix - 1] === value[value.length - suffix - 1]) suffix++;
      const oldEnd = old.length - suffix, delta = value.length - old.length;
      this.literal = this.literalAfter({ start: prefix, end: oldEnd, text: value.slice(prefix, value.length - suffix) }, value);
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
    this.currentChips = mode === 'literal' ? [] : enteredRichText ? composerChips(value)
      : reconcileComposerChips(this.value, value, this.currentChips);
    if (inputType === 'historyUndo' || inputType === 'historyRedo') {
      const remembered = this.historyOrigins.get(value);
      if (remembered) {
        this.literal = remembered.literal.map(range => ({ ...range }));
        if (mode !== 'literal') this.currentChips = remembered.chips.map(chip => ({ ...chip }));
      }
    }
    this.rememberOrigins(value, this.literal, this.currentChips);
    this.value = value;
    this.pending = null;
  }

  input(owner: string, value: string, selection: ComposerRange, text: string, paste = false): ComposerSessionResult {
    if (!this.matches(owner, value)) return { kind: 'stale' };
    const blocks = composerMarkdown(value, { styling: this.mode === 'rich' }).blocks, code = codeAt(blocks, selection);
    const outcome = composerCodeInput(value, selection, text, blocks);
    if (outcome.kind === 'edit') {
      const origins = this.after(outcome.edit);
      if (code) origins.set(code.start, this.origins.get(code.start) ?? code.fence);
      const next = applyComposerEdit(value, outcome.edit), literal = this.literalAfter(outcome.edit, next);
      const prefix = paste ? pastedPathQueryLength(value.slice(0, selection.start), text, selection.start === selection.end) : 0;
      if (prefix) literal.push({ start: selection.start, end: selection.start + prefix });
      this.pending = { value: next, origins, literal };
    }
    return outcome;
  }

  key(owner: string, value: string, selection: ComposerRange, key: ComposerKey): ComposerSessionResult {
    if (!this.matches(owner, value)) return { kind: 'stale' };
    if (this.mode !== 'rich') {
      if (key.name !== 'Enter') return { kind: 'default' };
      if (key.command || !key.shift) return { kind: 'send' };
      return this.input(owner, value, selection, '\n');
    }
    const code = codeAt(composerMarkdown(value).blocks, selection);
    const outcome = composerMarkdownKey(value, selection, key, code ? this.origins.get(code.start) : undefined);
    if (outcome.kind === 'edit') {
      const next = applyComposerEdit(value, outcome.edit), origins = this.after(outcome.edit);
      if (code && composerMarkdown(next).blocks.some(block => block.kind === 'code' && block.start === code.start)) {
        origins.set(code.start, this.origins.get(code.start) ?? code.fence);
      }
      this.pending = { value: next, origins, literal: this.literalAfter(outcome.edit, next) };
    }
    return outcome;
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

  private rememberOrigins(value: string, literal: readonly ComposerRange[], chips: readonly ComposerChip[]): void {
    this.historyOrigins.delete(value);
    if (!literal.length && !chips.length || value.length > 262_144 || literal.length + chips.length > 1024) return;
    this.historyOrigins.set(value, { literal: literal.map(range => ({ ...range })), chips: chips.map(chip => ({ ...chip })) });
    let characters = 0, count = 0;
    for (const [source, retained] of this.historyOrigins) { characters += source.length; count += retained.literal.length + retained.chips.length; }
    for (const [source, retained] of this.historyOrigins) {
      if (this.historyOrigins.size <= 32 && characters <= 262_144 && count <= 1024) break;
      this.historyOrigins.delete(source);
      characters -= source.length; count -= retained.literal.length + retained.chips.length;
    }
  }

  private literalAfter(edit: ComposerRange & { text: string }, value: string): ComposerRange[] {
    const delta = edit.text.length - (edit.end - edit.start), ranges: ComposerRange[] = [];
    for (const old of this.literal) {
      let start = old.start, end = old.end;
      if (edit.end <= start) { start += delta; end += delta; }
      else if (edit.start < end) {
        if (edit.start <= start && edit.end >= end) continue;
        start = edit.start < start ? edit.start + edit.text.length : start;
        end = edit.end >= end ? edit.start : end + delta;
      }
      const whitespace = value.slice(start, end).search(/\s/);
      if (whitespace >= 0) end = start + whitespace;
      if (end > start) ranges.push({ start, end });
    }
    return ranges;
  }
}
