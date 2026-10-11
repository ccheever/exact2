import type { T3Client } from './client';

// The app-wide toast queue (reference: apps/web/src/components/ui/toast.tsx and
// its callers). Any module posts with pushToast while handling a command; the
// command's refresh carries the queue to the shell, which renders and expires it.
export type ToastKind = 'success' | 'error' | 'warning' | 'info' | 'loading';
export type ToastAction = { label: string; op: string; id?: string; value?: string };
export type Toast = { id: number; kind: ToastKind; title: string; description: string; action: ToastAction | null; timeoutMs: number;
  /** Optional presentation (ThreadToastData): a leading glyph instead of the kind icon, a second button, the stacked-end layout. */
  leading?: string; secondary?: ToastAction | null; actionVariant?: 'default' | 'outline'; stacked?: boolean; hideCopy?: boolean;
  /** ThreadToastData.secondaryActionVariant: `ghost` drops the outline button's border and fill. */
  secondaryVariant?: 'outline' | 'ghost';
  /** Called with the toast when it is dismissed by its × (ThreadToastData.onClose); with `closeOnAction`, by its buttons too. */
  onClose?: () => void; closeOnAction?: boolean; key?: string;
  /** ThreadToastData.expandableContent as rows, behind a description that discloses them. */
  details?: ToastDetail[]; expandLabels?: { expand: string; collapse: string };
  /** ThreadToastData.additionalActions (one): drawn before the second button, in its variant (browser-surface part 3). */
  extra?: ToastAction | null };
export type ToastDetail = { id: string; title: string; subtitle: string };

const queues = new WeakMap<T3Client, Toast[]>();
let nextId = 1;

export function pushToast(client: T3Client, toast: { kind: ToastKind; title: string; description?: string; action?: ToastAction; timeoutMs?: number;
  leading?: string; secondary?: ToastAction; actionVariant?: 'default' | 'outline'; stacked?: boolean; hideCopy?: boolean; onClose?: () => void; key?: string;
  details?: ToastDetail[]; expandLabels?: { expand: string; collapse: string }; secondaryVariant?: 'outline' | 'ghost'; closeOnAction?: boolean; extra?: ToastAction }): number {
  const queue = (queues.get(client) ?? []).filter(entry => !toast.key || entry.key !== toast.key);
  const id = nextId++;
  queue.push({ id, kind: toast.kind, title: toast.title, description: toast.description ?? '', action: toast.action ?? null,
    timeoutMs: toast.timeoutMs ?? (toast.kind === 'loading' ? 0 : 5000),
    ...(toast.leading ? { leading: toast.leading } : {}), ...(toast.secondary ? { secondary: toast.secondary } : {}),
    ...(toast.actionVariant ? { actionVariant: toast.actionVariant } : {}), ...(toast.stacked ? { stacked: true } : {}),
    ...(toast.hideCopy ? { hideCopy: true } : {}), ...(toast.onClose ? { onClose: toast.onClose } : {}), ...(toast.key ? { key: toast.key } : {}),
    ...(toast.details ? { details: toast.details } : {}), ...(toast.expandLabels ? { expandLabels: toast.expandLabels } : {}),
    ...(toast.secondaryVariant ? { secondaryVariant: toast.secondaryVariant } : {}), ...(toast.closeOnAction ? { closeOnAction: true } : {}), ...(toast.extra ? { extra: toast.extra } : {}) });
  queues.set(client, queue.slice(-5));
  return id;
}

/**
 * toastManager.update: change a live toast in place (same id, same place). A phase change
 * (ProjectCloneToastCoordinator) also patches the kind, timeout, second button and copy
 * button; a changed timeout restarts its timer (shell.ts advanceToasts).
 */
export function updateToast(client: T3Client, id: number, patch: Partial<Pick<Toast, 'title' | 'description' | 'details' | 'expandLabels' | 'action' | 'kind' | 'timeoutMs' | 'secondary' | 'hideCopy' | 'extra'>>): void {
  queues.set(client, (queues.get(client) ?? []).map(toast => toast.id === id ? { ...toast, ...patch } : toast));
}

export function dismissToast(client: T3Client, id: number): void {
  queues.set(client, (queues.get(client) ?? []).filter(toast => toast.id !== id));
}

export function toasts(client: T3Client): Toast[] {
  return queues.get(client) ?? [];
}
