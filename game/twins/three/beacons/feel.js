// Opt-in observer only. Game/simulation source and its live clock are untouched.
import * as THREE from 'three';
import { inputRecorder } from '../../../bench/probes/input.mjs';
const inputs = inputRecorder();

const STRIDE = 8; // rAF ms, render-boundary ms, player xyz, camera xyz
const vp = new THREE.Matrix4(), player = new THREE.Vector3(), eye = new THREE.Vector3();
let projection;
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
      const j = count * 18, i = count++ * STRIDE;
      const p = player.setFromMatrixPosition(this.matrixWorld), c = eye.setFromMatrixPosition(camera.matrixWorld);
      vp.multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
      projection.set(vp.elements, j);
      projection[j + 16] = renderer.domElement.width; projection[j + 17] = renderer.domElement.height;
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
    projection = new Float64Array(frames.length / STRIDE * 18);
    inputs.begin();
    count = 0; lastStamp = -1; hidden = 0; unfocused = 0; overflow = false;
    active = true;
  },
  arm(trial) { inputs.arm(trial); },
  end() {
    active = false;
    const delivered = inputs.end();
    return { schema: 1, stride: STRIDE, frames: Array.from(frames.subarray(0, count * STRIDE)),
      raw_callback_ms: Array.from(frames.subarray(0, count * STRIDE)).filter((_, i) => i % STRIDE === 0),
      drawn_clock_ms: Array.from(frames.subarray(0, count * STRIDE)).filter((_, i) => i % STRIDE === 0),
      camera_projection: Array.from(projection.subarray(0, count * 18)),
      ...delivered, overflow: overflow || delivered.overflow,
      hidden_frames: hidden, unfocused_frames: unfocused,
      window_pixels: [width, height], viewport_css: [innerWidth, innerHeight],
      device_pixel_ratio: devicePixelRatio, engine_version: `three.js r${THREE.REVISION}`,
      timestamp_source: 'raw and drawn: requestAnimationFrame argument; render-boundary performance.now for latency',
      interpolation: false };
  },
};
