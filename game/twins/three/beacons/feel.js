// Opt-in observer only. Game/simulation source and its live clock are untouched.
import * as THREE from 'three';

const STRIDE = 8; // rAF ms, render-boundary ms, player xyz, camera xyz
let frames, events, count = 0, eventCount = 0, active = false, overflow = false;
let stamp = -1, lastStamp = -1, armedTrial = -1, hidden = 0, unfocused = 0;
let width = 0, height = 0;
const codes = { Enter: 13, KeyW: 87, KeyD: 68, KeyS: 83, KeyA: 65, Space: 32 };
function frameTime(t) { stamp = t; requestAnimationFrame(frameTime); }
requestAnimationFrame(frameTime);

// The real mesh callback runs immediately before WebGLRenderer submits it.
// This hook also sees the actual render camera, without exporting game internals.
const beforeRender = THREE.Mesh.prototype.onBeforeRender;
THREE.Mesh.prototype.onBeforeRender = function(renderer, scene, camera, geometry, material, group) {
  if (active && geometry.type === 'CapsuleGeometry' && stamp !== lastStamp) {
    lastStamp = stamp;
    if ((count + 1) * STRIDE > frames.length) overflow = true;
    else {
      const i = count++ * STRIDE, p = this.position, c = camera.position;
      frames[i] = stamp; frames[i + 1] = performance.now();
      frames[i + 2] = p.x; frames[i + 3] = p.y; frames[i + 4] = p.z;
      frames[i + 5] = c.x; frames[i + 6] = c.y; frames[i + 7] = c.z;
      hidden += document.visibilityState !== 'visible'; unfocused += !document.hasFocus();
      width = renderer.domElement.width; height = renderer.domElement.height;
    }
  }
  beforeRender.call(this, renderer, scene, camera, geometry, material, group);
};
function input(e) {
  if (!active || !(e.code in codes) || e.repeat) return;
  if ((eventCount + 1) * 4 > events.length) { overflow = true; return; }
  const i = eventCount++ * 4;
  events[i] = performance.now(); events[i + 1] = codes[e.code];
  events[i + 2] = +(e.type === 'keydown'); events[i + 3] = armedTrial;
  armedTrial = -1;
}
addEventListener('keydown', input, true);
addEventListener('keyup', input, true);

window.feel = {
  begin(durationMs) {
    frames = new Float64Array(Math.ceil((durationMs / 1000 + 10) * 1000) * STRIDE);
    events = new Float64Array(128 * 4);
    active = true;
  },
  arm(trial) { armedTrial = trial; },
  end() {
    active = false;
    return { schema: 1, stride: STRIDE, frames: Array.from(frames.subarray(0, count * STRIDE)),
      events: Array.from(events.subarray(0, eventCount * 4)), overflow,
      hidden_frames: hidden, unfocused_frames: unfocused,
      window_pixels: [width, height], viewport_css: [innerWidth, innerHeight],
      device_pixel_ratio: devicePixelRatio, engine_version: `three.js r${THREE.REVISION}`,
      timestamp_source: 'requestAnimationFrame; render-boundary performance.now for latency',
      interpolation: false };
  },
};
