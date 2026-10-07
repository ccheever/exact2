# Cross-browser conformance: Firefox and WebKit deliver a chord's modifiers as separate keys

**Status:** Open
**Systems:** scripts/agent-playwright.mjs, scripts/agent-keys.mjs, scripts/agent.mjs
**Author:** Claude (Opus 5.5), calibrating cross-browser conformance (lane/xbrowser)
**Date:** 2026-10-07

A drive's key chord (`type "field" key "Shift+Enter"`, `"Control+s"`, `"Meta+s"`, `"Shift+Tab"`) reaches the page differently by engine:

- **Chrome** (CDP `Input.dispatchKeyEvent`) sends one keydown and keyup for the key, with the modifiers as bits on it. A `key` handler hears `Enter`.
- **Firefox and WebKit** (Playwright's `keyboard.press('Shift+Enter')`) press each modifier as its own key first. A `key` handler hears `Shift`, then `Enter`.

So synthetic-keys' and synthetic-tabindex's `heard` logs diverge at the first chord (`… Tab;Shift;Tab;` against `… Tab;Tab;`), and every later step's state and tree differ with them. Most of the remaining cross-browser state and tree lines are this.

A person's keyboard does press Shift as its own key, so Chrome's delivery is the less physical one. Two fixes:

1. Chrome's carrier delivers the modifier's own keydown and keyup around the key, as a keyboard does (and as Playwright does). This changes every Chrome drive's key log, including apps' tests that count keys.
2. The Firefox and WebKit carriers dispatch the chord as one key with modifier state. Playwright has no API for that, so it would mean synthetic, untrusted events, which this carrier avoids.

Proposed: (1), checked against the tests and recipes that read key logs. It needs the agent's owner to agree, since it changes what Chrome drives report.
