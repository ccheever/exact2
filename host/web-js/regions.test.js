// A bound length's layout regions as the browser's safe area (LLP 1123 D10). `bun test ./host/web-js/regions.test.js`.
import { test, expect } from "bun:test";
import { regions } from "./regions.js";

test("each region is its edge's safe area, inside calculations and behind a fallback", () => {
  expect(regions("calc(env(-exact-layout-margins-inset-left) + 7px)")).toBe("calc(env(safe-area-inset-left) + 7px)");
  expect(regions("max(16px, env( -exact-readable-content-corners-inset-left, 30px))")).toBe("max(16px, env( safe-area-inset-left, 30px))");
  expect(regions("calc(1px /* \" url( */ + env(/**/-exact-layout-margins-inset-left, 30px))")).toBe("calc(1px /* \" url( */ + env(/**/safe-area-inset-left, 30px))");
  for (const region of ["safe-area-corners", "layout-margins", "layout-margins-corners", "readable-content", "readable-content-corners"]) {
    for (const edge of ["top", "right", "bottom", "left"]) expect(regions(`env(-exact-${region}-inset-${edge})`)).toBe(`env(safe-area-inset-${edge})`);
  }
});

test("escaped names are read decoded, as CSS tokenizes them", () => {
  expect(regions("max(16px, env(-exact-\\6c ayout-margins-inset-left, 8px))")).toBe("max(16px, env(safe-area-inset-left, 8px))");
  expect(regions("e\\6e v(-exact-layout-margins-inset-left, 30px)")).toBe("e\\6e v(safe-area-inset-left, 30px)");
  expect(regions("calc(E\\NV(-exact-\\73 afe-area-corners-inset-\\74op) + 1px)")).toBe("calc(E\\NV(safe-area-inset-top) + 1px)");
  // An escape ends at one whitespace character; CR LF, CR and form feed are read as a newline first
  for (const [end, read] of [[" ", " "], ["\t", "\t"], ["\n", "\n"], ["\r", "\n"], ["\f", "\n"], ["\r\n", "\n"]]) {
    expect(regions(`calc(e\\6e${end}v(-exact-layout-margins-inset-left) + 7px)`)).toBe(`calc(e\\6e${read}v(safe-area-inset-left) + 7px)`);
  }
});

test("only env()'s first argument is renamed", () => {
  for (const text of ['"env -exact-layout-margins-inset-left", serif', "'-exact-layout-margins-inset-left'", '"env \\\r\n-exact-layout-margins-inset-left", serif',
    "env -exact-layout-margins-inset-left, serif", "env, é-exact-layout-margins-inset-left, serif", "url(-exact-layout-margins-inset-left)",
    "url( a/-exact-layout-margins-inset-left.png )", "u\\72 l(-exact-layout-margins-inset-left)", 'url("-exact-layout-margins-inset-left")',
    "env(safe-area-inset-left, -exact-layout-margins-inset-left)", "env(-exact-layout-margins-inset-middle)", "env(x-exact-layout-margins-inset-left)",
    "var(--exact-layout-margins-inset-left)", "env(-exact-\\e9 -inset-left)", "16px", "\\31 0px", '"font \\61\nenv(-exact-layout-margins-inset-left)", serif',
    "url(a\\29\nenv(-exact-layout-margins-inset-left))",
    // Not functions, as CSS tokenizes them: a hash, an at-keyword, a dimension, CDO before a name, NUL (U+FFFD) in a name
    "#env(-exact-layout-margins-inset-left)", "@env(-exact-layout-margins-inset-left)", "1\\65nv(-exact-layout-margins-inset-left)",
    "<!--url(%\tenv(-exact-layout-margins-inset-left)", "\0env(-exact-layout-margins-inset-left)", "env(-exact-layout-margins-inset-left\0)",
    // A backslash at the end is an escape (U+FFFD), part of the name
    "env(-exact-layout-margins-inset-left\\"]) expect(regions(text)).toBe(text.replace(/\r\n?/g, "\n"));
  expect(regions("-->env(-exact-layout-margins-inset-left)")).toBe("-->env(safe-area-inset-left)");
  expect(regions('url( "a)-exact-layout-margins-inset-left" ) env(-exact-layout-margins-inset-left)')).toBe('url( "a)-exact-layout-margins-inset-left" ) env(safe-area-inset-left)');
  expect(regions("url(x) env(-exact-layout-margins-inset-left)")).toBe("url(x) env(safe-area-inset-left)");
});
