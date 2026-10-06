**The keyboard's blink on a field hand-off** (iOS): UIKit rebuilds the accessory bar
when first responder moves between two `UITextField`s — one bar-less frame (308 →
335), reproduced in a from-scratch two-field app with any traits and by re-traiting
one field in place; a phone shows it faintly. Safari does not, so WebKit hides it
somewhere below the responder (its `UITextInput`-conforming content view and its own
input-assistant handling). Worth finding out how, if a form ever needs it gone
(LLP 1008 §9).

*Filed under “Declared gaps, by system (each spec's "Not in v1")”.*
