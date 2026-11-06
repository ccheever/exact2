# Thread error disclosure parity

task: align the mobile error details control with the reference transcript

### Rough

The mobile error banner used a shorter “Show details” control with a smaller
blue label. The reference calls the action “Show full error” and “Show less”,
and keeps it as a muted inline disclosure below the clamped message.

### Lean in

The error text column now owns its alignment, while the disclosure gets the
reference four-point separation, 14-point type, medium weight and muted color.
The expanded and collapsed labels now match the reference wording.

### Needed

A native screenshot of a long provider error is still useful for pixel review.
This source change makes no native reachability or performance claim.

### Checkpoints

- Contract regression checks the labels, spacing and muted treatment.
- Thread error and scroll-focused tests pass after the change.
