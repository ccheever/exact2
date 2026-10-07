// GAP 001: bake cannot capture parent imports. Remove this copy when ancestor mounts work.
// Unchanged body from examples/t3-code/client-shared.ts at 887b2491b182f851b11253655f6aa84fe2a26708.
// Helpers client.ts and the client-ops-*.ts files share, kept apart so the
// area files import no value from client.ts.
import { str } from './domain';

export const groupingModes = ['repository', 'repository_path', 'separate'];
export const projectPath = (value: unknown) => str(value).trim().replace(/\\/g, '/').replace(/\/+$/, '');
export const message = (error: unknown) => error instanceof Error ? error.message : 'The operation failed.';
