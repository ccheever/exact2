# Bake-off r26

exact2 9d23a92da4. Exact over the other, per pair: minutes **2.25×**, tokens **3.18×** (geometric means, timeouts excluded). Preferred: Exact 5, other 10, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **14 of 15**, the other 10 of 15. Mean hitch ratio: Exact 1.3 ms/s, the other 2.0. Scrubbing (the scroll indicator dragged end to end and back, 15 pairs): Exact 45.0 ms/s, the other 68.3.

Scrub: hitch ms/s / longest ms while the indicator is dragged. Carousel: a horizontal list in the list, its hitch ms/s, and the diagonal drags' verdicts when either is not UIKit's.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel | Other hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 5.6 | 99 | 0 | 1/4 | 0 | 178.6 / 87 | – |
| recipes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| expenses | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.8 | 36 | 0 | 0/4 | 2 | 65.7 / 47 | – |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.3 | 45 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| podcasts | swiftui | 3.3 | 43 | 0 | 0/4 | 0 | 332.7 / 231 | – | 13.5 | 228 | 5 | 0/4 | 0 | 412.5 / 115 | – |
| notes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.5 | 38 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| watchlist | uikit | 10.1 | 39 | 2 | 1/4 | 0 | 23.6 / 46 | – | 0.0 | 17 | 0 | 0/4 | 0 | 4.3 / 33 | – |
| chat | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 79.6 / 71 | – | 1.6 | 35 | 2 | 0/4 | 0 | 4.2 / 33 | – |
| moodboard | swiftui | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| inbox | uikit | 3.6 | 55 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.9 | 35 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| tasks | uikit | 1.2 | 45 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| settings | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.7 | 42 | 2 | 0/4 | 0 | 0.0 / 17 | – |
| feed | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 238.8 / 115 | 0.0 | 2.5 | 42 | 4 | 1/4 | 0 | 358.8 / 124 | 0.0 |
| photos | uikit | 0.0 | 17 | 0 | 0/4 | 1 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | – | 0.0 / 17 | – |
| timetable | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |

## The pairs

Means, Exact vs other: overall 6.9 vs 7.9; platform 7.5 vs 7.2; tells an app 5.3 vs 4.5; bugs an app 1.5 vs 1.1.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 43.5 | 18.0 | 16.61 | 3.68 | 5 | 8 | 6 | 6 | 6 / 7 | 3 / 1 | ok | smooth.hitch tell, smooth.stall bad, momentum.lift tell, scrub.hitch bad, scrub.stall bad | swiftui |
| recipes | claude | uikit | 24.8 | 10.0 | 2.97 | 1.60 | 7 | 8 | 7 | 7 | 4 / 4 | 1 / 2 | ok | ok | uikit |
| expenses | gpt | uikit | 34.1 | 17.9 | 8.48 | 3.54 | 7 | 9 | 7 | 8 | 6 / 4 | 1 / 0 | ok | smooth.stall tell, tracking.lag bad, scrub.hitch bad, scrub.stall tell | uikit |
| workout-log | claude | swiftui | 21.9 | 9.3 | 4.15 | 1.24 | 7 | 9 | 8 | 9 | 4 / 2 | 2 / 1 | ok | smooth.stall tell | swiftui |
| podcasts | gpt | swiftui | 16.3 | 23.9 | 4.30 | 4.51 | 6 | 9 | 7 | 7 | 6 / 6 | 2 / 0 | smooth.stall tell, blank.blank bad, scrub.hitch bad, scrub.stall bad, scrub.blank bad | smooth.hitch bad, smooth.stall bad, momentum.jumps bad, scrub.hitch bad, scrub.stall bad | swiftui |
| notes | claude | uikit | 31.9 | 6.7 | 3.96 | 1.10 | 6 | 9 | 7 | 9 | 8 / 2 | 2 / 0 | ok | smooth.stall tell | uikit |
| watchlist | gpt | uikit | 32.5 | 12.8 | 10.99 | 1.59 | 8 | 7 | 8 | 6 | 5 / 7 | 3 / 2 | smooth.hitch bad, smooth.stall tell, momentum.lift tell, momentum.jumps tell, scrub.hitch bad, scrub.stall tell | ok | Exact |
| chat | claude | swiftui | 29.1 | 7.5 | 5.70 | 1.70 | 7 | 8 | 7 | 8 | 5 / 3 | 1 / 0 | scrollToTop.reached tell, scrub.hitch bad, scrub.stall bad | smooth.stall tell, momentum.jumps tell, scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 28.4 | 24.4 | 9.52 | 6.37 | 8 | 6 | 8 | 5 | 6 / 6 | 1 / 2 | ok | ok | Exact |
| inbox | gpt | uikit | 30.7 | 12.1 | 9.45 | 2.12 | 9 | 6 | 9 | 5 | 3 / 7 | 1 / 3 | smooth.stall tell | smooth.stall tell, rubberBand.ratio tell | Exact |
| tasks | claude | uikit | 27.1 | 9.3 | 10.19 | 1.70 | 4 | 9 | 7 | 9 | 5 / 3 | 3 / 0 | smooth.stall tell | ok | uikit |
| settings | claude | swiftui | 12.7 | 6.2 | 2.38 | 0.94 | 8 | 9 | 8 | 9 | 4 / 3 | 1 / 1 | ok | smooth.stall tell, momentum.jumps tell | swiftui |
| feed | gpt | swiftui | 22.2 | 13.7 | 5.58 | 2.39 | 8 | 6 | 8 | 6 | 4 / 5 | 0 / 1 | blank.blank bad, scrub.hitch bad, scrub.stall bad, scrub.blank bad | smooth.stall tell, momentum.lift tell, momentum.jumps bad, scrub.hitch bad, scrub.stall bad, tracking.glide bad, momentum.tau bad, momentum.handoff bad | Exact |
| photos | claude | uikit | 22.1 | 8.4 | 8.53 | 1.38 | 7 | 9 | 7 | 9 | 8 / 2 | 0 / 0 | tracking.ratio bad, tracking.lag tell | tracking.glide bad, scrollToTop.reached tell | uikit |
| timetable | gpt | uikit | 43.3 | 14.6 | 12.72 | 2.88 | 7 | 6 | 8 | 5 | 6 / 7 | 2 / 3 | ok | ok | Exact |
