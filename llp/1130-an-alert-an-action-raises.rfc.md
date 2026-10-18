# LLP 1130: An alert an action raises

**Type:** RFC
**Status:** Draft r11, 2026-10-18; to be built once it passes review (James, 2026-10-18: "We can build this if it is correct and follows Exact's principles and passes rigorous review")
**Systems:** Contract (one host command), the runner, the alert on every host that presents dialogs, the agent, docs
**Author:** Claude (Opus 5.5) for ide
**Related:** #579 (a dialog's `open` is refused), LLP 1101.001 P5 (`showModal(id)` and `close(id)` from an action), `kernel/src/confirmation.rs` (the alert shape and its words), LLP 1115 (author > platform > CSS default)

## 1. The problem

An app often has to tell the person that something went wrong, and wait for them to acknowledge it: "Couldn't save your changes", the reason, and OK. Writing it as `dialog … open=(error != "")`, with the host presenting a dialog whose `open` is true as an alert, gives HTML's attribute a meaning it does not have: `open` shows a dialog non-modally, and the specification advises against removing it by hand ("it is generally better to never remove the `open` attribute manually", [HTML §4.11.4](https://html.spec.whatwg.org/multipage/interactive-elements.html#the-dialog-element)). Exact refuses `open` on a dialog (#579).

A dialog opened by `showModal(id)` does not fit either. A message with one OK is not the alert shape every host presents, which has exactly one cancel (`confirmation::is_alert`), and the compiler wants an alertdialog to have an action. And the app must keep the dialog's presence in step with its state: two failures close together, a `close` that arrives after a newer error, a route pushed away while the alert shows. A declarative attribute in place of `open` would need answers to the same cases, and more.

HTML has a call for exactly this: [`alert(message)`](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#dom-alert), a message the person acknowledges, owned by the browser rather than the page.

## 2. Decisions

**D1. `alert(message)` from an action.** An action calls `alert(message)`, as it calls `showModal(id)`, and the host shows the platform's alert with one button, its OK:
- `UIAlertController` on iOS, iPadOS and tvOS, and `NSAlert` on macOS;
- the alert Linux's presenter shows for `showModal`: Android's Material window on a shell, and the neutral look's alert on desktop Linux, DRM and Windows;
- the terminal host's top layer, as it shows a modal dialog: the title, the message and an OK it focuses;
- on the web, an alert dialog of the host's own (D6), since the browser's `alert()` may not show at all ("If we cannot show simple dialogs for this, then return"), shows one text with no title, and stalls the page's tasks while it shows ([HTML §8.9.1](https://html.spec.whatwg.org/multipage/timers-and-user-prompts.html#simple-dialogs)).

**D2. Its words.** Line breaks in the message are made LF (a CR or a CRLF is one). The text before the first LF is the title, and the text after it is the message, blank lines kept; with no LF the whole is the title. So `alert("Couldn't save your changes\n" + reason)` shows the title "Couldn't save your changes" over the reason. A title or a message that is empty or only white space is left out. An alert with neither (`""`, `"\n"`, `" \n\n"`) is refused at compile time when the argument is a literal, and journaled and dropped when it is computed. However long the message, OK stays on screen: where the host draws the alert itself (the web, Linux's presenter, the terminal), the message scrolls inside it.

**D3. Nothing comes back.** The alert belongs to the host, not to the page: it has no element, no state and no event, and the app's state does not have to follow it. An app that must act on the person's answer asks with a dialog it opens (`showModal(id)`), as now. An error the app stored before a launch raises nothing by itself: the app raises it from the action that finds it.

**D4. One at a time, over what the session presents.**
- Alerts show one at a time, in the order they were raised, over the page and its sheets. Raising one does not wait: the action goes on, and the alert shows when its turn comes.
- An alert shows when the host can present it over the topmost presentation. It waits while another alert, a confirmation or a menu is open or a presentation is opening or closing, and, where the host presents in windows (Apple, the web, Android, desktop Linux and Windows), until the session has one in the foreground, so at launch and in the background. Where the host presents without a window (the neutral look's alerts on DRM and in a headless drive, the terminal's top layer), it shows at once. One the platform does not show (Android with no window) waits for the next window.
- While it shows, a confirmation or a menu the page opens is refused with a journal line, so that nothing takes the alert's place before it is answered: a person answers one thing at a time, and nothing waits that could outlive the element that asked.
- It is the session's, not a route's or a sheet's: navigation does not withdraw it, and if what it shows over goes (a sheet the page withdraws, whose dismissal takes the alert with it on iOS), it has not been answered, and it shows again over what remains once the transitions end.

**D5. Where `showModal` is refused, so is `alert`.** A host or mode that refuses `showModal` (Linux's presenter with neither a shell window nor the neutral look's alerts) refuses `alert` the same way, with a journal line naming its title.

**D6. Accessibility.** Each host exposes the alert to its screen reader as its own alerts are: the title and the message read when it shows, and the focus on OK.
- On Android, whose painted windows get their accessibility content from the reader's tree, the host adds the alert's.
- Desktop Linux and Windows have no screen-reader bridge (`host/linux/src/presenter/access.rs`: no AT-SPI tree; the Windows host paints its window and supplies no UI Automation provider), so the alert is not announced there, as nothing else is.
- The web host's alert follows the WAI alert dialog pattern ([APG](https://www.w3.org/WAI/ARIA/apg/patterns/alertdialog/)): `role="alertdialog"`, named by its title (or by its message when it has none) and described by its message; what is behind it is inert, and the focus is on OK. When it closes, the focus goes back where it was if that element can still take it and is in the topmost modal that remains (a sheet, or the page), else to the first element there that can.

**D7. The agent.** `tree` shows the alert that is ready for input, and `tap` presses its OK. `clock settle` waits for alerts to finish opening or closing, never for one to be answered.

## 3. Stages

1. Contract and the runner (`alert` among the host commands, with D2's words), the web host and the agent, with red/green tests, and the command's line in `rules/DEFERRED.md`.
2. The Apple hosts, then Linux's presenter (Android, desktop Linux, DRM and Windows), then the terminal host, each with its suite and a drive of two alerts raised together, one raised at launch, one raised over a sheet the page then withdraws, and one raised in the background.
