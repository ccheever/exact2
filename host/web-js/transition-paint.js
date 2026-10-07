// Embedded only for a bound transition. Mirrors motion/src/parse.rs and
// Transition/Easing/SpringConfig::validate, including whole-list rejection.
// src/paint.rs supplies accepted property names and their kernel policy.
v => {
  const properties = PROPERTIES, maxTransitions = MAX_TRANSITIONS, maxStops = MAX_LINEAR_STOPS;
  const trim = s => s.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, "");
  const number = s => /^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?(?![\s\S])/.test(s) ? Number(s) : NaN;
  const split = (s, sep) => {
    const out = []; let depth = 0, start = 0;
    for (let i = 0; i < s.length; i++) {
      if (s[i] === "(") depth++;
      else if (s[i] === ")") depth--;
      else if (s[i] === sep && depth === 0) { out.push(s.slice(start, i)); start = i + 1; }
    }
    out.push(s.slice(start)); return out;
  };
  const easing = s => {
    if (["linear", "ease", "ease-in", "ease-out", "ease-in-out", "step-start", "step-end"].includes(s)) return 1;
    const open = s.indexOf("(");
    if (open < 0 || !s.endsWith(")")) return 0;
    const name = s.slice(0, open), inner = trim(s.slice(open + 1, -1));
    const args = inner ? split(inner, ",").map(trim) : [];
    if (name === "steps") {
      const count = /^\+?\d+$/.test(args[0] ?? "") ? Number(args[0]) : 0;
      const position = args[1] ?? "jump-end";
      // The kernel reads only the first two arguments of steps().
      return +(count > 0 && count <= 65535 && (position !== "jump-none" || count > 1)
        && ["jump-start", "start", "jump-end", "end", "jump-none", "jump-both"].includes(position));
    }
    if (name === "linear") {
      const stops = [];
      for (const arg of args) {
        const fields = trim(arg).split(/\p{White_Space}+/u);
        if (fields.length > 3 || !Number.isFinite(number(fields[0]))) return 0;
        if (fields.length === 1) stops.push(null);
        for (const field of fields.slice(1)) {
          const input = field.endsWith("%") ? number(field.slice(0, -1)) / 100 : NaN;
          if (!Number.isFinite(input)) return 0;
          stops.push(input);
        }
      }
      if (stops.length < 2 || stops.length > maxStops) return 0;
      stops[0] ??= 0; stops[stops.length - 1] ??= 1;
      let greatest = -Infinity;
      for (const input of stops) {
        if (input === null) continue;
        greatest = Math.max(greatest, input);
        if (greatest < 0 || greatest > 1) return 0;
      }
      // Omitted positions interpolate between these finite, ordered endpoints.
      return 1;
    }
    const n = args.map(number);
    if (!n.every(Number.isFinite)) return 0;
    if (name === "cubic-bezier") return +(n.length === 4 && n[0] >= 0 && n[0] <= 1 && n[2] >= 0 && n[2] <= 1);
    if (name === "-exact-spring") return n.length === 0 || (n.length === 3 && n[0] > 0 && n[1] >= 0 && n[2] > 0) ? 2 : 0;
    return 0;
  };
  const text = trim(String(v ?? ""));
  if (!text || text === "none") return false;
  const declarations = split(text, ",");
  if (declarations.length > maxTransitions) return false;
  let stacks = false;
  for (const declaration of declarations) {
    const parts = split(trim(declaration), " ").filter(Boolean);
    if (!parts.length || parts.length > 4) return false;
    let property = null, timing = 0;
    const times = [];
    for (const part of parts) {
      const time = part.endsWith("ms") ? number(part.slice(0, -2)) * .001
        : part.endsWith("s") ? number(part.slice(0, -1)) : NaN;
      if (Number.isFinite(time)) times.push(time);
      else if (Object.hasOwn(properties, part)) {
        if (property !== null) return false;
        property = part;
      } else {
        if (timing || !(timing = easing(part))) return false;
      }
    }
    const [duration = 0, delay = 0] = times;
    if (times.length > 2 || duration < 0 || (timing === 2 && (duration !== 0 || delay < 0))) return false;
    stacks ||= property === null || properties[property];
  }
  return stacks;
}
