// `formatTime`, `formatDate` and `formatNumber` on the JS target: the runner's
// `en-US` strings (runner/src/stdlib.rs `format_time`, runner/src/format.rs),
// checked against the same Intl oracle rows (runner/tests/it/format.rs).
/** `wall_ms` (runner/src/stdlib.rs): the instant clipped, then shifted by the
 * offset; null for a non-finite input, an offset past ±18 h, or a wall time
 * outside years 1–9999, which every format entry prints as "". */
const wall = (ms, off) => {
  if (!Number.isFinite(ms) || !Number.isFinite(off) || Math.abs(off) > 1080) return null;
  const w = Math.trunc(ms) + off * 60000;
  return w >= -62135596800000 && w <= 253402300799999 ? w : null;
};
/** `h:mm AM` of (epoch ms, UTC offset minutes east), U+0020 before the period. */
export function x_formatTime(ms, off) {
  const w = wall(ms, off);
  if (w === null) return "";
  const m = Math.floor((((w % 864e5) + 864e5) % 864e5) / 6e4), h = m / 60 | 0;
  return `${h % 12 || 12}:${String(m % 60).padStart(2, "0")} ${h < 12 ? "AM" : "PM"}`;
}
// `formatDate` and `formatNumber`, runner/src/format.rs's `en-US` strings.
const MONTHS = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
/** `Sep 26, 2026` ("medium") or `September 2026` ("month-year"): Hinnant's `civil_from_days`. */
export function x_formatDate(ms, off, style) {
  const w = wall(ms, off);
  if (w === null) return "";
  const z = Math.floor(w / 864e5) + 719468, era = Math.floor(z / 146097), doe = z - era * 146097;
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100)), mp = Math.floor((5 * doy + 2) / 153);
  const day = doy - Math.floor((153 * mp + 2) / 5) + 1, month = mp < 10 ? mp + 3 : mp - 9, year = yoe + era * 400 + (month <= 2 ? 1 : 0);
  const name = MONTHS[month - 1];
  return style === "month-year" ? `${name} ${year}` : `${name.slice(0, 3)} ${day}, ${year}`;
}
/** A non-negative number's shortest round-trip digits, never exponential (Rust's `{}`). */
function plain(x) {
  const s = String(x), e = s.indexOf("e");
  if (e < 0) return s;
  const [i, f = ""] = s.slice(0, e).split("."), digits = i + f, point = i.length + Number(s.slice(e + 1));
  if (point <= 0) return "0." + "0".repeat(-point) + digits;
  return point >= digits.length ? digits + "0".repeat(point - digits.length) : digits.slice(0, point) + "." + digits.slice(point);
}
/** `Intl.NumberFormat("en-US", { notation: "compact", roundingMode: "trunc", signDisplay: "negative" })`: `1.2K`, `0.29`, `10,000T`. */
export function x_formatNumber(n) {
  if (!Number.isFinite(n)) return "";
  if (n === 0) return "0";
  let [int, frac = ""] = plain(Math.abs(n)).split(".");
  if (int === "0") int = "";
  const scale = int.length >= 4 ? Math.min((int.length - 1) / 3 | 0, 4) : 0, cut = int.length - 3 * scale;
  frac = int.slice(cut) + frac;
  int = int.slice(0, cut);
  let kept = "";
  if (!int.length) { let first = 0; while (first < frac.length && frac[first] === "0") first++; kept = frac.slice(0, first + 2); }
  else if (int.length === 1) kept = frac.slice(0, 1);
  kept = kept.replace(/0+$/, "");
  const grouped = !int ? "0" : int.length >= 5 ? int.replace(/\B(?=(\d{3})+$)/g, ",") : int;
  return (n < 0 ? "-" : "") + grouped + (kept ? "." + kept : "") + (scale ? "KMBT"[scale - 1] : "");
}
