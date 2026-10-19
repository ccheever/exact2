// T3 Code 9c161462c9 packages/shared/src/toolOutput.ts (MIT, LICENSE-T3).
import { obj, str, type Obj } from '../client/domain';
export interface ToolOutputImage { mimeType: string; data?: string }
const IMAGE_TYPES = new Set(['image/gif', 'image/jpeg', 'image/png', 'image/webp']);
export const MAX_TOOL_OUTPUT_IMAGES = 8;
export const MAX_TOOL_OUTPUT_IMAGE_BASE64_LENGTH = Math.ceil(10 * 1024 * 1024 / 3) * 4;
export function readToolOutputImage(value: unknown): ToolOutputImage | null {
  const block = obj(value);
  if (block.type !== 'image') return null;
  const source = block.source !== null && typeof block.source === 'object' && !Array.isArray(block.source) ? block.source : undefined;
  if (source !== undefined && source.type !== 'base64') return null;
  const mimeType = source === undefined ? block.mimeType : source.media_type, data = source === undefined ? block.data : source.data;
  if (typeof mimeType !== 'string' || !IMAGE_TYPES.has(mimeType.toLowerCase())) return null;
  return typeof data === 'string' ? { mimeType: mimeType.toLowerCase(), data } : { mimeType: mimeType.toLowerCase() };
}
export function toolOutputImages(value: unknown): ToolOutputImage[] {
  const blocks = Array.isArray(value) ? value : Array.isArray(obj(value).content) ? obj(value).content : [value];
  const images: ToolOutputImage[] = [];
  for (const block of Array.isArray(blocks) ? blocks : []) {
    const image = readToolOutputImage(block);
    if (!image) continue;
    images.push(image);
    if (images.length === MAX_TOOL_OUTPUT_IMAGES) break;
  }
  return images;
}
export function turnItemOutputImages(item: Obj): Obj[] {
  return item.type === 'dynamic_tool' && item.outputOmitted !== true ? toolOutputImages(item.output).map((_, index) => ({
    _tag: 'tool-output-image', threadId: str(item.threadId), itemId: str(item.id), index,
  })) : [];
}
