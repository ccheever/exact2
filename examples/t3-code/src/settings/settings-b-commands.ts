// The settings-b commands, reached as `sb:<op>` from T3Client.command:
// project actions (settings-b-actions.ts) and project icons (settings-b-icons.ts).
import { ClientError, type Native, type Files } from '../client/protocol';
import { runActionOp, ACTION_OPS } from './settings-b-actions';
import { runIconOp, ICON_OPS } from './settings-b-icons';
import { FAVICON_PICK_OP, pickProjectFavicon } from './desktop-shell-favicon'; // Open in Finder
import type { T3Client } from '../client/client';

export async function settingsBCommand(client: T3Client, native: Native, op: string, id: string, value: string, storage?: Files): Promise<string> {
  if (ACTION_OPS.includes(op)) return runActionOp(client, native, op, id, value);
  if (op === FAVICON_PICK_OP) return runIconOp(client, native, storage, 'project-favicon-set', id, await pickProjectFavicon(client, native, value));
  if (ICON_OPS.includes(op)) return runIconOp(client, native, storage, op, id, value);
  throw new ClientError(`Unknown action: ${op}`);
}
