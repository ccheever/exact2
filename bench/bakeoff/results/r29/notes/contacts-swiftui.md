# Contacts build notes

Finished and installed as **Contacts** (`dev.bakeoff.contacts.b699760`) on simulator `EF198926-1958-489D-A7A4-9196E454DD3A`. The final app is launched in light mode at the normal text size. No other simulator was used.

All requested features are implemented. The original 320-person JSON is bundled unchanged in `Resources/contacts.json`. Changes are written atomically to the app’s Application Support directory, in `Contacts/contacts-v1.json`. An empty saved directory stays empty on relaunch; unreadable saved data is reported rather than silently replaced.

The hardest parts were the alphabetical index and keyboard behavior at the largest accessibility text size. Screenshot review caught overlapping decorative icons, cramped summary text, and an email field that could receive focus beneath the keyboard. The final layout reserves room for the keyboard’s Done control and scrolls the focused editor row into view. Dark mode uses brighter accent colors and darker avatars.

I read the skeleton README and local command help for simulator appearance and content-size controls. No external libraries or web research were needed. Presentation choices left open by the spec: favorites appear in a horizontal section and also remain in the alphabetical directory; contacts without a last name sort under their first name. Search ignores case and accents, matches across words, and accepts phone digits without punctuation. Editing preserves existing birthdays and favorite status.

Validation completed:

- Model checks: bundled data, sorting, search, blank-name rejection, first-name-only contacts, Unicode names, add/edit/favorite/delete persistence, birthday preservation, empty storage, corrupt storage, and failed writes.
- Final normal-size simulator suite: **6 tests, 0 failures** (`build/ReleaseChecks.xcresult`).
- Final dark-mode suite at accessibility extra-extra-extra-large: **1 test, 0 failures** (`build/FinalAccessibility.xcresult`), including an assertion that the email field stays above the keyboard accessory.
- Screenshots reviewed in `build/screenshots/` and `build/screenshots-dark-accessibility/`. Disposable test contacts were deleted and the initial favorite state restored.

Nothing in the specification remains unfinished. Actual telephone calls and email delivery cannot be verified on the simulator; the app uses standard phone/mail URLs and offers an explanatory alert and copy actions when no handler is available.

A starter UI-test target and documented simulator interaction helper would have made this faster. Swift compiler macros and simulator services also required running the build/test commands outside the shell sandbox in this environment.
