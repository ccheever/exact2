# C2 Apple narrow re-check n1 (rebased onto field 16edf171d4: icon_name through exact_runner::icon_of), Astra (gpt-6-astra, max)

**RIGHT.**

- [icon_path.rs:84](/tmp/iosfix-rv-c2n1/host/apple/src/icon_path.rs:84): `icon_of(...) == Some(icon)` correctly matches even-odd and nonzero rows and skips rejected rows (`None`). [Icon equality](/tmp/iosfix-rv-c2n1/kernel/src/replaced.rs:152) includes `even_odd`.
- No other Apple reads of an icons row’s raw path remain.
- [icon_path_readers.rs:22](/tmp/iosfix-rv-c2n1/contract/cli/tests/it/icon_path_readers.rs:22): the guard still scans Apple, has no `icon_name` exception, and rejects unauthorized `raw_for_icon` calls. It is unchanged from `16edf171d4`.
- Tests still directly exercise `icon_name`: [ordinary matching and aliases](/tmp/iosfix-rv-c2n1/host/apple/src/icon_path.rs:103), and [identical paths with different fill rules](/tmp/iosfix-rv-c2n1/host/apple/src/icon_path.rs:240). They are unchanged by the rebase.

The range-diff matches the intended delta. Tests were inspected, not rerun: this read-only worktree has no built test binaries.
