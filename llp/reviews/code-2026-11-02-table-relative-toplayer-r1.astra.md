# Code review: a popover table row's relative position, round 1 (astra)

- **Family:** OpenAI. GPT-6 Astra, `codex exec -m gpt-6-astra -c model_reasoning_effort="xhigh" -s read-only --ephemeral`, a throwaway worktree at `dc2d098ac5`.
- **Method:** one brief (sha256 `047cabb4791e63e49a31444b94e8f15254d2d97b919d81b9cdc74370c747b2db`), shared by both reviewers, blind to the other review. The author (Claude, fb1) is not a reviewer.
- **Transcription:** the final message, unedited.
- **Response:** nothing to address.

No BLOCKER, MAJOR, or MINOR findings.

- [table.rs:192](/Users/admin/x2scratch/fb1-rv-astra/kernel/src/layout/table.rs:192): The accessor is correct here. `position` is non-inherited; authored relative values retain precedence. The platform position start belongs to neutral route headers, not table rows/groups. Authored sticky returns `Sticky`, so it skips relative offsets both before and after this change.
- [table.rs:665](/Users/admin/x2scratch/fb1-rv-astra/kernel/tests/it/table.rs:665): The fixture meaningfully exercises kernel top-layer layout. It creates a semantic `tr`, sets the string-valued `Popover` property, and recomputes with a definite viewport and no closed-popover cover. That activates the implied viewport block without a host open command. Assertions cover the row, cell, cell descendant, and unaffected sibling; the existing test covers ordinary row/group offsets.
- The change removes only the erroneous descendant-relative translation. Top-layer conversion to absolute matches [CSS Positioned Layout §3.1](https://www.w3.org/TR/css-position-4/#top-layer-styling). Insets remain available for placement through [top_layer_origin](/Users/admin/x2scratch/fb1-rv-astra/kernel/src/kernel/block.rs:449) and Apple’s [PositionArea.inset](/Users/admin/x2scratch/fb1-rv-astra/host/apple/Sources/ExactKit/PositionArea.swift:342). Swift’s `NodeView.layoutPosition` already matches the kernel accessor; no additional host change is needed for this fix. No remaining direct position read was found in table publication.

Validation: Cargo could not run because the sandbox denied creating its target directory. Bun’s two repository-position checks passed; its scanner self-test failed solely because temporary-directory creation was denied. No files were changed.

VERDICT: APPROVE
