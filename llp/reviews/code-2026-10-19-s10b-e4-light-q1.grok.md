# Code review: LLP 1121 stage 10b E4, the one light mode (the live Figma test), quick check 1, 2026-10-19 (grok)

- **Family:** xAI. Grok 4.7 via `grok`, reasoning effort xhigh, no subagents, Edit and Write denied, a throwaway worktree at `b199702127`.
- **Method:** one brief (sha256 `1b1674c981f2ee53078506dca7d2d6eb3472fb76138583c7f4ffdc36cf9276af`), shared by both reviewers, blind to the other review. The author (Claude, rt1) is not a reviewer.
- **Transcription:** the final message, unedited.

I'll review this commit read-only: the collapse of light contexts, then whether pull, push, conflicts, check, and write treat those twins the same way.The commit is small. Next I’ll read pull, light twins, push, and the plugin paths that consume those contexts.The fixture’s two light sides match. Next I’ll trace a four-mode file re-pulled under the new plan, and a push where those light values differ.Nothing blocks. The collapse matches the export header, and pull, push, check, write, and conflicts all treat that one light mode as the value for every light context it stands for. `bun test tools/figma-tamber/src/` — 49 passed.

**1. The collapse is right.**

Light contexts whose header entry has `scheme: "light"` and the same `modes` once every axis with `under === "dark"` is removed share one mode. Dark contexts stay separate. In the theme fixture that is `dark-style=black, light` and `dark-style=dim, light` (the header axis is `dark-style`, `under: "dark"`).

The kept context is the first of that set after the export's default context is sorted first. It is never a twin. Here the default is `dark-style=black, light`, so that context is kept and the mode list starts with it. `also.light` is `["dark-style=dim, light"]`. Reordering the context keys does not change the kept context while the default stays put. If the default is a dark context, that dark mode is first and the kept light context is the first light one in the remaining order (the exporter's order is the axis-default light).

The label is the remaining axes as `name=value`, then `light`. With only an under-dark axis it is `light`. With another axis it is `density=tight, light`.

An export with no header, no `axes`, or no header `contexts` does not collapse: four modes, no `also`. A light context missing from the header stays its own mode. `under` must be the string `"dark"` (what the exporter writes). `"scheme=dark"` does not collapse.

More than one axis works. Every `under: "dark"` axis is stripped together. Light contexts that still differ by another axis stay separate modes. With `density` ordinary and both `dark-style` and `contrast` under dark, the result is two light modes (`density=tight, light` and `density=loose, light`), each standing for the four light contexts of that density, and eight distinct dark modes.

The 40-character cut runs on that label, or on the full context when there is no label. The name is at most 40 characters; a collision becomes `… N`, still 40. `also` is keyed by that same mode name. The full context string stays on `contexts` and in `also`.

**2. They agree.**

Pull stores one Figma value, taken from the kept context, and records the others on `also`. Write makes one mode. `contextIds` maps only the kept context to that mode id, and the stored plan keeps `also`. Holding reads that one mode. Push writes its value to the kept context and to every twin. A light edit becomes `#ff0000` on both `dark-style=black, light` and `dark-style=dim, light`; `dark-style=dim, dark` stays as it was. Conflicts and check look at that same mode, so one edit is one conflict (`light` / `dark-style=black, light`). `contextsMoved` counts `also`, so a re-pull of the same export reports no added or removed context.

The fixture's two light contexts match on all 9 tokens, values and extensions. An unedited push leaves both at `#1d9bf0`.

A file pulled as the old four modes (the axis treated as ordinary) re-pulls cleanly when nothing was edited: check reports no conflicts and no context change, the `dark-style=black, light` mode id is kept and renamed `light`, `dark-style=dim, light` is removed, and `also` records it. An unedited push still has `#1d9bf0` on both light contexts. An edit that was only on the removed mode is a conflict on that mode before the pull; accepting the pull takes the design's value and both light contexts stay `#1d9bf0`.

The split layout still has no `also`. A file already split stays split while the context set is unchanged, including a 3-mode file. A fresh pull at that limit is the one collection of three modes.

**3. Notes, none of them blockers.**

- Push writes the light mode onto a twin whenever the twin's stored value differs, including when Figma was not edited, and conflicts does not list that twin. A checked design has one light side, and this fixture does, so that write does not happen. A divergent export would be flattened to the kept context.
- If the old file's Figma default mode is `dark-style=dim, light`, that mode cannot be removed, so the re-pull parks it as `unused: dark-style=dim, light` while `also` still stands for that context. Nothing is deleted. Every later check lists every variable on that parked mode as revived, because the context is still in the export. The ordinary pull, whose default is the kept light mode, does not do this.
