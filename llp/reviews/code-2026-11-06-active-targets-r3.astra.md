# Active targets: Astra review, round 3

- **Family:** OpenAI. GPT-6 Astra (`gpt-6-astra`) via `codex-cli 0.167.0`, reasoning effort max, read-only sandbox, a throwaway worktree at `c8c0c85ca7` on `70365218ab`.
- **Method:** one brief (sha256 `13b1de7ddf2167cc46af95bd8c9a5f758ce1daedc098b0a3f0ec6d5f962f2973`), one reviewer, with four passes: adversarial correctness, simplification, writing and verification. The author is not a reviewer.
- **Transcription:** the final message, unedited; its links name the review's evidence paths and its commits name that frozen source.

---

**Clean code review: no new material code findings** in `c8c0c85ca7698f7e1736f1638578e7de320b8c4d` against `70365218ab`. I completed separate adversarial correctness, simplification, writing, and verification passes. Validation is not fully green.

I agree with every technical settlement from R1 and R2:

| Settlement | Assessment |
|---|---|
| R1: grouped UIKit rows ignored active paint | **Agree, resolved.** Native configuration updates apply authored feedback while retaining UIKit content and selection. |
| R1: a first target appearing during a stationary press remained invisible | **Agree, resolved.** Web, Apple—including AppKit tables—and Linux retain the contact before paint exists and refresh its presentation. |
| R2: web scale remained stale | **Agree, resolved.** Refreshes re-aim scale for target appearance, changes and removal, including held presses and CSSOM/token/sheet updates. |
| R2: plain UIKit rows retained highlight and hidden separators | **Agree, resolved.** The shared eligibility predicate removes obsolete feedback, preserves selection, and clears the reverse card-to-row dim. |
| R2: trailing accessories overwrote leading-symbol ownership | **Agree, resolved.** Ownership follows the symbol used by the content configuration; hidden images and trailing accessories cannot overwrite it. |
| Linux activation and cancellation | **Agree.** Feedback changes preserve the activation route chosen at down, and cancellation clears feedback. |

I also agree with the prior conclusions on precedence, token routing, explicit native-button admission, AppKit handling, simplification and writing. Public-name retirement and border/shadow support remain separate work.

Independent verification passed:

- Four new compiler regressions and 25 native-button tests, using the existing binary after checking its provenance and matching changed Rust sources.
- All five focused Linux cases, using the frozen binary whose SHA-256 matches the supplied record.
- A bounded JavaScript probe executing the unchanged feedback block: R2 fails; the candidate passes scale appearance, retargeting, clearing, token/sheet refresh, release cleanup and retargeting during an ease. This used DOM/animation stubs, not browser pixels.

The supplied final evidence supports the 23 Chrome/WebKit press cases, affected UIKit suites, web Rust tests, build, clippy, workspace, formatting and boot. The iOS smoke now passes. The macOS selected test run has no failures but skips one window-focus test; tvOS compilation passes.

The remaining validation limits are material:

- The [R3 root run](/tmp/exact-active-r3-test.log) was still incomplete at my last read and had already failed `four_times_the_uses_compile_in_about_four_times_as_long`: **5.0× CPU time for 4× the uses**, exceeding 4.6×. No matching main control establishes its attribution.
- The previous complete root run failed the Apple held-write timeout and SVG wall-time bound. Their isolated main-source controls passed; those full failures were **not reproduced on main**.
- I supersede R2’s unresolved Linux source attribution with the later same-checkout controls: both network failures persist with main’s production sources restored. Full Linux remains non-green; the image natural-size failure occurs on both main and candidate.
- The [new web smoke](/tmp/exact-active-r3-web-smoke.log) passes authored tests and fails the same frame URL assertion as pure main. Caps still reports the existing `foldables.md` recipe violation; source-file limits pass.

No source changes, Firefox execution, GUI launches, fresh Cargo/Swift builds, or main-source swaps were performed during this review.