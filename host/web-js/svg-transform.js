// A dynamic SVG `transform` on the JS target (rt.js re-exports it; bundled
// only into a plan with the row).
/** SVG's `transform` grammar restated as CSS's (kernel `TransformList`):
 * units optional, `rotate(a cx cy)` as three functions; text it refuses is
 * no transform. */
export function svgTransform(v) {
  if (v == null) return v;
  const t = String(v).trim();
  if (!t || /^none$/i.test(t)) return "none";
  const num = a => { const n = Number(a); return isFinite(n) ? n : NaN; };
  const len = a => num(a.replace(/px$/, ""));
  const ang = a => { const m = /^(.*?)(deg|grad|rad|turn)?$/.exec(a); return num(m[1]) * ({ grad: 0.9, rad: 180 / Math.PI, turn: 360 }[m[2]] ?? 1); };
  const out = [], re = /([a-zA-Z]+)\s*\(([^)]*)\)/g;
  let m, at = 0;
  while ((m = re.exec(t))) {
    if (t.slice(at, m.index).replace(/[\s,]/g, "")) return null;
    at = re.lastIndex;
    const a = m[2].split(/[\s,]+/).filter(Boolean), name = m[1].toLowerCase(), n = a.length;
    let f = null;
    if (name === "matrix" && n === 6) f = `matrix(${a.map(num).join(", ")})`;
    else if (name === "translate" && (n === 1 || n === 2)) f = `translate(${len(a[0])}px, ${n === 2 ? len(a[1]) : 0}px)`;
    else if (name === "translatex" && n === 1) f = `translate(${len(a[0])}px, 0px)`;
    else if (name === "translatey" && n === 1) f = `translate(0px, ${len(a[0])}px)`;
    else if (name === "scale" && (n === 1 || n === 2)) f = `scale(${num(a[0])}, ${num(a[n - 1])})`;
    else if (name === "scalex" && n === 1) f = `scale(${num(a[0])}, 1)`;
    else if (name === "scaley" && n === 1) f = `scale(1, ${num(a[0])})`;
    else if (name === "rotate" && n === 1) f = `rotate(${ang(a[0])}deg)`;
    else if (name === "rotate" && n === 3) { const [x, y] = [num(a[1]), num(a[2])]; f = `translate(${x}px, ${y}px) rotate(${ang(a[0])}deg) translate(${-x}px, ${-y}px)`; }
    else if ((name === "skewx" || name === "skew") && n === 1) f = `skewX(${ang(a[0])}deg)`;
    else if (name === "skewy" && n === 1) f = `skewY(${ang(a[0])}deg)`;
    else if (name === "skew" && n === 2) f = `skew(${ang(a[0])}deg, ${ang(a[1])}deg)`;
    if (!f || f.includes("NaN")) return null;
    out.push(f);
  }
  return t.slice(at).replace(/[\s,]/g, "") || !out.length ? null : out.join(" ");
}
