// The change request links in the Markdown on screen, for a web link's menu: its "Link to thread" or "Unlink from
// thread" row depends on the link (external-link-menu.ts chatLinkMenuView), and the row has to be known before the
// right-click reaches the link's context popover. Every Markdown text a chip list is made for notes its links here
// (message-chips.ts messageChips); the most recent ones are kept.
const noted = new Set<string>();
const NOTED_MAX = 256;

/** Web links in Markdown source (inline `](url)`, `<url>`, bare URLs) that name a change request. */
export function pullRequestUrls(markdown: string): string[] {
  const found = [...markdown.matchAll(/\]\((https?:\/\/[^\s)]+)\)|<(https?:\/\/[^\s>]+)>|(https?:\/\/[^\s<>()[\]]+)/g)].map(match => match[1] ?? match[2] ?? match[3] ?? '');
  return found.map(url => url.replace(/[.,;:!?]+$/, '')).filter(url => /\/(?:pulls?|-\/merge_requests|pull-requests|pullrequest)\/\d+/.test(url));
}

/** `url` as written and as `new URL` spells it (a link node's href can be either: the source's text, or the parsed
 *  destination with its host lowercased and its path percent-encoded), so the window's exact match finds both. */
export function hrefSpellings(url: string): string[] {
  try { const parsed = new URL(url).href; return parsed === url ? [url] : [url, parsed]; } catch { return [url]; }
}

export function notePullRequestLinks(markdown: string): void {
  if (!markdown.includes('://')) return;
  for (const url of pullRequestUrls(markdown).flatMap(hrefSpellings)) {
    noted.delete(url);
    noted.add(url);
    if (noted.size > NOTED_MAX) noted.delete(noted.values().next().value!);
  }
}
export const notedPullRequestLinks = (): string[] => [...noted];
export function resetPullRequestLinksForTests(): void { noted.clear(); }
