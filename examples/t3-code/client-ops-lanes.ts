// The prefixed client.command() ops (client-ops.ts): each prefix names the
// lane module that owns its ops (settings-rest-commands.ts, chat-commands.ts,
// pages-commands.ts, composer-editor.ts, shell-commands.ts,
// composer-controls-commands.ts, sidebar-commands.ts, settings-b-commands.ts,
// settings-appearance-editor.ts).
// A '…local:' op changes device state only; the others write to the server.
import type { T3Client } from './client';
import type { OpOut } from './client-ops';
import { restCommand, restLocal } from './settings-rest-commands';
import { chatCommand } from './chat-commands';
import { shellCommand, shellLocal } from './shell-commands';
import { composerCommand, composerLocal } from './composer-controls-commands';
import { chatLocal } from './timeline-presentation';
import { editorLocal } from './composer-editor';
import { sidebarCommand, sidebarLocal } from './sidebar-commands';
import { settingsBCommand } from './settings-b-commands';
import { pagesLocal } from './pages-commands';
import { themeLocal } from './settings-appearance-editor';
import { type Native, type Files } from './protocol';

/** Device-local ops by prefix, each handled by its lane's module. */
export async function laneOps(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    if (op.startsWith('restlocal:')) { resultMessage = await restLocal(this, native, storage, op.slice(10), id, value);
    } else if (op.startsWith('chatlocal:')) { resultMessage = await chatLocal(this, native, op.slice(10), id, value, storage);
    } else if (op.startsWith('pageslocal:')) { resultMessage = await pagesLocal(this, native, storage, op.slice(11), id, value);
    } else if (op.startsWith('editorlocal:')) { resultMessage = await editorLocal(this, native, op.slice(12), id, value, n);
    } else if (op.startsWith('shelllocal:')) { resultMessage = await shellLocal(this, native, op.slice(11), id, value);
    } else if (op.startsWith('cclocal:')) { resultMessage = await composerLocal(this, native, storage, op.slice(8), id, value);
    } else if (op.startsWith('sidebarlocal:')) { resultMessage = await sidebarLocal(this, native, op.slice(13), id, value);
    } else if (op.startsWith('themelocal:')) { resultMessage = themeLocal(this, op.slice(11), id, value, n); // the theme editor's colour picker
    } else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
/** Server writes by prefix, each handled by its lane's module. */
export async function laneWrites(this: T3Client, op: string, id: string, value: string, n: number, native: Native, storage: Files, out: OpOut): Promise<boolean> {
  let resultMessage = '';
  try {
    if (op.startsWith('rest:')) resultMessage = await restCommand(this, native, storage, op.slice(5), id, value);
    else if (op.startsWith('chat:')) resultMessage = await chatCommand(this, native, storage, op.slice(5), id, value, n);
    else if (op.startsWith('shell:')) resultMessage = await shellCommand(this, native, storage, op.slice(6), id, value);
    else if (op.startsWith('sidebar:')) resultMessage = await sidebarCommand(this, native, storage, op.slice(8), id, value, n);
    else if (op.startsWith('cc:')) resultMessage = await composerCommand(this, native, storage, op.slice(3), id, value);
    else if (op.startsWith('sb:')) resultMessage = await settingsBCommand(this, native, op.slice(3), id, value, storage); // settings-b-commands.ts
    else return false;
    return true;
  } finally { Object.assign(out, { message: resultMessage, id, value }); }
}
