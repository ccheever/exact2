Investigate JavaScript storage unload/write ordering: the workspace run observed `cancel` where `unload_invalidates_continuations_and_configuration_survives_reload` expected `again` (`js/tests/storage.rs:210`, 2026-09-10). Its isolated six-test suite and complete `--no-fail-fast` workspace rerun passed; the cause remains unproven. Unload invalidates the continuation, but the underlying write may already be in flight; establish that ordering before changing its guarantee.

*Filed under “Later”.*
