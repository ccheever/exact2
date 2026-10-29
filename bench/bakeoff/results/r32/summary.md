# Bake-off r32

exact2 a1be48b493. Exact over the other, per pair: minutes **1.47×**, tokens **2.27×** (geometric means, timeouts excluded). Preferred: Exact 7, other 8, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **13 of 15**, the other 14 of 15. Mean hitch ratio: Exact 1.2 ms/s, the other 0.9. Scrubbing (the scroll indicator dragged end to end and back, 14 pairs): Exact 22.9 ms/s, the other 37.5.

Scrub: hitch ms/s / longest ms while the indicator is dragged. Carousel: a horizontal list in the list, its hitch ms/s, and the diagonal drags' verdicts when either is not UIKit's.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel | Other hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | – | – | 0.0 | 17 | 0 | 0/4 | 0 | 70.7 / 47 | – |
| recipes | uikit | 0.8 | 34 | 0 | 0/4 | 0 | 0.0 / 17 | – | 2.2 | 47 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| expenses | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| podcasts | swiftui | 3.7 | 41 | 1 | 1/4 | 0 | 272.0 / 119 | – | 4.0 | 79 | 0 | 0/4 | 0 | 371.6 / 85 | – |
| notes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| watchlist | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| chat | swiftui | 1.5 | 49 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.4 | 48 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| moodboard | swiftui | 3.0 | 39 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| inbox | uikit | 1.5 | 52 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| tasks | uikit | 7.4 | 83 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 5.7 / 36 | – |
| settings | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 2.1 | 37 | 0 | 0/4 | – | 0.0 / 17 | – |
| feed | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 48.1 / 45 | 0.0 | 3.1 | 35 | 4 | 0/4 | 0 | 148.1 / 67 | 0.0 |
| photos | uikit | 0.0 | 17 | 0 | 0/4 | – | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| timetable | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |

## The pairs

Means, Exact vs other: overall 7.5 vs 7.5; platform 7.6 vs 7.3; tells an app 4.7 vs 4.3; bugs an app 1.2 vs 1.3.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 16.6 | 11.8 | 5.10 | 1.78 | 6 | 7 | 9 | 5 | 4 / 8 | 4 / 3 | ok | scrub.hitch bad, scrub.stall tell | swiftui |
| recipes | claude | uikit | 13.3 | 7.3 | 2.28 | 1.10 | 7 | 9 | 8 | 9 | 5 / 2 | 0 / 0 | ok | smooth.stall tell | uikit |
| expenses | gpt | uikit | 16.3 | 13.5 | 5.75 | 2.67 | 7 | 8 | 8 | 7 | 3 / 3 | 2 / 3 | ok | ok | uikit |
| workout-log | claude | swiftui | 17.4 | 6.9 | 3.75 | 1.37 | 8 | 9 | 8 | 8 | 5 / 4 | 0 / 1 | ok | ok | swiftui |
| podcasts | gpt | swiftui | 17.8 | 13.4 | 4.44 | 2.47 | 6 | 7 | 6 | 8 | 6 / 5 | 2 / 1 | smooth.stall tell, momentum.lift tell, momentum.jumps tell, blank.blank bad, scrub.hitch bad, scrub.stall bad, scrub.blank bad | smooth.stall bad, scrub.hitch bad, scrub.stall bad | swiftui |
| notes | claude | uikit | 8.4 | 6.0 | 2.35 | 1.14 | 7 | 9 | 8 | 9 | 5 / 2 | 1 / 0 | ok | ok | uikit |
| watchlist | gpt | uikit | 16.1 | 14.4 | 4.93 | 2.18 | 8 | 6 | 8 | 5 | 4 / 5 | 2 / 2 | ok | ok | Exact |
| chat | claude | swiftui | 12.3 | 5.6 | 2.42 | 1.46 | 7 | 8 | 7 | 8 | 5 / 3 | 0 / 0 | smooth.stall tell, scrollToTop.reached tell | smooth.stall tell, scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 20.1 | 14.6 | 5.58 | 2.65 | 8 | 7 | 8 | 6 | 4 / 6 | 1 / 0 | smooth.stall tell | ok | Exact |
| inbox | gpt | uikit | 10.9 | 10.3 | 3.48 | 1.50 | 8 | 7 | 7 | 6 | 5 / 8 | 2 / 2 | smooth.stall tell, rubberBand.ratio tell | ok | Exact |
| tasks | claude | uikit | 20.2 | 8.5 | 6.34 | 1.50 | 8 | 7 | 6 | 9 | 7 / 3 | 0 / 2 | smooth.hitch tell, smooth.stall bad | scrollToTop.reached tell, scrub.hitch tell, scrub.stall tell | Exact |
| settings | claude | swiftui | 14.2 | 7.4 | 5.31 | 1.56 | 8 | 9 | 8 | 9 | 4 / 3 | 1 / 0 | ok | smooth.stall tell | swiftui |
| feed | gpt | swiftui | 14.4 | 15.0 | 4.27 | 3.16 | 8 | 7 | 9 | 6 | 3 / 5 | 1 / 1 | scrub.hitch bad, scrub.stall tell, tracking.glide bad, momentum.tau bad, momentum.handoff bad | smooth.stall tell, momentum.jumps bad, scrub.hitch bad, scrub.stall tell, tracking.glide bad, momentum.tau bad, momentum.handoff tell | Exact |
| photos | claude | uikit | 17.4 | 8.7 | 7.19 | 1.71 | 8 | 7 | 6 | 8 | 6 / 3 | 1 / 2 | tracking.glide bad | scrollToTop.reached tell | Exact |
| timetable | gpt | uikit | 13.3 | 19.4 | 4.18 | 3.72 | 8 | 6 | 8 | 6 | 4 / 4 | 1 / 3 | ok | ok | Exact |
