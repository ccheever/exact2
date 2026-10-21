# Recipe: a first-launch screen

A welcome shown until the person taps Continue, then never again. The main screen is the
root route; while a `persist` flag is unset, that route shows the welcome instead, as a
UIKit app swaps its window's root view controller (SwiftUI: an `if` over `@AppStorage`).

```contract
routes nav
  habits "/"

component App
  state welcomed = false persist
  state habits = ["Drink water", "Take a walk"]
  action begin
    welcomed = true
  action follow(url: string)
    nav = go(nav, url)
  view
    main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow width="100%" height="100%"
      for e in stack(nav) key=e.id
        if e.name == "habits"
          if welcomed
            column navigationKey=`${e.id}` position="absolute" inset=0
              header padding="8px 16px"
                text "Habits" role="heading" aria-level=1
              list appearance="auto" listStyle="inset-grouped" flex=1 min-height=0 testId="habits"
                section
                  for h in habits key=h
                    row
                      text h
          else
            column navigationKey=`${e.id}` navigationBar="none" position="absolute" inset=0
              scroll flex=1 min-height=0
                column min-height="100%" box-sizing="border-box" padding=24 gap=16
                  justify-content="center" text-align="center"
                  text "Build better habits" role="heading" aria-level=1 font="-exact-large-title"
                  text "Small steps add up." color="-exact-secondary-label"
              button press=begin buttonStyle="filled" margin=24 testId="continue"
                text "Continue"
```

```contract-test
test "the welcome shows until Continue, then never again"
  expect tree missing "habits"
  tap "continue"
  expect tree missing "continue"
  expect tree has "habits"
  reload
  expect tree missing "continue"
  expect tree has "habits"
```

- **Not a route of its own.** With a welcome at `/` and the list at `/habits`,
  `replace(nav, path("habits"))` is refused at launch: a stack's first visit keeps its
  root route (`lower-replace-root`). `push` puts the list over the welcome, with a Back.
- **No move after launch.** A `task` that leaves the welcome is a timer, too late: the
  first frame already shows the welcome, at every launch. A `persist` state is read before
  the first frame, so a relaunch opens on the list. On the web that holds for a route
  rendered in the browser, the default: one rendered ahead (`render=build`) cannot read it.
- On iOS, Continue swaps the screen in place, as `setViewControllers(_:animated:)` does
  without animation: the list has its large title and bar and nothing beneath it, so no
  Back, edge swipe or VoiceOver escape returns. `navigationBar="none"` gives the welcome no bar; its headline
  is content, and Continue stays below its scroll.
- Keep it one short screen ("fast, fun, and optional", HIG Onboarding); a tutorial
  someone may want again belongs in Settings too.
