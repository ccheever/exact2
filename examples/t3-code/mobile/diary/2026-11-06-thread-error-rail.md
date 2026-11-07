# Thread error banner rail clearance

## Rough

The transcript rows and composer already reserved the iPad Duo's right-side
occlusion, but a thread error banner was still laid out to the route edge. A
long provider error could therefore sit beneath the native vertical tab rail.

## Lean in

ThreadFeed now passes its measured trailing reservation to ThreadErrorBanner.
The banner's outer trailing margin includes that reservation, keeping its
border and actions within the same content column as transcript rows.

## Needed

A Duo drive with a visible provider error remains useful for pixel acceptance.
The source change preserves zero additional margin on compact phones.

## Checkpoints

- Scroll-contract coverage asserts the banner consumes the route reservation.
- Focused Contract and thread tests remain to be run after this change.
