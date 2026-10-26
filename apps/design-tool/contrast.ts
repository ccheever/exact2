// The contrast ratio the tool shows beside a pair of colours (LLP 1122).

/** WCAG 2's contrast ratio of a text colour `fg` over a background `bg`,
 * each `#rrggbb[aa]`: the text composited over the background first. A
 * background that is not opaque has no one ratio until what is behind it
 * is known: said, as is anything that is not such a colour. */
export function contrast(fg: string, bg: string): number | string {
  const rgba = (hex: string) => {
    const m = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(hex.trim());
    return m ? [0, 2, 4].map(i => parseInt(m[1].slice(i, i + 2), 16) / 255).concat(m[2] ? parseInt(m[2], 16) / 255 : 1) : null;
  };
  const [f, b] = [rgba(fg), rgba(bg)];
  if (!f || !b) return 'not a colour';
  if (b[3] < 1) return 'translucent background';
  const over = f.slice(0, 3).map((c, i) => c * f[3] + b[i] * (1 - f[3]));
  const lum = (c: number[]) => {
    const [r, g, bl] = c.map(v => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [x, y] = [lum(over), lum(b.slice(0, 3))];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}
