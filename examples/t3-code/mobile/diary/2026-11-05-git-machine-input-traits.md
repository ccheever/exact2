# Git branch fields preserve machine-name input traits

## Rough

The mobile Git branch and worktree fields accepted branch identifiers as ordinary
text inputs. iOS could therefore capitalize or spellcheck a value that must be
sent to source control byte-for-byte.

## Lean in

The three branch and worktree inputs now disable capitalization, autocomplete and
spell checking. Their existing draft owners, labels and submit actions remain
unchanged, so the fix only aligns text entry with the machine-name semantics.

## Needed

A connected Git workspace drive can still confirm the keyboard traits on the
native sheet. This source change makes no claim about repository operations.

## Checkpoints

- Added a contract regression for all three machine-name fields.
- Focused and full mobile checks cover the declaration.
