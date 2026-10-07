// The admitted backdrop grammar (kernel/src/style/backdrop.rs), linked only
// for a dynamic backdrop row. A rejected value clears the row on every host.
export function backdropValue(value, note = () => {}) {
  if (value == null) return null;
  const refuse = why => { note(why); return null; };
  const expected = "`backdrop-filter` is `none` or one `blur(<length>)` and/or `saturate(<number-or-percentage>)` (LLP 1053.000 D1)";
  if (typeof value !== "string") return refuse(expected);
  const text = value.trim();
  if (/^unset$/i.test(text)) return null;
  if (/^none$/i.test(text)) return "none";
  const number = "[+-]?(?:[0-9]*\\.[0-9]+|[0-9]+)(?:[eE][+-]?[0-9]+)?";
  const argument = new RegExp(`^(${number})(px|%)?$`, "i");
  const seen = new Set(), out = [];
  let rest = text;
  while (rest) {
    const m = /^([a-z-]+)\(([^)]*)\)/i.exec(rest);
    if (!m) return refuse(expected);
    const name = m[1].toLowerCase(), arg = m[2].trim();
    rest = rest.slice(m[0].length).trimStart();
    if (name !== "blur" && name !== "saturate") {
      if (!/^(brightness|contrast|grayscale|hue-rotate|invert|opacity|sepia|drop-shadow|url)$/.test(name)) return refuse(expected);
      const suffix = name === "url" ? "; an SVG filter on a backdrop is not built" : "";
      return refuse(`\`${name}()\` is CSS, but exact2's \`backdrop-filter\` builds only \`blur()\` and \`saturate()\`${suffix} (LLP 1053.000 D1)`);
    }
    if (seen.has(name)) return refuse(`\`backdrop-filter\` takes one \`${name}()\` in exact2 (LLP 1053.000 D1)`);
    seen.add(name);
    const match = arg ? argument.exec(arg) : null;
    const invalid = name === "blur" ? "`blur()` takes a length in px (unitless zero)" : "`saturate()` takes a nonnegative number or percentage";
    if (arg && !match) return refuse(invalid);
    const unit = match?.[2]?.toLowerCase(), raw = match ? Number(match[1]) : name === "blur" ? 0 : 1;
    const amount = name === "saturate" && unit === "%" ? raw / 100 : raw;
    if (!Number.isFinite(raw)) return refuse(invalid);
    if (name === "blur" && (Math.abs(raw) > 3.4028234663852886e38 || unit === "%" || (!unit && arg && /[1-9]/.test(match[1].split(/[eE]/)[0])))) return refuse(invalid);
    if (name === "saturate" && unit === "px") return refuse(invalid);
    if (!Number.isFinite(Math.fround(amount)) || amount < 0) return refuse(name === "blur" ? "`blur()` takes a nonnegative length" : "`saturate()` takes a finite nonnegative number or percentage");
    out.push(`${name}(${amount}${name === "blur" ? "px" : ""})`);
  }
  return out.length ? out.join(" ") : refuse(expected);
}
