# Rules

Speed is the constraint. Everything below exists to keep the edit-to-verified loop
under a minute, and to stop this repo accreting the apparatus that made its
predecessor unshippable.

Each rule is **[check]** (enforced in CI) or **[review]** (enforced by a human).

## Budgets — every budget is a trade, not a limit

- **5 blocking checks, 60s total.** A sixth requires deleting one, same PR. **[check]**
- **20 active design docs.** A 21st requires archiving one. **[check]**
- **This file: 700 words.** If it grows, something becomes a check or stops being a rule. **[check]**
- **1,500 lines per source file.** Generated files are exempt — and generated files are built, not committed. **[check]**

A limit gets exceptions. A trade doesn't: it forces someone to name what matters less.

## Loop shape

- **Every check reports all failures in one run.** Fail-fast is banned. A check that
  surfaces one defect per run turns N bugs into N x runtime — that pattern alone cost
  the old repo 89 hours on a single work package. **[check]**
- **Nothing blocks on anything slower than 60 seconds.** Slow verification runs
  asynchronously, per-commit, on fleet hardware, attributed to the breaking commit.
  Run everything; block on almost nothing. **[review]**
- **Fix loops get 3 rounds.** Then stop and either escalate to a human decision or
  descope the target. Never "iterate until green." **[review]**

## Time budgets

Tracked every commit. A regression is a P0 with a name on it.

| | |
|---|---|
| Touch one line, rebuild that crate | 30s |
| Test what you changed | 60s |
| Blocking gate suite | 60s |
| Full build, warm cache | 5 min |
| Dev restart, request to present | 100ms p50 |
| Cold start to interactive first frame | 100ms p50 |
| App JS executed before first pixel | none |

## Scope

- **`NOT-DOING.md` is binding.** Moving something onto the doing-list means writing why
  and taking something off. **[review]**
- **Delete; don't deprecate.** No compat shims, no migration paths, no legacy branches
  before 1.0. **[review]**
- **Web is the dev loop; native is swept.** One Contract source targets all four
  surfaces. Verify on the seconds-loop and let the minutes-loop run behind you. **[review]**
- **The first frame is data, not code.** No app JS runs before first pixel. Startup is
  emergent from every module's boot behavior and cannot be retrofitted, so the boot graph
  carries a budget from commit one — counted, not timed. **[check]**
- **Modules ship as bytecode. Nothing is compiled at runtime** — not source strings
  compiled per module at boot, not a transpile step in the loader, not a fallback. The old
  repo transpiles 5.47MB of ESM to CommonJS across 570 modules on every launch. **[check]**
- **A spec needs an implementer and a date, or it isn't written.** Specifying something
  you are about to build is transcription and costs nothing. Specifying something nobody
  is assigned to build is how a corpus reaches 8.2M words. **[review]**

## Agents

- **Agents remove apparatus freely and add none.** An agent PR that adds a check,
  script, registry, config file, or design doc needs explicit human approval saying so.
  This asymmetry is the whole game: agents produce governance faster than humans can
  read it. 675 checks and 450 scripts got there one reasonable addition at a time. **[review]**
- **The old repo is research, never authority.** Cite it for how something worked.
  Never to block. **[review]**

## Fleet

- Warm, pre-provisioned worktrees, one lane each. Never clone fresh.
- **Never `git stash`** — the stash stack is global across worktrees and will apply
  another lane's work into yours. Use a temporary commit.
- Kill only PIDs you recorded at launch. Never by name or pattern.

## The five checks

`build` · `test` · `lint` · `caps` · `boot`

`caps` enforces the budgets above. `boot` counts the module graph reachable before first
pixel and fails when it grows — a count, so it cannot flake the way a timer does. It takes
the last slot; a sixth check now costs one of these five.

Everything else runs async.
