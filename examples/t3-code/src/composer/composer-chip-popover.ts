// A skill chip's details popover, adapted
// from T3 Code 1e2ecbd975 (MIT, LICENSE-T3): ComposerPromptEditorTiptap.tsx ComposerSkillNodeView
// wraps each skill chip in a ContextChipPopover (contextChipParts.tsx): a press opens a popover above
// the chip with its label, the skill's description or "No description is available for this skill.",
// and, for a skill with a path, "View instructions" (ChatComposer's openMention: rightPanelStore.openFile).
// The native editors catch the press on their drawn chips and report it with the chip's window frame
// (T3ComposerChipPress.swift, `editorChip`); this builds the popover the window draws there
// (composer-chip-popover.contract). The composer's chips name the selected provider's skills for the
// thread's workspace (ChatComposer selectedProviderSkills); the Settings prompt sample passes none
// (SettingsFontPreviews EMPTY_SKILLS), so its chip always has no description.
// A terminal excerpt's chip is the same kind of trigger (TerminalContextInlineChip.tsx's
// ContextChipPopover, while the excerpt holds text): its details are the terminal's name, "Line N" or "Lines a–b", and the
// captured output, from the draft's terminal context record. A review comment's and a preview annotation's chips open
// their details the same way, and the other chips' presses are actions (composer-chip-details.ts).
import type { T3Client } from '../client/client';
import { obj, str, type Obj } from '../client/domain';
import { ClientError, type Native } from '../client/protocol';
import { workspaceValues } from './composer-workspace-snapshots';
import { selectedProvider, workspaceCwd } from './composer-editor';
import { openFileSurface } from '../panel/panel-surfaces';
import { clearCitationDraft, noteCitationDraft, registerCitationDraft } from '../timeline/citation-draft';
import { citationCommentLocal, citationPopover, citedStart } from '../timeline/citation-comment';
import { chipAct, composerImageStep, performChipAct, contextDetails, draftContextRecords } from './composer-chip-details';
import { composerFileLocal } from './composer-chip-file';
import { composerChipCurrent, composerChipFinishNoEdit, composerChipFinishView } from './composer-chip-bridge';
import { composerChipStore, NO_CHIP_FINISH, type ComposerChipFinish } from './composer-chip-store';

export type ChipPopover = {
  finish: ComposerChipFinish;
  /** The press's sequence (newest across the composer and the prompt sample); 0 before any. */
  seq: number;
  open: boolean;
  /** "composer" or "preview". */
  surface: string;
  label: string;
  /** PopoverTitle (sr-only) and the trigger's name: `Skill <label>`. */
  title: string;
  description: string;
  /** The skill's instructions file, when it has one (View instructions). */
  path: string;
  /** The chip's frame in the window (x, top, width, height), in points. */
  x: number;
  y: number;
  width: number;
  height: number;
  /** "skill", or "citation" (its comment, citation-comment.ts). */
  kind: string;
  /** A citation's saved comment. */
  comment: string;
  /** A citation's anchor: "chip" (its comment button) or "source" (the cited text, after Cite), and its frame. */
  anchor: string;
  ax: number;
  ay: number;
  aw: number;
  ah: number;
  /** The cited answer (its message id without "message:" / "turn-item:"), its quote and the quote's start in the answer's text stream when this app cited it (-1). */
  message: string;
  quote: string;
  at: number;
  /** A terminal excerpt's lines ("Line 3", "Lines 1–738") and its output's line count (`description` holds the output); a
   *  review comment's "<section> · <range>" and its diff's line count. */
  lines: string;
  rows: number;
  /** A review comment's diff (composer-chip-details.ts). */
  diff: string;
  /** A preview annotation's crop (a draft image id) and its height over its width (0: unknown). */
  image: string;
  aspect: number;
  /** An action press the window has not yet sent (closed, `open` false): "mention", "thread", "citation", "image"… */
  act: string;
};
export const NO_CHIP: ChipPopover = { finish: NO_CHIP_FINISH, seq: 0, open: false, surface: '', label: '', title: '', description: '', path: '', x: 0, y: 0, width: 0, height: 0,
  kind: '', comment: '', anchor: '', ax: 0, ay: 0, aw: 0, ah: 0, message: '', quote: '', at: -1, lines: '', rows: 0, diff: '', image: '', aspect: 0, act: '' };
export const NO_DESCRIPTION = 'No description is available for this skill.';

