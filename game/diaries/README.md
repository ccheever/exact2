# diaries — what building with it was like

The engine is judged as it is used (Charlie, 2026-09-17). Whoever builds a game or a
scene here — a person or a model — keeps a diary: what was asked, what was written,
every place the engine fought back, and what was changed in the *engine* because of
it. The same task is given to the twins (Godot 4, three.js) under the same brief, by
the same model, and the three diaries are scored side by side. A diary that flatters
is worthless; friction is the finding.

## An entry

`NNN-<task>-<engine>.md`: the brief (verbatim or linked); who built it and with what;
the wall time and the number of build/run iterations to a passing proof; lines of
game code and files touched; then the friction log — each item one line: what was
clumsy, what it cost, whether the engine changed.

## The score (each 0–5, higher is better; the scorer reads all three diaries blind to which is ours where that is possible)

| | what it asks |
|---|---|
| **Small** | lines, files and distinct engine concepts needed for the task |
| **Direct** | did the code say what the game does, or what the engine needs? boilerplate, ceremony, indirection |
| **Provable** | could the builder prove it works without looking at pixels? how much could be asserted, and how exactly? |
| **Repeatable** | same inputs, same result — across runs, across hosts, from a save |
| **Loop** | edit → verified, in seconds; what had to be waited for |
| **Feel** | frame pacing and input latency of the result, measured (game/bench) |
| **UI** | what the HUD, menus and text cost, and how good they are (layout, fonts, accessibility) |

A score below the twins' on any row is a queue line, not an argument.
