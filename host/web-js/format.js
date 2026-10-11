// `formatTime`, `formatDate`, `formatNumber`, `toFixed` and `formatDecimal` on the JS target: the runner's
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
/** `Sep 26, 2026` ("medium"), `September 2026` ("month-year") or `2026-09-26` ("iso"): Hinnant's `civil_from_days`. */
export function x_formatDate(ms, off, style) {
  const w = wall(ms, off);
  if (w === null) return "";
  const z = Math.floor(w / 864e5) + 719468, era = Math.floor(z / 146097), doe = z - era * 146097;
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100)), mp = Math.floor((5 * doy + 2) / 153);
  const day = doy - Math.floor((153 * mp + 2) / 5) + 1, month = mp < 10 ? mp + 3 : mp - 9, year = yoe + era * 400 + (month <= 2 ? 1 : 0);
  const name = MONTHS[month - 1], pad = (n, w) => String(n).padStart(w, "0");
  // "iso" (LLP 1102 §3.4): the date part of `toISOString`; `wall` admits years 1–9999 only, so four digits.
  if (style === "iso") return `${pad(year, 4)}-${pad(month, 2)}-${pad(day, 2)}`;
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
// LLP 1116 D8: each code's en-US symbol and fraction digits (runner/src/format.rs `CURRENCIES`).
const CURRENCIES = { USD: "$2", EUR: "€2", GBP: "£2", JPY: "¥0", CNY: "CN¥2", INR: "₹2", CAD: "CA$2", AUD: "A$2", NZD: "NZ$2",
  HKD: "HK$2", SGD: "SGD2", CHF: "CHF2", SEK: "SEK2", NOK: "NOK2", DKK: "DKK2", PLN: "PLN2", MXN: "MX$2", BRL: "R$2", KRW: "₩0",
  ZAR: "ZAR2", TWD: "NT$2", ILS: "₪2", PHP: "₱2", VND: "₫0" };
/** `decimal`, `currency` and `percent` (runner/src/format.rs `styled`): `Intl.NumberFormat("en-US")`'s, from
 * `String(n)`'s shortest digits cut half away from zero, grouped by threes, a negative zero signed. */
function styled(n, style, code) {
  let sym = "", lo = 0, hi = 3, shift = 0;
  if (style === "percent") { hi = 0; shift = 2; }
  if (style === "currency") {
    const c = CURRENCIES[code] ?? "";
    sym = c.slice(0, -1); lo = hi = +c.slice(-1);
    if (/[A-Z]$/.test(sym)) sym += "\u00a0";
  }
  let [int, frac = ""] = plain(Math.abs(n)).split(".");
  const cut = shift + hi, up = frac[cut] >= "5";
  let ds = int + frac.padEnd(cut, "0").slice(0, cut);
  if (up) { const k = ds.search(/9*$/); ds = (k ? ds.slice(0, k - 1) + (+ds[k - 1] + 1) : "1") + "0".repeat(ds.length - k); }
  int = ds.slice(0, ds.length - hi).replace(/^0+(?=\d)/, "");
  frac = ds.slice(ds.length - hi).replace(/0+$/, "").padEnd(lo, "0");
  return (n < 0 || Object.is(n, -0) ? "-" : "") + sym + int.replace(/\B(?=(\d{3})+$)/g, ",") + (frac ? "." + frac : "") + (style === "percent" ? "%" : "");
}
/** `compact` is `Intl.NumberFormat("en-US", { notation: "compact", roundingMode: "trunc", signDisplay: "negative" })`:
 * `1.2K`, `0.29`, `10,000T`; the other styles are `styled`'s. */
export function x_formatNumber(n, style, code) {
  if (!Number.isFinite(n)) return "";
  if (style !== "compact") return styled(n, style, code);
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
/** `toFixed(n, digits)` (LLP 1102 §3.2): JavaScript's own, except a non-finite `n` prints "" (D7), not `NaN`/`Infinity`. */
export const x_toFixed = (n, d) => Number.isFinite(n) ? n.toFixed(d) : "";
/** `formatDecimal(units, digits)`: an integer count of a smallest unit with `digits` places, exactly (a BigInt of it);
 * `-0` is `0`, and a count that is not an integer, or not finite, is "". */
export function x_formatDecimal(u, d) {
  if (!Number.isInteger(u)) return "";
  const s = BigInt(Math.abs(u)).toString().padStart(d + 1, "0");
  return (u < 0 ? "-" : "") + (d ? s.slice(0, -d) + "." + s.slice(-d) : s);
}
