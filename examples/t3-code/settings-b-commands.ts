// Lane settings-b's commands, reached as `sb:<op>` from T3Client.command:
// project actions (settings-b-actions.ts) and project icons (settings-b-icons.ts).
import { ClientError, type Native, type Files } from './protocol';
import { runActionOp, ACTION_OPS } from './settings-b-actions';
import { runIconOp, ICON_OPS } from './settings-b-icons';
import type { T3Client } from './client';

export async function settingsBCommand(client: T3Client, native: Native, op: string, id: string, value: string, storage?: Files): Promise<string> {
  if (ACTION_OPS.includes(op)) return runActionOp(client, native, op, id, value);
  if (ICON_OPS.includes(op)) return runIconOp(client, native, storage, op, id, value);
  throw new ClientError(`Unknown action: ${op}`);
}
