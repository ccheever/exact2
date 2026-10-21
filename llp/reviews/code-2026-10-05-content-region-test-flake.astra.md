# Round 1
No concrete defects found in HEAD `5b7423fa0`.

- **Diagnosis is correct.** `test_service` waits only when acquiring the guard. Presenter destruction closes the session asynchronously, so another boot under that same guard needs its own wait.
- **`SERVICE.occupied()` is the correct condition.** It becomes false only after `run_session` returns, including worker and session cleanup ([worker.rs:145](/Users/admin/projects/exact2-wt-cr/host/linux/src/content_region/worker.rs:145)). Admission checks that same mutex-protected flag. With the test guard excluding competing admissions, retirement cannot cause the reported refusal after a false read.
- **No missing waits found** across the content-region tests. Repeated successful admissions now wait directly, through the swipe helper, or through existing occupancy loops. Intentional refusal tests remain intact.
- **The swipe helper is safe for its current callers.** Concurrent ordinary presenters use `region=false`; replacement explicitly drops the first presenter ([swipe_tests.rs:944](/Users/admin/projects/exact2-wt-cr/host/linux/src/presenter/swipe_tests.rs:944)). Loop-local presenters and pause guards drop before the next iteration. No caller waits while keeping an admitted first region alive.

The 1 ms sleep and 90 s watchdog preserve the retirement condition.

Validation: **45 relevant tests passed** using the existing test binary with eight test threads. No files modified.