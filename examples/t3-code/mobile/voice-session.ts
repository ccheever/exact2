// Pinned T3 Code365aa87982 apps/mobile/src/features/voice-input/voiceInputSession.ts; imports adapted only.
// Original SHA256 c5f0689000690c60a68f032b66e8125ca705412ce7b2ad1b5358371e14ef9503. MIT: voice-LICENSE.
// @ref llp/1109.008-mobile-voice.decision.md#controller-reuse
import {
  VoiceInputController,
  voiceInputBlocksSubmission,
  type VoiceDraftSnapshot,
  type VoiceInputControllerDependencies,
} from "./voice-controller";

export type VoiceInputTarget = {
  readonly ownerKey: string;
  /** Names the draft being dictated into while its composer is off screen. */
  readonly label?: string;
  readonly readDraft: () => VoiceDraftSnapshot | null;
  readonly commitDraft: VoiceInputControllerDependencies["commitDraft"];
  readonly subscribe: () => () => void;
};

export function createVoiceInputTarget(
  ownerKey: string,
  readText: () => string | null,
  commitDraft: VoiceInputTarget["commitDraft"],
  selection: VoiceDraftSnapshot["selection"],
  subscribeToChanges: (onChange: () => void) => () => void,
): VoiceInputTarget {
  let revision = 0;
  return {
    ownerKey,
    readDraft: () => {
      const text = readText();
      if (text === null) return null;
      return { ownerKey, text, selection, revision };
    },
    commitDraft,
    subscribe: () => {
      let previousText = readText();
      return subscribeToChanges(() => {
        const text = readText();
        if (text !== previousText) {
          previousText = text;
          revision += 1;
        }
      });
    },
  };
}

export class VoiceInputSession {
  readonly controller: VoiceInputController;
  private target: VoiceInputTarget | null = null;
  private unsubscribe: (() => void) | null = null;

  constructor(dependencies: Omit<VoiceInputControllerDependencies, "readDraft" | "commitDraft">) {
    this.controller = new VoiceInputController({
      ...dependencies,
      onStateChange: (state) => {
        if (!voiceInputBlocksSubmission(state)) {
          this.unsubscribe?.();
          this.unsubscribe = null;
        }
        dependencies.onStateChange(state);
      },
      readDraft: () => this.target?.readDraft() ?? null,
      commitDraft: (text, selection) => this.target?.commitDraft(text, selection),
    });
  }

  get ownerKey(): string | null {
    return this.target?.ownerKey ?? null;
  }

  get label(): string | null {
    return this.target?.label ?? null;
  }

  cancel(ownerKey: string | null): void {
    if (ownerKey !== null && this.ownerKey === ownerKey) this.controller.cancel();
  }

  retry(): Promise<void> {
    return this.target ? this.start(this.target) : Promise.resolve();
  }

  start(target: VoiceInputTarget): Promise<void> {
    if (voiceInputBlocksSubmission(this.controller.currentState)) return Promise.resolve();
    this.target = target;
    this.unsubscribe = target.subscribe();
    return this.controller.start();
  }
}
