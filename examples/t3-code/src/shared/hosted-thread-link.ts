// shared/threadLinks.ts hostedThreadLink and connectAuth.ts DEFAULT_HOSTED_APP_URL at T3 Code v0.1.7 (MIT, see
// LICENSE-T3). A leaf module: the sidebar's Copy thread writes it without loading the timeline or the composer.

/** The hosted app. Its thread route offers the desktop app, the mobile app, or opens the thread itself. */
export const HOSTED_APP_URL = 'https://app.t3.codes';

// Escapes even URL-safe punctuation, so a sentence's closing period never reads as part of an id.
const hostedSegment = (id: string) => encodeURIComponent(id).replace(/[.!~*'()]/g, character => `%${character.charCodeAt(0).toString(16).toUpperCase()}`);

/** hostedThreadLink: a link that opens the thread from anywhere. */
export function hostedThreadLink(ref: { environmentId: string; threadId: string }, origin = HOSTED_APP_URL): string {
  return new URL(`/threads/${hostedSegment(ref.environmentId)}/${hostedSegment(ref.threadId)}`, origin).toString();
}
