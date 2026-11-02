// @ref LLP 1042 §9. `requestPictureInPicture(id)` on the JS target: the image's
// pictures are drawn onto a canvas, its `captureStream()` plays in a hidden
// video, and the video floats by HTMLVideoElement.requestPictureInPicture(),
// as a page floats pictures that are not a video. A picture the image loads
// next is drawn again; the image leaving the document closes the window. A
// picture from another origin without CORS taints the canvas, and the browser
// then sends none of its frames (Media Capture from DOM Elements, origin-clean),
// so such a picture is refused, not floated blank.
//
// `presentation: "window"` or `"panel"` (LLP 1042 §9.1) asks for a window the page draws:
// Document Picture-in-Picture's, where the browser has it, holding an <img>
// that loads each picture the image loads; its document's title is `title`
// and it opens at `width` by `height` at the picture's aspect. A browser
// without it floats the default way.

// One floating element per document: the browser keeps one picture-in-picture window, and
// one Document Picture-in-Picture window, so a request closes the one before, of either kind.
let active = null;

// A failed request is the element's `pictureinpictureerror`, whose detail is the reason: HTML's
// rejected promise, as an event.
export function float(img, id, say, options = {}) {
  const refuse = why => { say(`requestPictureInPicture "${id}" refused: ${why}`); img?.dispatchEvent?.(new CustomEvent("pictureinpictureerror", { detail: why })); };
  if (img?.localName !== "img") return refuse(img ? "not an image" : "no live node with that id");
  if (active?.img === img) return;
  active?.stop(true);
  if ((options.presentation === "window" || options.presentation === "panel") && typeof globalThis.documentPictureInPicture?.requestWindow === "function") return floatWindow(img, options, refuse);
  if (typeof HTMLCanvasElement.prototype.captureStream !== "function" || !document.pictureInPictureEnabled) return refuse("this browser does not float a canvas's stream");
  const canvas = document.createElement("canvas"), context = canvas.getContext("2d");
  let tainted = false, current = null;
  const draw = () => {
    if (!img.naturalWidth) return;
    if (canvas.width !== img.naturalWidth || canvas.height !== img.naturalHeight) { canvas.width = img.naturalWidth; canvas.height = img.naturalHeight; }
    context.drawImage(img, 0, 0);
    try { context.getImageData(0, 0, 1, 1); } catch {
      tainted = true;
      refuse("a picture from another origin needs CORS (Access-Control-Allow-Origin)");
      current?.stop(true);
    }
  };
  canvas.width = 640; canvas.height = 400;
  // The stream sends a frame per draw, so the first is drawn after it starts.
  const stream = canvas.captureStream();
  if (img.complete && img.naturalWidth) draw(); else context.fillRect(0, 0, canvas.width, canvas.height);
  const video = Object.assign(document.createElement("video"), { muted: true, playsInline: true, srcObject: stream });
  // Safari floats only a video in the document.
  video.style.cssText = "position:fixed;right:0;bottom:0;width:1px;height:1px;opacity:0;pointer-events:none";
  document.body.append(video);
  let entered = false;
  const leave = () => current.stop(true);
  // A removed image loads nothing more: the window closes as it leaves.
  const watch = new MutationObserver(() => { if (!img.isConnected) current.stop(false); });
  watch.observe(document, { childList: true, subtree: true });
  current = {
    img,
    stop(heard) {
      if (active !== current) return;
      active = null;
      watch.disconnect();
      img.removeEventListener("load", draw);
      video.removeEventListener("leavepictureinpicture", leave);
      if (document.pictureInPictureElement === video) document.exitPictureInPicture().catch(() => {});
      for (const track of stream.getTracks()) track.stop();
      video.remove();
      if (heard && entered) img.dispatchEvent(new Event("leavepictureinpicture"));
    },
  };
  active = current;
  if (tainted) return current.stop(false);
  img.addEventListener("load", draw);
  video.play()
    .then(() => video.readyState >= HTMLMediaElement.HAVE_METADATA || new Promise(ok => video.addEventListener("loadedmetadata", ok, { once: true })))
    .then(() => active === current && video.requestPictureInPicture())
    .then(() => {
      if (active !== current) return;
      entered = true;
      video.addEventListener("leavepictureinpicture", leave);
      img.dispatchEvent(new Event("enterpictureinpicture"));
    })
    .catch(error => { current.stop(false); refuse(error.name); });
}

// The window's size before it takes the picture's aspect: the area of `width` by `height`
// (480 by 320 where neither is given) at the picture's aspect, as the window opens.
export function windowSize(options, natural) {
  const width = Math.max(1, Number(options.width) || 480), height = Math.max(1, Number(options.height) || 320);
  if (!natural?.[0] || !natural?.[1]) return [Math.round(width), Math.round(height)];
  const aspect = natural[0] / natural[1], w = Math.sqrt(width * height * aspect);
  return [Math.round(w), Math.round(w / aspect)];
}

function floatWindow(img, options, refuse) {
  let current = null, win = null;
  const shown = () => { if (win && !win.closed) win.document.querySelector("img").src = img.currentSrc || img.src; };
  const watch = new MutationObserver(() => { if (!img.isConnected) current.stop(false); });
  let entered = false;
  current = {
    img,
    panel: options.presentation === "panel",
    stop(heard) {
      if (active !== current) return;
      active = null;
      watch.disconnect();
      img.removeEventListener("load", shown);
      if (win && !win.closed) { win.removeEventListener("pagehide", left); win.close(); }
      if (heard && entered) img.dispatchEvent(new Event("leavepictureinpicture"));
    },
  };
  const left = () => current.stop(true);
  active = current;
  watch.observe(document, { childList: true, subtree: true });
  const [width, height] = windowSize(options, [img.naturalWidth, img.naturalHeight]);
  globalThis.documentPictureInPicture.requestWindow({ width, height })
    .then(opened => {
      win = opened;
      if (active !== current) return win.close();
      const doc = win.document;
      doc.title = options.title ?? document.title;
      doc.body.style.cssText = "margin:0;height:100vh;background:#000;display:grid;place-items:center;overflow:hidden";
      const picture = Object.assign(doc.createElement("img"), { alt: doc.title, draggable: false });
      picture.style.cssText = "width:100%;height:100%;object-fit:contain;user-select:none";
      doc.body.append(picture);
      shown();
      img.addEventListener("load", shown);
      win.addEventListener("pagehide", left, { once: true });
      entered = true;
      img.dispatchEvent(new Event("enterpictureinpicture"));
    })
    .catch(error => { current.stop(false); refuse(error.name); });
}

/** `exitPictureInPicture()`: the floating image leaves the window and hears it, as HTML's
 *  document.exitPictureInPicture() ends the document's one picture-in-picture element. A panel
 *  is a window of its own, as on macOS, and stays. */
export function exit(say) {
  if (!active || active.panel) return say("exitPictureInPicture ignored: no image is in the picture-in-picture window");
  active.stop(true);
}
