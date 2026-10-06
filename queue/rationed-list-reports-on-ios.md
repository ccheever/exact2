**Rationed list reports on iOS** (2026-09-19; LLP 1010 §6): UIKit also scrolls on
the thread that lays out; `PresenterIOS` still reports a whole window inside the
scroll callback and paints text when first seen. The runner and host halves are
shared (`list_viewport_within`, one-call settle); the presenter half is not made.

*Filed under “Next, in order (2026-08-29)”.*
