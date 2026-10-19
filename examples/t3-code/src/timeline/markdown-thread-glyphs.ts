// MarkdownThreadLink's project favicon, T3 Code 9c161462c9 (MIT, see LICENSE-T3).
import type { T3Client } from '../client/client';
import { str } from '../client/domain';
import { projectIdentity } from '../client/presentation';
import { projectGlyph, type ProjectGlyph } from '../sidebar/sidebar-glyph';
import { markdownLinks } from './message-chips';
import { parseThreadLinkHref, percentDecodedThreadLinkId } from './thread-links';

export interface ThreadLinkGlyph { href: string; title: string; glyph: ProjectGlyph }
export function markdownThreadGlyphs(client: T3Client, text: string): ThreadLinkGlyph[] {
  const found = new Map<string, ThreadLinkGlyph>();
  for (const { href } of markdownLinks(text)) {
    const id = parseThreadLinkHref(href);
    if (!id || found.has(href)) continue;
    const thread = client.shell.threads.find(thread => thread.id === id) ?? client.shell.threads.find(thread => thread.id === percentDecodedThreadLinkId(id));
    const project = thread ? client.shell.projects.find(project => project.id === thread.projectId) : undefined;
    if (project) found.set(href, { href, title: str(project.title), glyph: projectGlyph(client, project, projectIdentity) });
  }
  return [...found.values()];
}
