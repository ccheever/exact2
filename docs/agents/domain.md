# Domain documentation

Use a single context: an optional root `GLOSSARY.md` and `docs/adr/`.

Before exploring an area, read the glossary if it exists and any relevant ADRs.
If they are absent, proceed silently. Create them only when actual terminology
or decisions need recording through domain modeling.

Use glossary terms consistently in code, tickets, tests and proposals. Call out
a conflict with an existing decision and explain what requires reconsideration.

Exact's `rules/RULES.md` and `rules/DEFERRED.md` remain binding. Consult the
relevant LLPs through `llp/1000-exact2-root.explainer.md`. Keep existing app-local
decisions in `examples/t3-code/mobile/docs/`; do not move or duplicate them
into ADRs. A new glossary or ADR does not grant framework scope or approval.
