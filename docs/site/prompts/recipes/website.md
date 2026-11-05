```prompt Build a website
Build a website with Exact in this folder: pages served to a browser. Use https://github.com/ccheever/exact2/blob/main/docs/recipes/website.md as the source for each step. The exact2 checkout has the same page at docs/recipes/website.md, and docs/start-here-site.md is the guide for a website.

1. Find the exact2 checkout (the folder with scripts/exact.mjs); if you cannot find it, ask me where it is. If this folder is not an Exact app (it has no exact.mjs and app.contract), make one: `bun <exact2>/scripts/exact.mjs new <name>`, then `cd <name>`.
2. If I did not say which pages the site has and where each page's data comes from (a list in app.ts, a live API, or one reader's data behind a sign-in), ask me before you write code.
3. Run `bun exact.mjs pattern site` and start from it: a list built ahead, a page per item rendered for each request, and an API's origin as a setting.
4. Give each route the `render` policy that fits its data, as the recipe's table says: `render=build` for what the build has, `render=request` for a live API's data, `render=cached` for an answer every reader may share for a while, and `render=client` (the default) for one reader's own data.
5. Read an API's origin as a setting: `"env": ["API_BASE_URL"]` in app.json, `import.meta.env.API_BASE_URL` in app.ts, and its value in .env. The value is public on every host: never put a secret there.
6. Use the tags as the HTML elements they name: `main`, `nav aria-label`, `header` and `footer` beside `main`, `section aria-labelledby`, headings in order, and a list of links or cards as a `column role="list"` of `role="listitem"`s.
7. In app.test.contract, add a test for each page, and one that opens a deep link with `at "/<path>#<id>"`.
8. Run `bun exact.mjs test web`, then `bun exact.mjs verify` until it passes. Fix the app, not the test. If a finding stays after three rounds of fixes, or the app had it before you changed anything, record it (step 9) and report it; do not keep trying.
9. When a doc was wrong or missing, or a step needed a workaround, record it: `bun exact.mjs feedback note "<what happened>"`. Do not send the notes; tell me how many you recorded.
10. Tell me each page and its render policy, what the tests and verify reported, and how to see the site: `bun exact.mjs web-build --out dist`, then `bun exact.mjs serve dist`. Start the API's stand-in first (`bun server.ts`), or set `API_BASE_URL` to an API that answers.
```
