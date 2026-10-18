// Gamepads for world canvases (LLP 1127.003 D2), read with the W3C Gamepad API
// (https://w3c.github.io/gamepad/). The browser exposes pads only by polling,
// so while one is connected an animation-frame loop reads them and sends the
// focused world a `pad` event when a button or an axis changed — a change, not
// a sample per frame, so the recording is the pad's, not the display's. A pad
// without the standard mapping is reported connected and sends nothing more.

const STEP = 256; // values travel in 1/256 steps: exact in an f32, and a stick's noise below that is no change
const quantize = (v, low) => Math.min(1, Math.max(low, Math.round(v * STEP) / STEP)) || 0;
const BUTTONS = 17, AXES = 4, MAX_PADS = 16;

/** One standard-mapping reading, or zeros for a pad without that mapping. */
export function padReading(pad) {
  const standard = pad.mapping === "standard";
  const buttons = Array.from({ length: BUTTONS }, (_, i) => {
    const b = standard ? pad.buttons[i] : undefined;
    // A digital button some browsers report pressed with value 0.
    return b ? quantize(b.pressed && !b.value ? 1 : b.value, 0) : 0;
  });
  const axes = Array.from({ length: AXES }, (_, i) => (standard && Number.isFinite(pad.axes[i]) ? quantize(pad.axes[i], -1) : 0));
  return { standard, buttons, axes };
}

/** Poll the pads into `target()`'s world each frame while any is connected. `target` returns the focused world's entry
 * (with `pads`, the readings it was last sent, and `sendPad`) or null. */
export function padPoller(target) {
  let loop = null;
  const poll = () => {
    loop = null;
    const pads = [...(navigator.getGamepads?.() ?? [])].filter((p) => p && p.connected !== false && p.index < MAX_PADS);
    const entry = document.hasFocus() ? target() : null;
    if (entry) {
      const seen = new Set();
      for (const pad of pads) {
        seen.add(pad.index);
        const reading = padReading(pad), key = JSON.stringify(reading);
        if (entry.pads.get(pad.index) === key) continue;
        entry.pads.set(pad.index, key);
        entry.sendPad({ t: "pad", index: pad.index, connected: true, ...reading });
      }
      for (const index of [...entry.pads.keys()]) if (!seen.has(index)) {
        entry.pads.delete(index);
        entry.sendPad({ t: "pad", index, connected: false, standard: true, buttons: Array(BUTTONS).fill(0), axes: Array(AXES).fill(0) });
      }
    }
    if (pads.length) loop = requestAnimationFrame(poll); // else gamepadconnected starts it again
  };
  const start = () => { if (loop === null) loop = requestAnimationFrame(poll); };
  window.addEventListener("gamepadconnected", start);
  // A pad that went away is reported by the next poll; the loop stops once none is left.
  window.addEventListener("gamepaddisconnected", start);
  start();
}
