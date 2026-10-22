import type { T3Client } from '../client/client';
import type { Native } from '../client/protocol';

// Keep the hook outside the sidebar/drawer runtime graph so eager registration follows its initialization.
export let closeThreadTerminals: (client: T3Client, native: Native, threadId: string) => Promise<void> = async () => undefined;
export function setCloseThreadTerminals(hook: typeof closeThreadTerminals): void { closeThreadTerminals = hook; }
