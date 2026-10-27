# Bake-off r28

exact2 d7ed471d11. Exact over the other, per pair: minutes **1.87×**, tokens **3.13×** (geometric means, timeouts excluded). Preferred: Exact 3, other 12, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **15 of 15**, the other 11 of 15. Mean hitch ratio: Exact 0.5 ms/s, the other 0.9. Scrubbing (the scroll indicator dragged end to end and back, 13 pairs): Exact 10.8 ms/s, the other 38.7.

Scrub: hitch ms/s / longest ms while the indicator is dragged. Carousel: a horizontal list in the list, its hitch ms/s, and the diagonal drags' verdicts when either is not UIKit's.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel | Other hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 0.0 | 17 | 0 | 0/4 | 1 | 0.0 / 17 | – | 1.5 | 36 | 1 | 1/4 | 0 | – | – |
| recipes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| expenses | uikit | 0.0 | 17 | 0 | 0/4 | 1 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| podcasts | swiftui | 1.5 | 33 | 0 | 0/4 | 0 | 95.0 / 83 | – | 1.3 | 48 | 0 | 0/4 | 0 | 378.2 / 96 | – |
| notes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.8 | 35 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| watchlist | uikit | 0.7 | 33 | 0 | 0/4 | 0 | 5.2 / 33 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| chat | swiftui | 0.8 | 35 | 0 | 0/4 | 0 | 0.0 / 17 | – | 3.8 | 81 | 2 | 0/4 | 0 | 0.0 / 17 | – |
| moodboard | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| inbox | uikit | 1.5 | 54 | 0 | 0/4 | 0 | 4.2 / 33 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| tasks | uikit | 1.4 | 39 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| settings | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 2 | 0/4 | 0 | 0.0 / 17 | – |
| feed | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 36.1 / 51 | 0.0 | 4.9 | 38 | 1 | 1/4 | 0 | 125.4 / 72 | 0.0 |
| photos | uikit | 1.3 | 35 | 0 | 1/4 | 0 | not taken | – | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| timetable | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |

## The pairs

Means, Exact vs other: overall 6.7 vs 7.7; platform 7.1 vs 7.3; tells an app 5.3 vs 4.5; bugs an app 1.3 vs 1.1.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 27.9 | 12.6 | 10.65 | 2.20 | 6 | 8 | 6 | 7 | 5 / 6 | 2 / 1 | tracking.lag tell | smooth.stall tell, momentum.lift tell, momentum.jumps tell | swiftui |
| recipes | claude | uikit | 17.2 | 6.6 | 3.48 | 1.01 | 7 | 8 | 7 | 8 | 5 / 5 | 1 / 1 | ok | ok | uikit |
| expenses | gpt | uikit | 16.6 | 17.0 | 5.82 | 3.26 | 6 | 8 | 8 | 7 | 5 / 6 | 2 / 0 | tracking.lag tell | ok | uikit |
| workout-log | claude | swiftui | 15.7 | 8.5 | 2.95 | 1.78 | 7 | 8 | 7 | 8 | 3 / 3 | 1 / 1 | ok | ok | swiftui |
| podcasts | gpt | swiftui | 16.0 | 12.6 | 5.32 | 3.60 | 5 | 9 | 5 | 8 | 8 / 5 | 2 / 0 | blank.blank bad, scrub.hitch bad, scrub.stall bad, scrub.blank bad | smooth.stall tell, scrub.hitch bad, scrub.stall bad | swiftui |
| notes | claude | uikit | 7.6 | 4.7 | 2.05 | 0.79 | 8 | 9 | 8 | 9 | 5 / 3 | 1 / 0 | ok | smooth.stall tell | uikit |
| watchlist | gpt | uikit | 16.2 | 17.4 | 4.92 | 3.58 | 8 | 6 | 8 | 5 | 4 / 5 | 2 / 4 | scrub.hitch tell | scrollToTop.reached tell | Exact |
| chat | claude | swiftui | 16.2 | 7.8 | 4.56 | 1.34 | 7 | 9 | 7 | 9 | 5 / 3 | 2 / 2 | smooth.stall tell, blank.blank tell, scrub.blank tell | smooth.stall bad, momentum.jumps tell, scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 17.0 | 10.6 | 6.45 | 2.23 | 8 | 7 | 7 | 6 | 5 / 6 | 0 / 1 | ok | ok | Exact |
| inbox | gpt | uikit | 21.5 | 10.1 | 7.48 | 1.78 | 7 | 7 | 8 | 6 | 4 / 6 | 1 / 2 | smooth.stall tell, rubberBand.ratio tell | ok | uikit |
| tasks | claude | uikit | 22.3 | 7.6 | 7.62 | 1.23 | 4 | 5 | 6 | 8 | 7 / 2 | 2 / 3 | smooth.stall tell | ok | uikit |
| settings | claude | swiftui | 13.5 | 3.7 | 4.81 | 0.71 | 8 | 9 | 9 | 9 | 5 / 4 | 0 / 0 | ok | momentum.jumps tell | swiftui |
| feed | gpt | swiftui | 22.3 | 10.0 | 9.31 | 2.03 | 7 | 8 | 7 | 6 | 6 / 4 | 2 / 0 | scrub.hitch bad, scrub.stall tell | smooth.stall tell, momentum.lift tell, momentum.jumps tell, scrub.hitch bad, scrub.stall bad, tracking.glide bad, momentum.tau bad, momentum.handoff tell | swiftui |
| photos | claude | uikit | 19.5 | 9.0 | 7.50 | 1.88 | 8 | 7 | 6 | 8 | 5 / 4 | 0 / 1 | smooth.stall tell, momentum.lift tell | scrollToTop.reached tell | Exact |
| timetable | gpt | uikit | 22.7 | 13.8 | 8.17 | 2.51 | 5 | 7 | 7 | 6 | 7 / 5 | 2 / 1 | ok | scrollToTop.reached tell | uikit |
