/** An image, by the URL or asset an `image` node's `src` takes (LLP 1056 D9). */
export type ImageHandle = string;
/** The 2D context a surface draws with (LLP 1056 §3, stages 1 and 2). */
export type Ctx2D = Pick<OffscreenCanvasRenderingContext2D,
  | 'save' | 'restore' | 'reset'
  | 'translate' | 'rotate' | 'scale' | 'transform' | 'setTransform' | 'resetTransform' | 'getTransform'
  | 'beginPath' | 'moveTo' | 'lineTo' | 'quadraticCurveTo' | 'bezierCurveTo' | 'arc' | 'arcTo' | 'ellipse' | 'rect' | 'roundRect' | 'closePath'
  | 'fill' | 'stroke' | 'clip' | 'fillRect' | 'strokeRect' | 'clearRect'
  | 'lineWidth' | 'lineCap' | 'lineJoin' | 'miterLimit' | 'setLineDash' | 'getLineDash' | 'lineDashOffset'
  | 'fillStyle' | 'strokeStyle' | 'createLinearGradient' | 'createRadialGradient' | 'createConicGradient'
  | 'globalAlpha' | 'globalCompositeOperation'
  | 'shadowColor' | 'shadowBlur' | 'shadowOffsetX' | 'shadowOffsetY'
  | 'imageSmoothingEnabled' | 'imageSmoothingQuality'
  | 'font' | 'textAlign' | 'textBaseline' | 'direction' | 'letterSpacing' | 'wordSpacing'
  | 'fontKerning' | 'fontStretch' | 'fontVariantCaps' | 'textRendering'
  | 'fillText' | 'strokeText' | 'measureText'
  | 'createImageData' | 'putImageData'> & {
  drawImage(image: ImageHandle, dx: number, dy: number): void;
  drawImage(image: ImageHandle, dx: number, dy: number, dw: number, dh: number): void;
  drawImage(image: ImageHandle, sx: number, sy: number, sw: number, sh: number, dx: number, dy: number, dw: number, dh: number): void;
  createPattern(image: ImageHandle, repetition: string | null): CanvasPattern | null;
};
/** What one draw is told (LLP 1056 D4–D6). */
export interface Frame {
  readonly time: number;
  readonly mounted: number;
  readonly cause: 'mount' | 'args' | 'size' | 'frame' | 'image' | 'font';
  readonly causes: readonly string[];
  readonly width: number;
  readonly height: number;
  readonly pixelWidth: number;
  readonly pixelHeight: number;
  readonly scale: number;
}
/** A module's `draw`: true asks for another frame. */
export type Draw = (surface: string, args: any, ctx: Ctx2D, frame: Frame) => boolean;
