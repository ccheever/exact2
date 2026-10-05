// The arena's textures, drawn per pixel: a trim-sheet wall panel, floor tiles,
// painted crates, the deck's glowing hazard plate, a logo decal, brushed gunmetal
// and an HDR sunset (and night) sky for image-based lighting.
import { Canvas, fbm, noise, clamp, mix, mix3, normalMap, textCovers, textWidth } from './png.mjs';

const S = 512;
const tileEdge = (u, n) => { const f = (u * n) % 1; return Math.min(f, 1 - f) * 2; }; // 0 at a seam, 1 mid-tile
const seam = (u, v, n, w) => Math.min(tileEdge(u, n), tileEdge(v, n)) < w;
const cell = (u, v, n) => [Math.floor(u * n), Math.floor(v * n)];
const bolt = (u, v, n, r = 0.035) => {
  const fu = (u * n) % 1, fv = (v * n) % 1;
  return [[0.1, 0.1], [0.9, 0.1], [0.1, 0.9], [0.9, 0.9]].some(([a, b]) => Math.hypot(fu - a, fv - b) < r);
};

/** Wall panels: a 2 × 2 m tile of four light grey plates with seams, bolts and grime. */
export function panel() {
  const h = (u, v) => (seam(u, v, 2, 0.02) ? 0 : 0.6) + (bolt(u, v, 2) ? 0.35 : 0) + fbm(u, v, 8, 3, 3) * 0.05;
  const albedo = new Canvas(S, S).each((x, y, u, v) => {
    const [cx, cy] = cell(u, v, 2), tone = 0.62 + 0.06 * noise(cx + 0.5, cy + 0.5, 2, 9);
    const grime = fbm(u, v, 6, 4, 1), streak = fbm(u * 0.3, v * 3, 4, 3, 5);
    let c = [tone * 0.86, tone * 0.93, tone * 1.04].map((t) => t * (0.88 + 0.12 * grime) * (1 - 0.18 * Math.max(0, streak - 0.55)));
    if (seam(u, v, 2, 0.02)) c = c.map((t) => t * 0.35);
    if (bolt(u, v, 2)) c = [0.42, 0.43, 0.45];
    // A thin accent stripe across each plate's upper third.
    const fv = (v * 2) % 1;
    if (fv > 0.28 && fv < 0.31) c = [0.95, 0.42, 0.08];
    return c;
  });
  const mr = new Canvas(S, S).each((x, y, u, v) => {
    const metal = bolt(u, v, 2) ? 0.9 : 0.15, rough = seam(u, v, 2, 0.02) ? 0.9 : 0.38 + 0.25 * fbm(u, v, 6, 3, 2);
    return [0, rough, metal, 1];
  });
  return { albedo, normal: normalMap(S, h, 6), mr };
}

/** Floor: 1 m slate tiles (four to a 2 m texture), worn, with inlaid lines. */
export function floor() {
  const h = (u, v) => (seam(u, v, 2, 0.015) ? 0.1 : 0.5) + fbm(u, v, 16, 4, 7) * 0.12;
  const albedo = new Canvas(S, S).each((x, y, u, v) => {
    const [cx, cy] = cell(u, v, 2), tone = 0.16 + 0.04 * noise(cx + 0.5, cy + 0.5, 2, 4);
    const wear = fbm(u, v, 5, 5, 11);
    let c = [tone * 0.92, tone * 0.97, tone * 1.08].map((t) => t * (0.8 + 0.4 * wear));
    if (seam(u, v, 2, 0.015)) c = [0.05, 0.05, 0.06];
    // Inlaid guide lines along one edge of each tile.
    const fu = (u * 2) % 1;
    if (fu > 0.47 && fu < 0.5 && !seam(u, v, 2, 0.08)) c = [0.38, 0.4, 0.44];
    return c;
  });
  const mr = new Canvas(S, S).each((x, y, u, v) => [0, seam(u, v, 2, 0.015) ? 0.95 : 0.55 + 0.3 * fbm(u, v, 12, 3, 13), 0.05, 1]);
  return { albedo, normal: normalMap(S, h, 5), mr };
}

