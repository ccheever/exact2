import type { T3Client } from '../client/client';
import { obj } from '../client/domain';
import { composerInput } from './requests';
import { composerRichProjection, type ComposerRichProjection } from './composer-rich-projection';
import { composerSourceSession } from './composer-rich-state';
import { composerHistoryAvailability } from './composer-prompt-history';
import { CODE_BLOCK_LANGUAGES } from './composer-code-languages';
import { composerChipFaces, type ComposerChipFace } from './composer-chip-presentation';
import { offeredSkillNames } from './composer-editor-menu';
import { selectedProvider, workspaceCwd } from './composer-editor';
import { workspaceValues } from './composer-workspace-snapshots';

/** SettingsFontPreviews PREVIEW_SKILLS: the prompt sample's one skill. */
export const PREVIEW_SKILLS = ['frontend-design'];

type ComposerSourceChipView = {
  kind: string; start: number; end: number; source: string; label: string;
  name: string; path: string; href: string; contextKind: string; contextId: string;
  image: boolean; comment: string; quote: string;
  face: ComposerChipFace;
};
export type ComposerRichView = Omit<ComposerRichProjection, 'chips'> & {
  generation: number; historyPrevious: boolean; historyNext: boolean; chips: ComposerSourceChipView[];
  languages: ReadonlyArray<{ readonly id: string; readonly label: string }>;
};

/** The root passes its live field value, which can precede draft persistence.
 *  Both signatures travel back with the ranges for action-time owner checks. */
export function composerRichSource(client: T3Client, input: {
  snapshotOwner: string; owner: string; value: string; inputType?: string; surface?: 'composer' | 'preview';
}): ComposerRichView {
  const current = input.snapshotOwner === client.snapshotOwner, preview = input.surface === 'preview';
  const styling = current && !preview && obj(client.local.clientSettings).composerRichTextEnabled !== false;
  const literal = !current || !preview && composerInput(client)?.active?.initialAnswer !== undefined;
  const session = composerSourceSession(client, preview ? 'preview' : 'composer');
  // Only the skills the provider offers users become chips (composerSkillMeta).
  const skills = preview ? PREVIEW_SKILLS : offeredSkillNames(workspaceValues(selectedProvider(client), workspaceCwd(client), 'skills'));
  if (current) session.observe(input.owner, input.value, input.inputType, literal ? 'literal' : styling ? 'rich' : 'plain', skills);
  const chips = current ? session.chips : [];
  const projection = composerRichProjection({
    owner: current ? input.owner : '', value: input.value,
    styling, literal,
    blockOrigins: current ? session.blockOrigins : [],
    chips,
  });
  const faces = composerChipFaces(client, input.value, projection.chips, preview);
  return { ...projection, generation: client.generation, languages: CODE_BLOCK_LANGUAGES, ...(!current || preview || composerInput(client) ? { historyPrevious: false, historyNext: false } : composerHistoryAvailability(client, input.owner, input.value)), chips: projection.chips.map((chip, index) => ({
    kind: chip.kind, start: chip.start, end: chip.end, source: chip.source, label: chip.label,
    name: chip.kind === 'skill' ? chip.name : '', path: chip.kind === 'mention' ? chip.path : '',
    contextKind: chip.kind === 'context' ? chip.contextKind : '', contextId: chip.kind === 'context' ? chip.contextId : '',
    image: chip.kind === 'context' && chip.image, href: chip.kind === 'citation' ? chip.href : '',
    comment: chip.kind === 'citation' ? chip.comment ?? '' : '', quote: chip.kind === 'citation' ? chip.quote : '',
    face: faces[index]!,
  })) };
}
