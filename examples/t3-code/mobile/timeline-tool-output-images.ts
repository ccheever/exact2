// Tool output image blocks from T3 Code's shared toolOutput.ts (MIT, see LICENSE-T3).
import { obj, str, type Obj } from './shared/domain';

export interface ToolOutputImage { mimeType: string; data?: string }
const IMAGE_TYPES = new Set(['image/gif', 'image/jpeg', 'image/png', 'image/webp']);
export const MAX_TOOL_OUTPUT_IMAGES = 8;

export function readToolOutputImage(value: unknown): ToolOutputImage | null {
  const block = obj(value);
  if (block.type !== 'image') return null;
  const source = block.source !== null && typeof block.source === 'object' && !Array.isArray(block.source) ? block.source : undefined;
  if (source !== undefined && source.type !== 'base64') return null;
  const mimeType = source === undefined ? block.mimeType : source.media_type;
  const data = source === undefined ? block.data : source.data;
  if (typeof mimeType !== 'string' || !IMAGE_TYPES.has(mimeType.toLowerCase())) return null;
  return typeof data === 'string' ? { mimeType: mimeType.toLowerCase(), data } : { mimeType: mimeType.toLowerCase() };
}

/** The first eight image blocks a dynamic tool output can preview. */
export function toolOutputImageBlocks(value: unknown): unknown[] {
  const blocks = Array.isArray(value) ? value : Array.isArray(obj(value).content) ? obj(value).content : [value];
  const images: unknown[] = [];
  for (const block of Array.isArray(blocks) ? blocks : []) {
    if (!readToolOutputImage(block)) continue;
    images.push(block);
    if (images.length === MAX_TOOL_OUTPUT_IMAGES) break;
  }
  return images;
}

export function toolOutputImages(value: unknown): ToolOutputImage[] {
  return toolOutputImageBlocks(value).flatMap(block => readToolOutputImage(block) ?? []);
}

export function turnItemOutputImages(item: Obj): Obj[] {
  return item.type === 'dynamic_tool' && item.outputOmitted !== true ? toolOutputImages(item.output).map((_, index) => ({
    _tag: 'tool-output-image', threadId: str(item.threadId), itemId: str(item.id), index,
  })) : [];
}
