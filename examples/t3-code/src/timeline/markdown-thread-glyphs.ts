// ThreadContextChip's project favicon, T3 Code v0.1.5 (MIT, see LICENSE-T3): a thread chip, a thread link's
// or a sent thread context's, leads with its thread's project icon; with no known project it keeps MessagesSquare.
import type { T3Client } from '../client/client';
import { str } from '../client/domain';
import { projectIdentity } from '../client/presentation';
import { projectGlyph, type ProjectGlyph } from '../sidebar/sidebar-glyph';
import type { ChipView } from './message-chips';
import { parseThreadLinkHref, resolveThreadLink, threadLinksIn } from './thread-links';

export interface ThreadLinkGlyph { href: string; title: string; glyph: ProjectGlyph }
export function markdownThreadGlyphs(client: T3Client, text: string, chips: ChipView[] = []): ThreadLinkGlyph[] {
  const found = new Map<string, ThreadLinkGlyph>();
  // Another environment's project draws with that server's favicon (sidebar-glyph.ts keys them by server).
  const add = (href: string, project: Parameters<typeof projectGlyph>[1], environmentId = '') => {
    if (!project || found.has(href)) return;
    found.set(href, { href, title: str(project.title), glyph: projectGlyph(client, project, projectIdentity, environmentId) });
  };
  for (const { href } of threadLinksIn(text)) {
    const link = parseThreadLinkHref(href);
    const target = link ? resolveThreadLink(client, link) : null;
    if (target) add(href, target.project, target.environmentId);
  }
  for (const chip of chips) {
    if (chip.kind !== 'thread' || !chip.target || chip.detail === 'link') continue;
    const thread = client.shell.threads.find(candidate => candidate.id === chip.target);
    add(chip.href, thread ? client.shell.projects.find(project => project.id === thread.projectId) : undefined);
  }
  return [...found.values()];
}
