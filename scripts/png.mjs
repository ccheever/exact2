// PNG, the little the fixtures and the agent's film need and nothing more:
// decode an 8-bit, non-interlaced RGB or RGBA image to RGBA bytes; encode RGBA
// bytes as an unfiltered RGBA image, as an animated PNG of equal frames, or as
// a contact sheet of them. Node's zlib does the compression; this does the
// chunks, the filters, and the CRC. No dependency (rules/RULES.md: none).
import { deflateSync, inflateSync } from 'node:zlib';

/** Find a simulator framebuffer inside its window picture. No bezel constants:
 * compare a spatially distributed set of contrasting pixels, then validate the
 * whole interior. Uniform/ambiguous pictures refuse instead of inventing a map.
 * Result coordinates are pixels of `window`, with uniform `scale` from `screen`.
 * @ref LLP 1035.003 D3 — derive the desktop mapping from observed geometry. */
export function locateScreen(window, screen) {
  const rgb = (im, x, y) => {
    const at = (Math.max(0, Math.min(im.height - 1, Math.round(y))) * im.width + Math.max(0, Math.min(im.width - 1, Math.round(x)))) * 4;
    return [im.data[at], im.data[at + 1], im.data[at + 2]];
  };
  const points = [];
  // In each tile, keep the most distinctive adjacent pair. A page that is
  // mostly white must not match arbitrary white window chrome with low error.
  const step = Math.max(2, Math.round(screen.width / 100));
  for (let gy = 1; gy < 9; gy++) for (let gx = 1; gx < 6; gx++) {
    let best = null;
    for (let y = screen.height * gy / 10; y < screen.height * (gy + 1) / 10; y += step) {
      for (let x = screen.width * gx / 7; x < screen.width * (gx + 1) / 7; x += step) {
        const a = rgb(screen, x, y), b = rgb(screen, x + step, y + step);
        const contrast = a.reduce((s, v, i) => s + Math.abs(v - b[i]), 0);
        if (!best || contrast > best.contrast) best = { x, y, a, b, contrast };
      }
    }
    if (best?.contrast > 90) points.push(best);
  }
  if (points.length < 8) return { error: 'the simulator picture has too little detail to calibrate safely' };
  points.sort((a, b) => b.contrast - a.contrast);
  const samples = points.slice(0, 24).flatMap(p => [{x:p.x, y:p.y, c:p.a}, {x:p.x+step, y:p.y+step, c:p.b}]);
  const score = (x, y, scale, cutoff = Infinity) => {
    let sum = 0;
    for (const p of samples) {
      const c = rgb(window, x + p.x * scale, y + p.y * scale);
      for (let k = 0; k < 3; k++) sum += Math.abs(c[k] - p.c[k]);
      if (sum > cutoff * samples.length * 3) return Infinity;
    }
    return sum / (samples.length * 3);
  };
  const maximum = Math.min(window.width / screen.width, window.height / screen.height);
  let candidates = [];
  // Retain several basins before pixel/subpixel refinement; thin text can
  // make the best coarse sample differ from the best actual alignment.
  let separation = 4;
  const distance = (a,b) => Math.max(Math.abs(a.x-b.x), Math.abs(a.y-b.y),
    Math.abs(a.x+screen.width*a.scale-b.x-screen.width*b.scale), Math.abs(a.y+screen.height*a.scale-b.y-screen.height*b.scale));
  const keep = c => {
    const near = candidates.findIndex(p => distance(c,p) < separation);
    if (near >= 0) { if (c.error >= candidates[near].error) return; candidates.splice(near,1); }
    candidates.push(c); candidates.sort((a,b) => a.error-b.error); candidates.length = Math.min(24,candidates.length);
  };
  for (let width = screen.width * maximum * 0.45; width <= screen.width * maximum; width += 2) {
    const scale = width / screen.width, h = screen.height * scale;
    for (let y = 0; y <= window.height - h; y += 3) for (let x = 0; x <= window.width - width; x += 3) {
      const error = score(x, y, scale, candidates.length < 24 ? Infinity : candidates.at(-1).error);
      if (candidates.length < 24 || error <= candidates.at(-1).error) keep({x,y,scale,error});
    }
  }
  for (const delta of [1, 0.25]) {
    const seeds = candidates; candidates = [];
    separation = delta === 1 ? 3 : 1;
    for (const seed of seeds) for (let dw=-2; dw<=2; dw++) for (let dy=-3;dy<=3;dy++) for (let dx=-3;dx<=3;dx++) {
      const x=seed.x+dx*delta, y=seed.y+dy*delta, scale=seed.scale+dw*delta/screen.width;
      if (x<0 || y<0 || x+screen.width*scale>window.width+1 || y+screen.height*scale>window.height+1) continue;
      const error=score(x,y,scale,candidates.length < 24 ? Infinity : candidates.at(-1).error);
      if (candidates.length<24 || error<=candidates.at(-1).error) keep({x,y,scale,error});
    }
  }
  const best = candidates[0];
  if (!best || best.error > 38) return { error: 'the simulator framebuffer does not match its window picture' };
  if (candidates.some(c => distance(c,best)>3 && c.error<=best.error+3)) return { error: 'the simulator window picture has more than one plausible screen mapping' };
  let total=0, count=0;
  for (let gy=1;gy<20;gy++) for (let gx=1;gx<10;gx++) {
    const x=screen.width*gx/10, y=screen.height*gy/20;
    const a=rgb(screen,x,y), b=rgb(window,best.x+x*best.scale,best.y+y*best.scale);
    for (let k=0;k<3;k++) {total+=Math.abs(a[k]-b[k]);count++;}
  }
  if (total/count > 15) return { error: 'the simulator picture changed or its window match is inconsistent' };
  return { x: best.x, y: best.y, scale: best.scale, score: best.error };
}