/** Crates: orange paint over steel, ribbed, chipped at the edges, stencilled. */
export function crate() {
  const rib = (u) => Math.abs(((u * 8) % 1) - 0.5) < 0.06;
  const chip = (u, v) => {
    const edge = Math.min(u, 1 - u, v, 1 - v);
    return edge < 0.08 && fbm(u, v, 12, 3, 21) > 0.45 + edge * 4;
  };
  const stencil = (x, y) => textCovers('RIVALS', S / 2 - textWidth('RIVALS', 10) / 2, S / 2 - 35, 10, x, y);
  const h = (u, v) => (rib(u) && Math.min(v, 1 - v) > 0.12 ? 0.6 : 0.3) + (Math.min(u, 1 - u, v, 1 - v) < 0.05 ? 0.4 : 0) - (chip(u, v) ? 0.15 : 0);
  const albedo = new Canvas(S, S).each((x, y, u, v) => {
    const dirt = fbm(u, v, 6, 4, 17);
    if (chip(u, v)) return [0.5, 0.5, 0.52];
    let c = [0.86, 0.36, 0.06].map((t) => t * (0.78 + 0.3 * dirt));
    if (Math.min(u, 1 - u, v, 1 - v) < 0.05) c = [0.2, 0.2, 0.22];
    if (stencil(x, y)) c = [0.08, 0.08, 0.09];
    return c;
  });
  const mr = new Canvas(S, S).each((x, y, u, v) => chip(u, v) || Math.min(u, 1 - u, v, 1 - v) < 0.05 ? [0, 0.35, 0.95, 1] : [0, 0.5, 0.1, 1]);
  return { albedo, normal: normalMap(S, h, 5), mr };
}

/** The deck: teal armour plate with hazard edges; the emission map lights its seams. */
export function deck() {
  const hazard = (u, v) => Math.min(v, 1 - v) < 0.09;
  const stripe = (u, v) => (((u + v) * 10) % 1) < 0.5;
  const h = (u, v) => (seam(u, v, 4, 0.02) ? 0 : 0.5) + (hazard(u, v) ? 0.15 : 0) + fbm(u, v, 8, 3, 31) * 0.05;
  const albedo = new Canvas(S, S).each((x, y, u, v) => {
    if (hazard(u, v)) return stripe(u, v) ? [0.95, 0.72, 0.05] : [0.04, 0.04, 0.04];
    const t = 0.8 + 0.3 * fbm(u, v, 6, 4, 33);
    return seam(u, v, 4, 0.02) ? [0.02, 0.05, 0.06] : [0.08 * t, 0.36 * t, 0.42 * t];
  });
  const emissive = new Canvas(S, S).each((x, y, u, v) => (seam(u, v, 4, 0.012) && !hazard(u, v) ? [0.2, 0.95, 1] : [0, 0, 0]));
  const mr = new Canvas(S, S).each((x, y, u, v) => [0, hazard(u, v) ? 0.7 : 0.32, 0.55, 1]);
  return { albedo, normal: normalMap(S, h, 6), mr, emissive };
}

/** A banner: RIVALS in block capitals over a team-coloured gradient. */
export function logo(top, bottom) {
  const W = 1024, H = 256, scale = 22, w = textWidth('RIVALS', scale);
  return new Canvas(W, H).each((x, y, u, v) => {
    const bg = mix3(top, bottom, v);
    const frame = Math.min(u * W, (1 - u) * W, v * H, (1 - v) * H) < 10;
    if (textCovers('RIVALS', W / 2 - w / 2, H / 2 - 3.5 * scale, scale, x, y)) return [1, 1, 1];
    if (frame) return [0.95, 0.95, 0.95];
    const chevron = Math.abs(((u * 24 + Math.abs(v - 0.5) * 6) % 1) - 0.5) < 0.12;
    return bg.map((c) => c * (chevron ? 1.15 : 0.85));
  });
}

/** Brushed gunmetal and its roughness, for the weapons. */
export function gunmetal() {
  const S2 = 256;
  const albedo = new Canvas(S2, S2).each((x, y, u, v) => {
    const b = 0.18 + 0.05 * fbm(u * 0.1, v * 6, 4, 3, 41) + 0.03 * noise(u * 256, v * 4, 256, 43);
    return [b, b * 1.02, b * 1.06];
  });
  const mr = new Canvas(S2, S2).each((x, y, u, v) => [0, 0.34 + 0.2 * fbm(u * 0.2, v * 8, 4, 3, 45), 0.65, 1]);
  const normal = normalMap(S2, (u, v) => noise(u * 128, v * 2, 128, 47) * 0.2, 2);
  return { albedo, mr, normal };
}

