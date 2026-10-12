// A skill chip's details popover (settings-appearance-and-skill-chip, desktop audit S1-12), adapted
// from T3 Code 1e2ecbd975 (MIT, LICENSE-T3): ComposerPromptEditorTiptap.tsx ComposerSkillNodeView
// wraps each skill chip in a ContextChipPopover (contextChipParts.tsx): a press opens a popover above
// the chip with its label, the skill's description or "No description is available for this skill.",
// and, for a skill with a path, "View instructions" (ChatComposer's openMention: rightPanelStore.openFile).
// The native editors catch the press on their drawn chips and report it with the chip's window frame
// (T3ComposerChipPress.swift, `editorChip`); this builds the popover the window draws there
// (composer-chip-popover.contract). The composer's chips name the selected provider's skills for the
// thread's workspace (ChatComposer selectedProviderSkills); the Settings prompt sample passes none
// (SettingsFontPreviews EMPTY_SKILLS), so its chip always has no description.
import type { T3Client } from './client';
import { obj, str, type Obj } from './domain';
import { bridgeReply, ClientError, type Native } from './protocol';
import { workspaceValues } from './composer-workspace-snapshots';
import { selectedProvider, workspaceCwd } from './composer-editor';
import { openFileSurface } from './r4-surfaces-panel';
import { citationCommentLocal, citationPopover, citedStart } from './citation-comment';

export type ChipPopover = {
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
  /** "skill", or "citation" (diff-cite-popover DC-1: its comment, citation-comment.ts). */
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
};
export const NO_CHIP: ChipPopover = { seq: 0, open: false, surface: '', label: '', title: '', description: '', path: '', x: 0, y: 0, width: 0, height: 0,
  kind: '', comment: '', anchor: '', ax: 0, ay: 0, aw: 0, ah: 0, message: '', quote: '', at: -1 };
export const NO_DESCRIPTION = 'No description is available for this skill.';

/** The popover for a press the native editor reported; `skills` is what the chip's editor was given. */
export function chipPopover(press: Obj, skills: Obj[], startOf?: Parameters<typeof citationPopover>[2]): ChipPopover {
  const seq = Number(press.seq) || 0;
  if (press.open === true && str(press.kind) === 'citation') return citationPopover(press, NO_CHIP, startOf);
  if (press.open !== true || str(press.kind) !== 'skill') return { ...NO_CHIP, seq };
  const frame = Array.isArray(press.frame) ? (press.frame as unknown[]).map(Number) : [];
  const name = str(press.name), label = str(press.label) || name;
  // ComposerSkillNodeView: `skill?.description ?? skillDescription ?? "No description…"`.
  const skill = skills.find(candidate => str(candidate.name) === name);
  const description = skill && typeof skill.description === 'string' ? skill.description : NO_DESCRIPTION;
  return { ...NO_CHIP, seq, open: true, surface: str(press.surface), kind: 'skill', label, title: `Skill ${label}`, description, path: skill ? str(skill.path) : '',
    x: frame[0] || 0, y: frame[1] || 0, width: frame[2] || 0, height: frame[3] || 0 };
}

/** The window's `chip` resource: re-asked when a press opens, moves or closes (`t3.chip`). */
export async function chipPopoverView(client: T3Client, native: Native | null | undefined): Promise<ChipPopover> {
  if (!native?.available) return NO_CHIP;
  native.watch('t3.chip');
  let press: Obj;
  try {
    const reply = await bridgeReply(native, { op: 'editorChip' });
    if (!reply.ok) return NO_CHIP;
    press = obj(reply.value);
  } catch { return NO_CHIP; }
  // The composer's press is the thread's on screen; the sample's skills are none.
  const composer = str(press.surface) === 'composer';
  if (composer && str(press.owner) && str(press.owner) !== client.snapshotOwner) return { ...NO_CHIP, seq: Number(press.seq) || 0 };
  return chipPopover(press, composer ? workspaceValues(selectedProvider(client), workspaceCwd(client), 'skills') : [], citation => citedStart(client, citation));
}

/**
 * `editorlocal:chip-close` (a press outside, Escape on the window) and `editorlocal:chip-instructions` (View instructions);
 * a citation's comment's `chip-cite-…` (citation-comment.ts).
 */
export async function chipPopoverLocal(client: T3Client, native: Native, op: string, id: string, value: string): Promise<string> {
  if (op.startsWith('chip-cite-') && ['save', 'send', 'commit', 'cancel', 'escape'].includes(op.slice(10))) return citationCommentLocal(native, op, id, value);
  if (op !== 'chip-close' && op !== 'chip-instructions') throw new ClientError(`Unknown editor action: ${op}`);
  await bridgeReply(native, { op: 'editorChipClose', seq: Number(id) || 0 }).catch(() => undefined);
  if (op === 'chip-instructions' && value) await openFileSurface(client, native, value, 0);
  return '';
}

/**
 * A press on a diff's or the Files preview's gutter (files-gutter-parity FG-4): Base UI's outside press closes the details
 * there too, but the window's root never hears it, as a node that hears `pointerdown` keeps the press from its ancestors
 * (X71). The gutters report their primary presses (diff-review.ts, pages-pr-code.ts, diff-file-comments.ts), and the
 * newest open press closes as the root's `chip-close` closes it; the window's `chip` resource follows the module's `t3.chip`.
 */
export async function closeChipFromPress(native: Native): Promise<void> {
  try {
    const reply = await bridgeReply(native, { op: 'editorChip' });
    const press = reply.ok ? obj(reply.value) : {};
    if (press.open === true) await bridgeReply(native, { op: 'editorChipClose', seq: Number(press.seq) || 0 });
  } catch { /* no editor module: nothing is open */ }
}
