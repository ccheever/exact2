You are grading two iPhone apps built from the same specification, A and B, blind: you are not told how either was
built, and must not try to find out (do not inspect bundles, binaries or file systems). Judge only what a person
holding the phone would see and feel. You run unattended; nobody will answer questions.

Both apps are installed on the iOS simulator `{SIM}`, which is yours alone. App A is `{A}`, app B is `{B}`.
Drive them with the driver below; take screenshots into `./shots/` (name them `A-…png`, `B-…png`) and look at them.

{DRIVE}

# What to do

1. Read the specification at the end. For each numbered requirement, try it in both apps, including the edge cases
   it names. Relaunch to check persistence where the spec asks for it.
2. Look at every screen in light mode, in dark mode (`appearance dark`), and at the largest text size
   (`textsize largest`); put both back afterwards (`appearance light`, `textsize default`).
3. Scroll every long list: drag slowly, fling it, fling back to the top, pull past the top. Note anything that does not
   feel like an iPhone list: jumps, blank rows, rows popping in, stutter, scroll position lost on returning to a screen.
4. List **tells**: each thing someone who knows iOS would notice is not as a hand-built UIKit or SwiftUI app on iOS 27
   would do it (a control drawn by hand where iOS has one, wrong metrics or colours, missing system behaviour such as
   swipe actions, large titles, keyboard handling, haptics-free toggles, text that truncates or overlaps at large
   text, a web-like look). Syllable hyphenation and a truncated large title at the largest size are excused; a break
   inside a word or number is not.
5. List **bugs**: anything that does not work as the spec says or crashes.

# Output

Write `grade.json` in your working directory, exactly this shape:

```json
{
  "A": { "requirements": [{"n": 1, "status": "pass|partial|fail", "note": "…"}],
         "bugs": ["…"], "tells": ["…"], "scroll": "one or two sentences on list scrolling feel",
         "scores": {"features": 1-10, "platform": 1-10, "polish": 1-10, "overall": 1-10} },
  "B": { …the same… },
  "better": "A" | "B" | "tie",
  "why": "two or three sentences"
}
```

`platform` is how indistinguishable it is from a hand-built iPhone app; `overall` is which you would rather use and
ship. Be concrete and consistent: the same flaw costs the same in both apps.

# Specification

{SPEC}
