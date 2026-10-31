// @ref LLP 1042 §9. `requestPictureInPicture(id)` on the JS target: the image's
// pictures are drawn onto a canvas, its `captureStream()` plays in a hidden
// video, and the video floats by HTMLVideoElement.requestPictureInPicture(),
// as a page floats pictures that are not a video. A picture the image loads
// next is drawn again; the image leaving the document closes the window.

// One floating element per document, as the Picture-in-Picture API keeps.
let active = null;

export function float(img, id, say) {
  const refuse = why => say(`requestPictureInPicture "${id}" refused: ${why}`);
  if (img?.localName !== "img") return refuse(img ? "not an image" : "no live node with that id");
  if (active?.img === img) return;
  active?.stop(true);
  if (typeof HTMLCanvasElement.prototype.captureStream !== "function" || !document.pictureInPictureEnabled) return refuse("this browser does not float a canvas's stream");
  const canvas = document.createElement("canvas"), context = canvas.getContext("2d");
  const draw = () => {
    if (!img.naturalWidth) return;
    if (canvas.width !== img.naturalWidth || canvas.height !== img.naturalHeight) { canvas.width = img.naturalWidth; canvas.height = img.naturalHeight; }
    context.drawImage(img, 0, 0);
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
  // A removed image loads nothing more: looked for once a second.
  const watch = setInterval(() => { if (!img.isConnected) current.stop(false); }, 1000);
  const current = {
    img,
    stop(heard) {
      if (active !== current) return;
      active = null;
      clearInterval(watch);
      img.removeEventListener("load", draw);
      video.removeEventListener("leavepictureinpicture", leave);
      if (document.pictureInPictureElement === video) document.exitPictureInPicture().catch(() => {});
      for (const track of stream.getTracks()) track.stop();
      video.remove();
      if (heard && entered) img.dispatchEvent(new Event("leavepictureinpicture"));
    },
  };
  active = current;
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
    .catch(error => { refuse(error.name); current.stop(false); });
}
