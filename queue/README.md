# Queue

What would make sense to do next, one file per entry, so two lanes adding or
deleting entries never touch the same file. This is not a spec and decides nothing:
`rules/RULES.md` §Scope says a spec without an implementer and a date isn't written,
so an entry is one to three lines — the thing, why, what it needs — with its date and
where it came from. When someone picks one up it becomes an LLP with a name and a date
(archive a link when the working set is full); when it lands, delete the file. Anyone,
agents included, may add or remove an entry. `rules/DEFERRED.md` still binds: an entry
that sits on that list carries the trade it would take.

Name a new entry by what it is, in kebab case (`linux-autofocus-dispatches-no-focus.md`),
and write links relative to this directory (`../llp/…`). There is no index to keep: `ls
queue` lists the entries, `head -qn1 queue/*.md` their first lines, and `git log
--diff-filter=A --name-only -- queue` the newest first. An entry migrated from the old
single file ends with the heading it sat under there.
