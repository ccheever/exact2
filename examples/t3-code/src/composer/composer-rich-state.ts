import { ComposerSourceSession } from './composer-source-session';

const sessions = new WeakMap<object, { composer: ComposerSourceSession; preview: ComposerSourceSession }>();
export function composerSourceSession(owner: object, surface: 'composer' | 'preview' = 'composer'): ComposerSourceSession {
  let entry = sessions.get(owner);
  if (!entry) { entry = { composer: new ComposerSourceSession(), preview: new ComposerSourceSession() }; sessions.set(owner, entry); }
  return entry[surface];
}
