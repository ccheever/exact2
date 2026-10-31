// Helpers client.ts and the client-ops-*.ts files share, kept apart so the
// area files import no value from client.ts.
import { str } from './domain';

export const groupingModes = ['repository', 'repository_path', 'separate'];
export const projectPath = (value: unknown) => str(value).trim().replace(/\\/g, '/').replace(/\/+$/, '');
export const message = (error: unknown) => error instanceof Error ? error.message : 'The operation failed.';
