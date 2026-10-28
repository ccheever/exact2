# Bake-off r29

exact2 0b27bed463. Exact over the other, per pair: minutes **1.75×**, tokens **2.69×** (geometric means, timeouts excluded). Preferred: Exact 5, other 10, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **13 of 15**, the other 14 of 15. Mean hitch ratio: Exact 0.9 ms/s, the other 1.2. Scrubbing (the scroll indicator dragged end to end and back, 14 pairs): Exact 20.5 ms/s, the other 18.3.

Scrub: hitch ms/s / longest ms while the indicator is dragged. Carousel: a horizontal list in the list, its hitch ms/s, and the diagonal drags' verdicts when either is not UIKit's.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel | Other hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.4 | 38 | 0 | 1/4 | 0 | 4.3 / 33 | – |
| recipes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.7 | 36 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| expenses | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.5 | 36 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.9 | 35 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| podcasts | swiftui | 1.5 | 41 | 0 | 1/4 | 0 | 149.0 / 54 | – | 1.9 | 45 | 0 | 0/4 | 0 | 222.3 / 72 | – |
| notes | uikit | 0.0 | 17 | 2 | 0/4 | 0 | 0.0 / 17 | – | 0.8 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| watchlist | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| chat | swiftui | 1.0 | 42 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.5 | 35 | 0 | 0/4 | 0 | 4.3 / 33 | – |
| moodboard | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| inbox | uikit | 1.3 | 48 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.9 | 45 | 0 | 0/4 | 0 | 8.1 / 43 | – |
| tasks | uikit | 6.1 | 61 | 0 | 1/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| settings | swiftui | 0.8 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.8 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| feed | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 138.0 / 67 | 0.0 | 2.9 | 39 | 0 | 0/4 | 0 | 17.0 / 34 | 0.0 |
| photos | uikit | 2.7 | 40 | 0 | 0/4 | – | not taken | – | 1.5 | 38 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| timetable | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 22 | 13 | 0/4 | 0 | 0.0 / 17 | – |

## The pairs

Means, Exact vs other: overall 7.0 vs 7.7; platform 7.2 vs 7.3; tells an app 5.5 vs 5.0; bugs an app 1.4 vs 1.0.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 22.1 | 21.4 | 6.50 | 7.01 | 6 | 8 | 5 | 7 | 7 / 6 | 5 / 2 | ok | smooth.stall tell, momentum.lift tell | swiftui |
| recipes | claude | uikit | 22.9 | 10.7 | 4.39 | 1.27 | 7 | 8 | 7 | 8 | 7 / 4 | 1 / 1 | ok | smooth.stall tell | uikit |
| expenses | gpt | uikit | 13.6 | 11.8 | 4.93 | 2.59 | 7 | 8 | 8 | 7 | 4 / 6 | 2 / 1 | ok | smooth.stall tell | uikit |
| workout-log | claude | swiftui | 19.3 | 8.4 | 2.59 | 1.20 | 7 | 8 | 8 | 8 | 3 / 3 | 0 / 1 | ok | smooth.stall tell | swiftui |
| podcasts | gpt | swiftui | 17.5 | 12.1 | 4.70 | 3.14 | 6 | 6 | 5 | 6 | 8 / 6 | 0 / 1 | smooth.stall tell, momentum.lift tell, blank.blank bad, scrub.hitch bad, scrub.stall tell, scrub.blank bad | smooth.stall tell, scrub.hitch bad, scrub.stall bad | Exact |
| notes | claude | uikit | 10.7 | 8.2 | 3.02 | 0.92 | 7 | 9 | 8 | 9 | 5 / 3 | 3 / 0 | momentum.jumps tell | ok | uikit |
| watchlist | gpt | uikit | 26.1 | 13.6 | 10.71 | 3.60 | 8 | 7 | 8 | 6 | 5 / 7 | 2 / 0 | ok | ok | Exact |
| chat | claude | swiftui | 20.8 | 6.2 | 4.70 | 0.85 | 7 | 8 | 7 | 8 | 6 / 3 | 0 / 1 | smooth.stall tell | smooth.stall tell, smooth.main tell, scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 30.2 | 15.9 | 9.77 | 4.61 | 7 | 6 | 7 | 5 | 6 / 7 | 1 / 2 | ok | ok | Exact |
| inbox | gpt | uikit | 18.1 | 11.3 | 6.34 | 2.09 | 9 | 7 | 9 | 6 | 3 / 8 | 0 / 2 | smooth.stall tell | smooth.stall tell, scrub.hitch tell, scrub.stall tell | Exact |
| tasks | claude | uikit | 28.0 | 12.3 | 13.50 | 2.02 | 6 | 9 | 6 | 9 | 7 / 3 | 1 / 0 | smooth.hitch tell, smooth.stall tell, momentum.lift tell | ok | uikit |
| settings | claude | swiftui | 15.2 | 10.8 | 3.64 | 2.43 | 7 | 9 | 8 | 9 | 6 / 3 | 1 / 0 | ok | ok | swiftui |
| feed | gpt | swiftui | 20.6 | 13.3 | 8.20 | 3.07 | 8 | 7 | 8 | 6 | 5 / 8 | 0 / 1 | scrub.hitch bad, scrub.stall tell | smooth.stall tell, scrub.hitch bad, tracking.glide bad, momentum.tau bad, momentum.handoff bad | Exact |
| photos | claude | uikit | 19.5 | 8.0 | 7.53 | 1.65 | 7 | 8 | 7 | 9 | 5 / 3 | 1 / 1 | smooth.stall tell, tracking.glide bad | smooth.stall tell, scrollToTop.reached tell | uikit |
| timetable | gpt | uikit | 20.4 | 12.5 | 7.96 | 2.42 | 6 | 7 | 7 | 7 | 5 / 5 | 4 / 2 | ok | momentum.jumps bad, scrollToTop.reached tell | uikit |
