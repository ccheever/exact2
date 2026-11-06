// An archive's picture this page draws itself (MIT reference, see LICENSE-T3: apps/web/src/components/files/
// PreviewImageContent.tsx; T3 Code 7c3b3b3605ef): an SVG, which the app's image cannot draw, or one too large to send
// to the app. The size bar's zoom out and in, fit and actual size act on an `<img>` of the entry's Blob URL, as there.
// The picture opens no expanded dialog: that dialog is the app's, and the app cannot draw these pictures.
import { Maximize, Minus, Plus, Scan } from "../icons";
import { h, observeResize, surfaceAction, surfaceFailure, type IconNode, type SurfaceAction } from "../chrome";

const TYPES: Record<string, string> = {
  svg: "image/svg+xml", png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp",
  avif: "image/avif", bmp: "image/bmp", ico: "image/x-icon",
};

export class PictureView {
  readonly element = h("div", { class: "picture" });
  private readonly url: string;
  private natural = { width: 0, height: 0 };
  private viewport = { width: 0, height: 0 };
  /** null fits the picture to the viewport. */
  private zoom: number | null = null;
  private readonly size = h("span", { class: "tabular" }, "Loading image…");
  private readonly percent = h("span", { class: "picture-percent" });
  private readonly out: SurfaceAction;
  private readonly in: SurfaceAction;
  private readonly fit: SurfaceAction;
  private readonly actual: SurfaceAction;
  private readonly stage = h("div", { class: "picture-stage" });
  private readonly image: HTMLImageElement;
  private readonly stop: () => void;

  constructor(readonly path: string, bytes: Uint8Array) {
    const extension = path.slice(path.lastIndexOf(".") + 1).toLowerCase();
    this.url = URL.createObjectURL(new Blob([bytes as Uint8Array<ArrayBuffer>], { type: TYPES[extension] ?? "application/octet-stream" }));
    this.out = surfaceAction({ label: "Zoom out", icon: Minus as IconNode, onPress: () => this.change(1 / 1.25) });
    this.in = surfaceAction({ label: "Zoom in", icon: Plus as IconNode, onPress: () => this.change(1.25) });
    this.fit = surfaceAction({ label: "Fit image", icon: Scan as IconNode, pressed: true, onPress: () => { this.zoom = null; this.render(); } });
    this.actual = surfaceAction({ label: "Actual image size", icon: Maximize as IconNode, pressed: false, onPress: () => { this.zoom = 1; this.render(); } });
    this.image = h("img", { src: this.url, alt: path, class: "picture-image", draggable: "false" });
    this.image.addEventListener("load", () => {
      this.natural = { width: this.image.naturalWidth, height: this.image.naturalHeight };
      this.render();
    });
    this.image.addEventListener("error", () => this.element.replaceChildren(surfaceFailure("Unable to load image.")));
    const scroll = h("div", { class: "picture-scroll" }, this.stage);
    this.stage.append(this.image);
    this.element.append(
      h("div", { class: "picture-bar" }, this.size,
        h("div", { class: "picture-actions" }, this.out.element, this.percent, this.in.element, this.fit.element, this.actual.element)),
      scroll);
    this.stop = observeResize(scroll, () => {
      this.viewport = { width: scroll.clientWidth, height: scroll.clientHeight };
      this.render();
    });
    this.render();
  }

  private scale(): number {
    const { natural, viewport } = this;
    const fit = natural.width
      ? Math.min(1, Math.max(1, viewport.width - 32) / natural.width, Math.max(1, viewport.height - 32) / natural.height)
      : 1;
    return this.zoom ?? fit;
  }

  private change(factor: number) {
    this.zoom = Math.max(0.05, Math.min(8, this.scale() * factor));
    this.render();
  }

  private render() {
    const { natural } = this;
    const scale = this.scale();
    this.size.textContent = natural.width ? `${natural.width.toLocaleString()} × ${natural.height.toLocaleString()}` : "Loading image…";
    this.percent.textContent = `${Math.round(scale * 100)}%`;
    this.out.update({ disabled: !natural.width || scale <= 0.05 });
    this.in.update({ disabled: !natural.width || scale >= 8 });
    this.fit.update({ pressed: this.zoom === null });
    this.actual.update({ pressed: this.zoom === 1 });
    this.stage.style.width = natural.width ? `${natural.width * scale + 32}px` : "";
    this.stage.style.height = natural.height ? `${natural.height * scale + 32}px` : "";
    this.image.style.width = natural.width ? `${natural.width * scale}px` : "";
    this.image.style.height = natural.width ? `${natural.height * scale}px` : "";
    this.image.style.maxWidth = natural.width ? "none" : "100%";
  }

  dispose() {
    this.stop();
    URL.revokeObjectURL(this.url);
  }
}
