// ThreadContextChip's project favicon, T3 Code 74541bab17 (MIT, see LICENSE-T3): a thread chip, a thread link's
// or a sent thread context's, leads with its thread's project icon; with no known project it keeps MessagesSquare.
import type { T3Client } from '../client/client';
import { str } from '../client/domain';
import { projectIdentity } from '../client/presentation';
import { projectGlyph, type ProjectGlyph } from '../sidebar/sidebar-glyph';
import { markdownLinks, type ChipView } from './message-chips';
import { parseThreadLinkHref, resolveThreadLink } from './thread-links';

export interface ThreadLinkGlyph { href: string; title: string; glyph: ProjectGlyph }
export function markdownThreadGlyphs(client: T3Client, text: string, chips: ChipView[] = []): ThreadLinkGlyph[] {
  const found = new Map<string, ThreadLinkGlyph>();
  const add = (href: string, project: Parameters<typeof projectGlyph>[1], remote: boolean) => {
    if (!project || found.has(href)) return;
    const glyph = projectGlyph(client, project, projectIdentity);
    // Another environment's favicon is that server's: its monogram stands in.
    found.set(href, { href, title: str(project.title), glyph: remote && glyph.kind === 'image' ? { ...glyph, kind: 'monogram', src: '' } : glyph });
  };
  for (const { href } of markdownLinks(text)) {
    const id = parseThreadLinkHref(href);
    const target = id ? resolveThreadLink(client, id) : null;
    if (target) add(href, target.project, target.remote);
  }
  for (const chip of chips) {
    if (chip.kind !== 'thread' || !chip.target || chip.detail === 'link') continue;
    const thread = client.shell.threads.find(candidate => candidate.id === chip.target);
    add(chip.href, thread ? client.shell.projects.find(project => project.id === thread.projectId) : undefined, false);
  }
  return [...found.values()];
}
