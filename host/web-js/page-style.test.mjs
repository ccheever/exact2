import { expect, test } from 'bun:test';
import { analyze, select, tokens } from './page-style.mjs';

// The same sheet and documents as page_style.rs's test: the two renderers select alike (LLP 1048.007 D4).
const CSS = ':root{--a:red;--b:var(--a);--c:blue;--d:green}p{color:var(--b)}@media (hover:hover){:where(html:not([data-x])){button:hover{color:var(--c)}dialog{color:red}}}.c9{x:1}';

test('a page keeps the rules its markup can meet, and the custom properties they read', () => {
  const rules = analyze(CSS);
  expect(JSON.stringify(rules)).toBe('[["",":root{--a:red;--b:var(--a);--c:blue;--d:green}",[[]]],["","p{color:var(--b)}",[["t:p"]]],["@media (hover:hover){:where(html:not([data-x])){","button:hover{color:var(--c)}",[["t:button"]]],["@media (hover:hover){:where(html:not([data-x])){","dialog{color:red}",[["t:dialog"]]],["",".c9{x:1}",[["c:c9"]]]]');
  expect(select(rules, tokens('<p title="a>b">x</p><button>y</button>'))).toBe(':root{--a:red;--b:var(--a);--c:blue}p{color:var(--b)}@media (hover:hover){:where(html:not([data-x])){button:hover{color:var(--c)}}}');
  expect(select(rules, tokens('<p style="margin:var(--d)">x</p>'))).toBe(':root{--a:red;--b:var(--a);--d:green}p{color:var(--b)}');
  expect(select(rules, tokens('<b class="c9">'))).toBe('.c9{x:1}');
});
