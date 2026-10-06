# Code review: what the gate does not test (AGENTS.md), 2026-10-05 (grok)

- **Family:** xAI — `~/.grok/bin/grok -m grok-4.7 --reasoning-effort xhigh --always-approve --no-subagents --output-format plain --prompt-file <brief>`, headless, a detached worktree at `dd04cc87b`.
- **Method:** one brief (sha256 `53d08501820627eeafe9ae388a8e27bcbf2050efbdeba6e6d2438057aced5c85`), the same sent to both; one round, sized to a five-line documentation change; blind to the other review. Requested by the coordinator. The author (Claude) is not a reviewer.
- **Transcription:** stdout, unedited; the first sentences are progress narration.
- **Verdict:** LAND WITH FIXES.
- **Disposition** (the landing commit):
  1. Taken: the gate tests only `default-members`; non-members are built only as a dependency; the list names `js/`, `gpu/` and most apps beside the hosts.
  2. Taken: the dataset test is described as failing when the web host writes a `data-*` name `contract/lower` does not reserve.
  3. Taken: `--bins` for a bin-only crate such as `exact-web-js` (no `src/lib.rs`).
  4. Taken: its own bullet, after the `contract-difftest` advice, marked advice, not a check.

---

I'll review only commit `dd04cc87b` against the facts you listed and the repo's own rules, without editing anything.The commit is five lines in `AGENTS.md`. Next I’ll check those claims against the gate, `default-members`, and the rules on adding apparatus.**LAND WITH FIXES**

A documentation rule is the right fix. Commit `794073089` kept the blocking gate near 80s (down from about 236s) by leaving hosts, GPU, Hermes, platform shells, and stress fixtures out of `default-members`; putting the hosts back on that gate fights the 60s blocking budget in `rules/RULES.md`. These five lines are not apparatus: `rules/RULES.md` names a check, script, registry, config file, or design doc, and `ccheever` co-authored the commit. `exact-web`, `exact-web-js`, `exact-linux`, and `exact-apple` are real packages outside the gate. The async-lane timing is right: `scripts/async.mjs` runs the same Cargo commands with `--workspace` on each first-parent commit of `origin/main` and files a failure only then. The new sentences still misstate scope, the test, and the command.

1. **Major** — `AGENTS.md:51`. "The gate never builds a member outside `default-members` (the hosts: …)" is false on both sides. The gate compiles non-members it depends on: `exact-gpu-reflect` (`bake` → `gpu/reflect`), `messages-stress-data` (`exact-live-data`), and, because the build is `--all-targets`, the dev-dependency `exact-data-host` (`calendar-data` → `data/host`). It does not run their tests. The 116 non-default members are not "the hosts": they include `js/`, `gpu/`, and most apps. `exact-raster`, `exact-svg-raster`, and `exact-web-js-motion` are default members, so the gate does build and test them. The four named packages are outside and are not in that dependency closure. Edit: "The gate selects only `default-members`. It still compiles a non-member it depends on (`exact-gpu-reflect`, `exact-data-host`, `messages-stress-data`) and does not test it. Hosts it does not select (`exact-web`, `exact-web-js`, `exact-linux`, `exact-apple`, `exact-windows`, `exact-render`, …) are neither compiled nor tested."

2. **Major** — `AGENTS.md:53`. The parenthetical reverses the test. `host::element::dataset_tests::every_data_name_the_host_writes_is_a_reserved_word` (`host/web/src/element.rs:1269`) fails when the host writes a `data-*` name that `contract_lower::dataset::reserved` rejects. Extra reserved words stay green. `contract/lower/src/dataset.rs:17-21` says that test keeps the host-written half whole, including names from `host/web-js`, which this test does not itself enumerate. Edit: "`exact-web`'s dataset test reads `contract/lower`'s reserved `data-*` words and fails when the host writes one that is not reserved."

3. **Major** — `AGENTS.md:55`. The command is the right `exact-web` invocation (lib tests hold the dataset test; that crate has no bin) and is written as the command for every host just named. `exact-web-js` has no `src/lib.rs`; its tests are in the bin (`src/main.rs`, `src/code.rs`), so `--lib` does not run them. Edit: "for that package, for example `cargo test -p exact-web --lib --tests --no-fail-fast`. A bin-only package is `cargo test -p exact-web-js --bins --no-fail-fast`."

4. **Minor** — `AGENTS.md:51`. The obligation is buried in the five-checks bullet, after the tier-2 aside, so it reads as part of the gate. Length is fine. Move it to its own bullet beside the `contract-difftest` "before landing" note (`AGENTS.md:95`) and call it advice, not a check, as that note does. `AGENTS.md` does not bind (`AGENTS.md:7`).
