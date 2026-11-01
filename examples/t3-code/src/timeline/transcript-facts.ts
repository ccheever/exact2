// The transcript's viewport as the window reads it from its own scroll, wheel and keys (app.contract restScrolled and
// restGesture): which thread's transcript it is (`${origin}:${projectId}:${threadId}`), whether the transcript is at its
// end, and how many pulls toward the end it has had there. The snapshot's arguments carry them; the composer's
// reservation (composer-overlay.ts) and the transcript's away state (presentation.ts) read them.
export type TranscriptFacts = { owner: string; atEnd: boolean; pulls: number };

const facts = new WeakMap<object, TranscriptFacts>();

/** The snapshot's arguments 4 to 6. */
export function noteTranscriptFacts(client: object, args: unknown[]): void {
  facts.set(client, { owner: String(args[4] ?? ''), atEnd: args[5] !== false, pulls: Number(args[6]) || 0 });
}

export function transcriptFacts(client: object): TranscriptFacts {
  return facts.get(client) ?? { owner: '', atEnd: true, pulls: 0 };
}