/** {width, height, data: Uint8Array of RGBA} from a PNG buffer. */
export function decodePng(buf) {
  const sig = [137, 80, 78, 71, 13, 10, 26, 10];
  for (let i = 0; i < 8; i++) if (buf[i] !== sig[i]) throw new Error('not a PNG');
  let pos = 8, width = 0, height = 0, depth = 0, color = 0, interlace = 0;
  const idat = [];
  while (pos + 8 <= buf.length) {
    const len = buf.readUInt32BE(pos);
    const type = buf.toString('latin1', pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (type === 'IHDR') { width = data.readUInt32BE(0); height = data.readUInt32BE(4); depth = data[8]; color = data[9]; interlace = data[12]; }
    else if (type === 'IDAT') idat.push(data);
    else if (type === 'IEND') break;
    pos += 12 + len;
  }
  if (depth !== 8 || interlace !== 0 || (color !== 2 && color !== 6)) throw new Error(`unsupported PNG: depth ${depth}, color type ${color}, interlace ${interlace}`);
  const bpp = color === 6 ? 4 : 3, stride = width * bpp;
  const raw = inflateSync(Buffer.concat(idat));
  const out = new Uint8Array(width * height * 4);
  let prev = new Uint8Array(stride), cur = new Uint8Array(stride);
  for (let y = 0; y < height; y++) {
    const f = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let i = 0; i < stride; i++) {
      const a = i >= bpp ? cur[i - bpp] : 0, b = prev[i], c = i >= bpp ? prev[i - bpp] : 0;
      let x = line[i];
      switch (f) {
        case 1: x += a; break;
        case 2: x += b; break;
        case 3: x += (a + b) >> 1; break;
        case 4: { const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c); x += pa <= pb && pa <= pc ? a : pb <= pc ? b : c; break; }
      }
      cur[i] = x & 255;
    }
    for (let x = 0; x < width; x++) {
      const s = x * bpp, d = (y * width + x) * 4;
      out[d] = cur[s]; out[d + 1] = cur[s + 1]; out[d + 2] = cur[s + 2]; out[d + 3] = bpp === 4 ? cur[s + 3] : 255;
    }
    [prev, cur] = [cur, prev];
  }
  return { width, height, data: out };
}

/** An image's compressed, unfiltered scanlines. */
function scanlines({ width, height, data }) {
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) { raw[y * (stride + 1)] = 0; Buffer.from(data.buffer, data.byteOffset + y * stride, stride).copy(raw, y * (stride + 1) + 1); }
  return deflateSync(raw);
}
const SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
function header(width, height) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0); ihdr.writeUInt32BE(height, 4); ihdr[8] = 8; ihdr[9] = 6; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  return chunk('IHDR', ihdr);
}

/** A PNG buffer from {width, height, data: RGBA bytes}. */
export function encodePng(image) {
  return Buffer.concat([SIGNATURE, header(image.width, image.height), chunk('IDAT', scanlines(image)), chunk('IEND', Buffer.alloc(0))]);
}

