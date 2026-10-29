# Contacts build notes

- Built with Exact, using its list-detail pattern and native UIKit search, navigation, indexed plain lists, modal forms, grouped detail rows, alert confirmation, and swipe deletion.
- All 320 supplied records are bundled by importing `data/contacts.json` into the data module. User changes are stored atomically in the app's private `contacts-v1.json`; an absent file starts from the bundled records. An empty saved directory stays empty.
- Favorite entries also remain in their alphabetic sections. First-name-only contacts use their first initial; names that begin with a nonletter use `#`. Edits preserve existing birthdays and favorite status. The form permits duplicate names, since different people can have the same name.
- The main difficulty was learning Exact's component boundary rules: a form's navigation properties must live on the same root as its header. Each edit form now initializes from the contact identified in its route, so it can reopen correctly after a relaunch.
- Lookups: the Exact start guide plus vocabulary for search, swipe deletion, grouped forms, edit sheets, file grants, app manifests, navigation, symbols, and simulator driving.
- The initial native build took several minutes. Subsequent packaged builds reused the runtime and took seconds.
- Web behavior tests pass. Web accessibility probing reported offscreen swipe-row clipping after its automated scrolling, despite a direct large-text check reporting no findings. The separate formal oracle also reports unsupported string methods and boot divergences with synthetic data. Those are recorded separately from actual iPhone behavior checks.
- Faster: a directory starter with bundled seeds, persisted CRUD, route-based editing, and example accessibility tests would have reduced iteration. Clearer stale-build messaging when assets or native declarations change would help too.

## Final validation and delivery

- Built, installed, and launched **Contacts**, bundle ID `dev.bakeoff.contacts.b79ced8`, on simulator `486FD498-0E3E-4B64-8E4B-152BD8265EE7`. No other simulator was used.
- Final iPhone verification: **7 passed, 0 failed**; light/dark checks had no findings; all nine checked screens fit at the largest accessibility text size (body 53 pt); scrolling reported **like UIKit**. The native verification phase reported `verify: clean in 194.1 s`. Timing comparisons were limited by high machine load; scrolling physics were checked.
- Final web behavioral tests: **7 passed, 0 failed**.
- Real-touch simulator smoke test exercised name and phone editing with the software keyboard, cancellation, live search, and opening contact details.
- No requested app features remain unfinished. The separate formal-oracle phase appended by Exact still exits nonzero with synthetic-data boot divergences and unsupported `toLowerCase`; this does not change the passing native behavior/layout results above. Formal proof is therefore not claimed. Its receipts were left unchanged.
- Relaunch persistence tests explicitly reopen a contact after iOS returns to the root directory, then check the saved name, company, favorite status, and deletions.
- The app was finally launched through `bun exact.mjs ios --run` with the specified `EXACT_SIM` value. The finished package remains installed.