/** A soft smoke puff for the particles: lumpy, brightest at its core, clear at the rim. */
export function smoke() {
  const S2 = 128;
  return new Canvas(S2, S2).each((x, y, u, v) => {
    const r = Math.hypot(u - 0.5, v - 0.5) * 2;
    const lump = fbm(u, v, 4, 4, 61);
    const a = clamp((1 - r) * 1.6 - (1 - lump) * 0.9) ** 1.4;
    const shade = 0.75 + 0.25 * fbm(u, v, 8, 3, 63);
    return [shade, shade, shade, a];
  });
}

const unit = (v) => { const l = Math.hypot(...v); return v.map((x) => x / l); };
/** Where the game's sun and moon shine from (art.rs places the lights there too). */
export const SUN = unit([-24, 9, -34]), MOON = unit([-14, 22, -9]);

/** An equirectangular sky as RGBM (radiance = rgb × a × range) for EnvironmentMap:
 * it lights the arena and is drawn as its visible sky. */
export function sky(night = false, range = 8) {
  const W = 1024, H = 512, floor = night ? 0.06 : 0.1;
  const sun = night ? MOON : SUN;
  const out = new Canvas(W, H).each((x, y, u, v) => {
    const lon = (u - 0.5) * 2 * Math.PI, lat = (0.5 - v) * Math.PI;
    const d = [Math.sin(lon) * Math.cos(lat), Math.sin(lat), -Math.cos(lon) * Math.cos(lat)];
    const along = Math.max(0, d[0] * sun[0] + d[1] * sun[1] + d[2] * sun[2]);
    let c;
    if (night) {
      c = mix3([0.035, 0.04, 0.07], [0.005, 0.007, 0.022], clamp(d[1] * 1.5));
      c = c.map((t, i) => t + along ** 600 * [3, 3.1, 3.4][i] + along ** 16 * [0.05, 0.06, 0.09][i]);
      const star = noise(u * 1400, v * 700, 1400, 51);
      if (d[1] > 0.05 && star > 0.994) c = c.map(() => 0.3 + (star - 0.994) * 60);
      // A faint band of glow across the sky, and thin cloud the moon lights.
      const band = Math.exp(-((d[0] * 0.8 + d[2] * 0.6 - d[1] * 0.3) ** 2) * 20) * clamp(d[1] * 3);
      c = c.map((t, i) => t + band * fbm(u * 3, v * 3, 8, 4, 57) * [0.02, 0.022, 0.03][i]);
      const cloud = d[1] > 0 ? clamp((fbm(u * 2, v * 5, 6, 5, 59) - 0.56) * 2.5) * clamp(1 - d[1] * 1.8) : 0;
      c = mix3(c, [0.05, 0.055, 0.07].map((t) => t + along ** 8 * 0.08), cloud * 0.8);
      if (d[1] < 0) c = mix3([0.03, 0.03, 0.04], [0.008, 0.008, 0.01], clamp(-d[1] * 4));
    } else {
      const horizon = [1.6, 0.72, 0.32], zenith = [0.1, 0.18, 0.5], ground = [0.08, 0.06, 0.05];
      c = d[1] >= 0 ? mix3(horizon, zenith, clamp(d[1] * 1.4) ** 0.7) : mix3([0.5, 0.3, 0.2], ground, clamp(-d[1] * 4));
      c = c.map((t, i) => t + along ** 80 * [7, 4.8, 2.6][i] + along ** 6 * [0.9, 0.38, 0.1][i]);
      // Two decks of cloud: broad banks low down, lit from the sun's side, and thin streaks above.
      const low = d[1] > 0.01 ? clamp((fbm(u * 2, v * 6, 6, 5, 53) - 0.5) * 3) * clamp(1 - d[1] * 2.4) : 0;
      const high = d[1] > 0.08 ? clamp((fbm(u * 1.5, v * 10, 5, 5, 55) - 0.58) * 3) * clamp(1.2 - d[1] * 1.6) : 0;
      const lit = (k) => [1.5, 0.66, 0.42].map((t, i) => t * (0.35 + along * 1.4) * k + [0.06, 0.05, 0.09][i]);
      c = mix3(c, lit(1), low * 0.75);
      c = mix3(c, lit(1.3), high * 0.45);
    }
    // RGBM: the largest channel picks the multiplier, quantized up. A floor keeps
    // the multiplier smooth across dark sky, where block compression would
    // otherwise turn its pixel-to-pixel swings into visible squares.
    const m = clamp(Math.max(...c, 1e-4) / range, floor, 1), mq = Math.ceil(m * 255) / 255;
    return [...c.map((t) => clamp(t / (mq * range))), mq];
  });
  return out;
}
