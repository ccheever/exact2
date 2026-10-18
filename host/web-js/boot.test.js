// The JS target's boot refusal names the step it stopped in and the exception's own words, cut, never the journal's
// last line (which the refused commit's own settlement writes past). Run alone, `bun test ./host/web-js/boot.test.js`:
// reorder.test.js mocks rt.js for its process.
import { test, expect, mock } from "bun:test";
import { createDocument } from "./dom.js";

// The build copies navigation.js and prompt-glue.js in from host/web and writes names.js.
mock.module("./navigation.js", () => import("../web/navigation.js"));
mock.module("./prompt-glue.js", () => ({}));
mock.module("./names.js", () => ({ sourceTypes: {} }));
globalThis.document = createDocument();
globalThis.location ??= { pathname: "/", search: "", href: "http://boot.invalid/", origin: "" };
const { mount, journal, say, Refusal } = await import("./rt.js");

test("a refused boot says its step and the exception's words, cut, and the journal keeps them whole", () => {
  const value = "x".repeat(1000);
  expect(() => mount(() => { say("a line the refusal is not"); throw new Refusal(`a write does not conform to its slot's type: "${value}"`); }))
    .toThrow(/^boot refused while building the view: a write does not conform to its slot's type: "x{254}… \(747 more characters in the journal\)$/);
  expect(journal.some(l => l.endsWith(`refused boot: a write does not conform to its slot's type: "${value}"`))).toBe(true);
});

test("a boot that throws names the exception's kind", () => {
  expect(() => mount(() => { throw new TypeError("table[name] is not a function"); }))
    .toThrow(/^boot refused while building the view: TypeError: table\[name\] is not a function$/);
});

test("a reason is cut between code points, and counted in them", () => {
  const astral = "😀".repeat(400);
  expect(() => mount(() => { throw new Refusal(astral); }))
    .toThrow(`boot refused while building the view: ${"😀".repeat(300)}… (100 more characters in the journal)`);
});
