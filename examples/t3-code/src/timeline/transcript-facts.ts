// The transcript's viewport as the window reads it from its own scroll, wheel and keys (app.contract restScrolled and
// restGesture): which thread's transcript it is (`${origin}:${projectId}:${threadId}`), whether the composer rests,
// whether the transcript is at its end, and how many pulls toward the end it has had there. The snapshot's arguments
// carry them; the composer's reservation (composer-overlay.ts) and its resting row (presentation.ts) read them.
export type TranscriptFacts = { owner: string; resting: boolean; atEnd: boolean; pulls: number };

const facts = new WeakMap<object, TranscriptFacts>();

/** The snapshot's arguments 4 to 7. */
export function noteTranscriptFacts(client: object, args: unknown[]): void {
  facts.set(client, { owner: String(args[4] ?? ''), resting: args[5] === true, atEnd: args[6] !== false, pulls: Number(args[7]) || 0 });
}

export function transcriptFacts(client: object): TranscriptFacts {
  return facts.get(client) ?? { owner: '', resting: false, atEnd: true, pulls: 0 };
}