/** An animated PNG of equal-size RGBA frames, each shown `delayMs`, looping. A viewer without APNG shows the first. */
export function encodeApng(frames, delayMs) {
  const { width, height } = frames[0];
  if (frames.some(f => f.width !== width || f.height !== height)) throw new Error('animated PNG: every frame must be one size');
  const actl = Buffer.alloc(8);
  actl.writeUInt32BE(frames.length, 0); actl.writeUInt32BE(0, 4);
  const parts = [SIGNATURE, header(width, height), chunk('acTL', actl)];
  let seq = 0;
  frames.forEach((frame, i) => {
    const fctl = Buffer.alloc(26);
    fctl.writeUInt32BE(seq++, 0); fctl.writeUInt32BE(width, 4); fctl.writeUInt32BE(height, 8);
    fctl.writeUInt16BE(Math.min(65535, Math.round(delayMs)), 20); fctl.writeUInt16BE(1000, 22);
    parts.push(chunk('fcTL', fctl));
    const data = scanlines(frame);
    if (i === 0) parts.push(chunk('IDAT', data));
    else { const fdat = Buffer.alloc(4 + data.length); fdat.writeUInt32BE(seq++, 0); data.copy(fdat, 4); parts.push(chunk('fdAT', fdat)); }
  });
  parts.push(chunk('IEND', Buffer.alloc(0)));
  return Buffer.concat(parts);
}

/** Equal-size RGBA frames in a grid, left to right, at most `columns` wide, a one-pixel gray gap between cells; shrunk by a whole factor (box average) until the sheet is at most `maxWidth` wide. */
export function contactSheet(frames, { columns = 6, maxWidth = 2048 } = {}) {
  const cols = Math.min(columns, frames.length), rows = Math.ceil(frames.length / cols);
  const k = Math.max(1, Math.ceil((cols * frames[0].width + cols - 1) / maxWidth));
  const w = Math.floor(frames[0].width / k), h = Math.floor(frames[0].height / k);
  const width = cols * w + cols - 1, height = rows * h + rows - 1;
  const data = new Uint8Array(width * height * 4).fill(128);
  for (let i = 3; i < data.length; i += 4) data[i] = 255;
  frames.forEach((f, i) => {
    const ox = (i % cols) * (w + 1), oy = Math.floor(i / cols) * (h + 1);
    for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
      const d = ((oy + y) * width + ox + x) * 4;
      for (let c = 0; c < 4; c++) {
        let sum = 0;
        for (let dy = 0; dy < k; dy++) for (let dx = 0; dx < k; dx++) sum += f.data[((y * k + dy) * f.width + x * k + dx) * 4 + c];
        data[d + c] = Math.round(sum / (k * k));
      }
    }
  });
  return { width, height, data };
}

/** The RGBA bytes of a rectangle of an image, as an image. */
export function crop({ width, data }, x, y, w, h) {
  const out = new Uint8Array(w * h * 4);
  for (let row = 0; row < h; row++) out.set(data.subarray(((y + row) * width + x) * 4, ((y + row) * width + x + w) * 4), row * w * 4);
  return { width: w, height: h, data: out };
}

/** Two images of one size compared: the share of pixels whose largest channel difference exceeds `band`, and the mean absolute difference. */
export function diff(a, b, band = 8) {
  if (a.width !== b.width || a.height !== b.height) return { differing: 1, mean: 255, size: `${a.width}×${a.height} vs ${b.width}×${b.height}` };
  let over = 0, sum = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    let m = 0;
    for (let c = 0; c < 3; c++) { const d = Math.abs(a.data[i + c] - b.data[i + c]); sum += d; if (d > m) m = d; }
    if (m > band) over++;
  }
  const n = a.data.length / 4;
  return { differing: over / n, mean: sum / (n * 3), size: `${a.width}×${a.height}` };
}

const CRC = new Int32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c; });
function crc32(buf) { let c = -1; for (const b of buf) c = CRC[(c ^ b) & 255] ^ (c >>> 8); return (c ^ -1) >>> 0; }
function chunk(type, data) {
  const out = Buffer.alloc(12 + data.length);
  out.writeUInt32BE(data.length, 0); out.write(type, 4, 'latin1'); data.copy(out, 8);
  out.writeUInt32BE(crc32(out.subarray(4, 8 + data.length)), 8 + data.length);
  return out;
}