/** The popover for a press the native editor reported; `skills` is what the chip's editor was given, `records` the draft's context records by id. */
export function chipPopover(press: Obj, skills: Obj[], startOf?: Parameters<typeof citationPopover>[2], records: Obj = {}): ChipPopover {
  const seq = Number(press.seq) || 0;
  if (press.open !== true && str(press.act)) return { ...NO_CHIP, seq, surface: str(press.surface), act: str(press.act) };
  if (press.open === true && str(press.kind) === 'citation') return citationPopover(press, NO_CHIP, startOf);
  if (press.open === true && str(press.kind) === 'terminal') return terminalPopover(press, obj(records[str(press.context)]));
  if (press.open === true && ['review-comment', 'preview-annotation'].includes(str(press.kind))) {
    const details = contextDetails(str(press.kind), obj(records[str(press.context)]), str(press.label) || str(press.name));
    if (!details) return { ...NO_CHIP, seq };
    const frame = Array.isArray(press.frame) ? (press.frame as unknown[]).map(Number) : [];
    return { ...NO_CHIP, ...details, seq, open: true, surface: str(press.surface), x: frame[0] || 0, y: frame[1] || 0, width: frame[2] || 0, height: frame[3] || 0 };
  }
  if (press.open !== true || str(press.kind) !== 'skill') return { ...NO_CHIP, seq };
  const frame = Array.isArray(press.frame) ? (press.frame as unknown[]).map(Number) : [];
  const name = str(press.name), label = str(press.label) || name;
  // ComposerSkillNodeView (T3 Code 74541bab17): the provider's user-invocable skill by that name, its
  // description or else its short description, trimmed; the label is the provider's current display name.
  const skill = skills.find(candidate => str(candidate.name) === name && candidate.enabled !== false && candidate.userInvocable !== false);
  const description = (skill && (str(skill.description).trim() || str(skill.shortDescription).trim())) || NO_DESCRIPTION;
  return { ...NO_CHIP, seq, open: true, surface: str(press.surface), kind: 'skill', label, title: `Skill ${label}`, description, path: skill ? str(skill.path) : '',
    x: frame[0] || 0, y: frame[1] || 0, width: frame[2] || 0, height: frame[3] || 0 };
}

/**
 * TerminalContextInlineChip's details: the record's terminal name and lines, its captured output (the editor opens it only
 * while the excerpt holds text); the trigger's name and the sr-only title are "Terminal excerpt, <chip label>".
 */
export function terminalPopover(press: Obj, record: Obj): ChipPopover {
  const seq = Number(press.seq) || 0, text = str(record.text);
  if (record.kind !== 'terminal' || !text) return { ...NO_CHIP, seq };
  const frame = Array.isArray(press.frame) ? (press.frame as unknown[]).map(Number) : [];
  const start = Number(record.lineStart) || 0, end = Number(record.lineEnd) || start;
  return { ...NO_CHIP, seq, open: true, surface: str(press.surface), kind: 'terminal', label: str(record.terminalLabel), title: `Terminal excerpt, ${str(press.label) || str(press.name)}`,
    description: text, lines: start === end ? `Line ${start}` : `Lines ${start}–${end}`, rows: text.split('\n').length,
    x: frame[0] || 0, y: frame[1] || 0, width: frame[2] || 0, height: frame[3] || 0 };
}

/** The root's measured controls and shared source own the current press. */
export function chipPopoverView(client: T3Client): ChipPopover {
  const press = composerChipCurrent(client);
  if (!press) return { ...NO_CHIP, seq: composerChipStore(client).presses.latest, finish: composerChipFinishView(client) ?? NO_CHIP_FINISH };
  // The composer's press is the thread's on screen; the sample's skills are none.
  const composer = str(press.surface) === 'composer';
  registerCitationDraft(client, press);
  return chipPopover(press, composer ? workspaceValues(selectedProvider(client), workspaceCwd(client), 'skills') : [], citation => citedStart(client, citation),
    composer ? draftContextRecords(client) : {});
}

/**
 * `editorlocal:chip-close` (a press outside; Escape on the window, value `escape`) and `editorlocal:chip-instructions` (View instructions);
 * a citation's comment's `chip-cite-…` (citation-comment.ts).
 */
export async function chipPopoverLocal(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  if (op === 'chip-cite-draft') {
    noteCitationDraft(client, Number(id) || 0, value);
    return '';
  }
  if (op.startsWith('chip-cite-') && ['save', 'send', 'commit', 'cancel', 'escape'].includes(op.slice(10))) {
    clearCitationDraft(client, Number(id) || 0);
    return citationCommentLocal(client, native, op, id, value);
  }
  if (op === 'chip-act') return chipAct(client, native, Number(id) || 0); // a mention, thread, citation, pull request or attachment press (composer-chip-details.ts)
  if (op === 'chip-image-open') return performChipAct(client, native, { act: 'image', context: `image_${id}` }); // a shelf thumbnail's Preview button
  if (op.startsWith('chip-image-')) return composerImageStep(client, op); // the expanded image preview's close and steps
  if (op === 'chip-file') return composerFileLocal(client, native, id, value); // a file chip's preview (composer-chip-file.ts)
  if (op !== 'chip-close' && op !== 'chip-instructions') throw new ClientError(`Unknown editor action: ${op}`);
  // Escape gives the prompt the focus back with the chip selected (PopoverPopup's finalFocus: the chip, its trigger).
  const press = composerChipCurrent(client);
  if (!composerChipFinishNoEdit(client, Number(id) || 0, { send: false, focus: op === 'chip-close' && value === 'escape' ? 'select' : op === 'chip-instructions' ? 'none' : 'outside' })) return '';
  if (op === 'chip-instructions' && value && press?.kind === 'skill' && press.seq === Number(id)) {
    const skill = workspaceValues(selectedProvider(client), workspaceCwd(client), 'skills').find(candidate => str(candidate.name) === press.name);
    if (str(skill?.path) === value) await openFileSurface(client, native, value, 0);
  }
  return '';
}
