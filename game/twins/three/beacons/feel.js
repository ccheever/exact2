// Opt-in observer only. Game/simulation source and its live clock are untouched.
import * as THREE from 'three';
import { inputRecorder } from '../../../bench/probes/input.mjs';
const inputs = inputRecorder();

const STRIDE = 8; // rAF ms, render-boundary ms, player xyz, camera xyz
let frames, count = 0, active = false, overflow = false;
let stamp = -1, lastStamp = -1, hidden = 0, unfocused = 0;
let width = 0, height = 0;
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
window.feel = {
  begin(durationMs) {
    frames = new Float64Array(Math.ceil((durationMs / 1000 + 10) * 1000) * STRIDE);
    inputs.begin();
    count = 0; lastStamp = -1; hidden = 0; unfocused = 0; overflow = false;
    active = true;
  },
  arm(trial) { inputs.arm(trial); },
  end() {
    active = false;
    const delivered = inputs.end();
    return { schema: 1, stride: STRIDE, frames: Array.from(frames.subarray(0, count * STRIDE)),
      ...delivered, overflow: overflow || delivered.overflow,
      hidden_frames: hidden, unfocused_frames: unfocused,
      window_pixels: [width, height], viewport_css: [innerWidth, innerHeight],
      device_pixel_ratio: devicePixelRatio, engine_version: `three.js r${THREE.REVISION}`,
      timestamp_source: 'requestAnimationFrame; render-boundary performance.now for latency',
      interpolation: false };
  },
};
