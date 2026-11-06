// Copied from T3 Code 1e2ecbd975 (MIT reference, see LICENSE-T3: apps/web/src/lib/terminalCloseConfirm.ts).
// Changes from the reference:
// - `readLocalApi()` is a seam the clone sets (`setTerminalCloseLocalApi`); the clone's drawer asks
//   through its own confirm dialog (app.contract titleMenuPick "ui:confirm" → AppConfirm), whose
//   title and body are the two lines of `terminalCloseConfirmMessage` (terminal-drawer-view.ts).
// - The message text is split out as `terminalCloseConfirmMessage` so the dialog and this function
//   share it.
type LocalApi = { dialogs: { confirm: (message: string, options?: unknown) => Promise<boolean> } };
let readLocalApi: () => LocalApi | undefined = () => undefined;
/** The clone's seam for `~/localApi`'s `readLocalApi`. */
export function setTerminalCloseLocalApi(read: () => LocalApi | undefined): void {
  readLocalApi = read;
}

let pendingConfirmations = 0;

/** Whether a terminal-close confirmation is currently waiting on the user. */
export function isTerminalCloseConfirmPending(): boolean {
  return pendingConfirmations > 0;
}

/** The confirmation's two lines: the question and what closing does. */
export function terminalCloseConfirmMessage(labels: readonly [string, ...string[]]): [string, string] {
  return labels.length === 1
    ? [`Close terminal "${labels[0]}"?`, "This stops the running process and clears its history."]
    : [
        `Close ${labels.length} terminals?`,
        `This stops their running processes and clears their histories: ${labels
          .map((label) => `"${label}"`)
          .join(", ")}.`,
      ];
}

/**
 * Confirmation for individual terminal close actions: drawer buttons, panel
 * buttons, the `terminal.close` keybinding, and closing a terminal surface from
 * the tab strip. Auto-exit cleanup and bulk tab closes skip this path and close
 * directly.
 */
export async function confirmTerminalClose(
  labels: readonly [string, ...string[]],
): Promise<boolean> {
  const localApi = readLocalApi();
  if (!localApi) return true;
  pendingConfirmations += 1;
  try {
    return await localApi.dialogs.confirm(terminalCloseConfirmMessage(labels).join("\n"), {
      variant: "destructive",
    });
  } catch {
    return false;
  } finally {
    pendingConfirmations -= 1;
  }
}
