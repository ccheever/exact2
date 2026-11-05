# Agent resources

Each section below is one way for an agent to read or use these docs, with the command, URL or header it needs.

## Start prompt

Give your agent one file before it builds an app with Exact. The home page has a copy button for this prompt:

```text
{{START_PROMPT}}
```

For a website (pages served to a browser), the guide is [Start here: a website with Exact](../../start-here-site.md) in place of [Start here: building an app with Exact](../../start-here.md). Everything else in the docs is lookup.

## Markdown pages

Add `.md` to a page's URL to get the page as Markdown: `/recipes/money` is `/recipes/money.md`, and the home page is `/index.md`. Each page's HTML names its Markdown with `<link rel="alternate" type="text/markdown">`.

Each Markdown page starts with frontmatter (`title`, `description` and `modificationDate`) and a pointer to `/llms.txt`. Its links go to other Markdown pages, or to the file on GitHub.

## llms.txt

`/llms.txt` lists every page with its title, its Markdown URL and a one-line description. On a host that reads the site's `_headers` file, such as Cloudflare Pages or Netlify, each response names it in two headers: `Link: </llms.txt>; rel="llms-txt"` and `X-Llms-Txt: /llms.txt`.

## sitemap.md

`/sitemap.md` lists the same pages by section, nested as the guides nest. Each line has the page's Markdown URL, its type (Guide, How-to, Reference), the date it last changed, a summary, and its prerequisites: the page to read before it.

## Open in your agent

Each page has these actions, at the top of the page and in the rail beside it:

- **Copy Markdown:** copies the page as Markdown.
- **View as Markdown:** opens the page's `.md` in a new tab.
- **Claude, ChatGPT and Cursor:** open a new chat that reads the page.
- **Claude Code and Codex:** open the app with a prompt that reads the page.

## Terminal lookups

An app that `bun scripts/exact.mjs new` makes has an `AGENTS.md` and a `CLAUDE.md` that tell its agent where to look. From the app's folder, these commands answer without the website:

```sh
bun exact.mjs docs <heading words>      # a section of a guide
bun exact.mjs contract vocab <name>     # a tag or property
bun exact.mjs pattern                   # the patterns; pattern <name> writes one, tested
bun exact.mjs help                      # every verb
```

These guides are for lookup by section, not for reading front to back:

- [Contract for agents](../../contract-for-agents.md)
- [Agent pitfalls](../../agent-pitfalls.md)
- [Contract grammar](../../contract-grammar.md)
- [Reference](../../reference.md)

## Feedback

Each Markdown page has a feedback block below its title: a command and an HTTP request that report an error on that page. In an app's folder, the agent can also record what went wrong:

```sh
bun exact.mjs feedback note "<what happened>"
```

`bun exact.mjs feedback status` says whether notes are sent. `EXACT_FEEDBACK=0` keeps them on the machine.
