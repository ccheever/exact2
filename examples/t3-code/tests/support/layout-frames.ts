// Hands a client the layout frames app.contract would measure (src/shell/layout-frames.ts), by name.
import { noteLayoutFrames } from '../../src/shell/layout-frames';

export function setLayoutFrames(client: object, frames: Record<string, unknown>): void {
  noteLayoutFrames(client, Object.entries(frames).map(([name, box]) => `${name}:${(box as number[]).join(',')}`).join(';'));
}
