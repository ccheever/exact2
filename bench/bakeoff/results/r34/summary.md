# Bake-off r34

exact2 f2cbe5de1c. Exact over the other, per pair: minutes **1.45×**, tokens **1.98×** (geometric means, timeouts excluded). Preferred: Exact 6, other 9, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **15 of 15**, the other 12 of 15. Mean hitch ratio: Exact 0.6 ms/s, the other 1.7. Scrubbing (the scroll indicator dragged end to end and back, 12 pairs): Exact 0.0 ms/s, the other 38.4.

Scrub: hitch ms/s / longest ms while the indicator is dragged. Carousel: a horizontal list in the list, its hitch ms/s, and the diagonal drags' verdicts when either is not UIKit's.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel | Other hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | – | – | 1.5 | 36 | 0 | 0/4 | 0 | 29.3 / 47 | – |
| recipes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 3.6 | 57 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| expenses | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 4.9 / 33 | – |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 20 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| podcasts | swiftui | 1.2 | 44 | 0 | 0/4 | 0 | 0.0 / 17 | – | 4.8 | 50 | 1 | 1/4 | 0 | 402.9 / 100 | – |
| notes | uikit | 0.7 | 34 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| watchlist | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 2.2 | 41 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| chat | swiftui | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.5 | 35 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| moodboard | swiftui | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.9 | 37 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| inbox | uikit | 2.8 | 52 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| tasks | uikit | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| settings | swiftui | 1.8 | 33 | 0 | 0/4 | 0 | – | – | 9.4 | 74 | 0 | 0/4 | 0 | – | – |
| feed | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | 0.0 | 1.0 | 33 | 1 | 1/4 | 0 | 53.1 / 47 | 0.0 |
| photos | uikit | 0.0 | 17 | 0 | 0/4 | – | not taken | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| timetable | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |

## The pairs

Means, Exact vs other: overall 7.3 vs 7.9; platform 7.3 vs 7.1; tells an app 4.8 vs 4.3; bugs an app 1.2 vs 0.9.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 18.6 | 14.7 | 5.60 | 2.51 | 9 | 7 | 9 | 5 | 3 / 7 | 0 / 1 | ok | smooth.stall tell, scrub.hitch bad, scrub.stall tell | Exact |
| recipes | claude | uikit | 19.6 | 8.6 | 3.86 | 1.82 | 7 | 9 | 8 | 8 | 5 / 3 | 1 / 0 | ok | smooth.stall tell | uikit |
| expenses | gpt | uikit | 15.9 | 17.2 | 3.57 | 3.29 | 6 | 8 | 7 | 7 | 5 / 5 | 3 / 0 | ok | ok | uikit |
| workout-log | claude | swiftui | 20.9 | 9.8 | 5.11 | 2.05 | 8 | 9 | 8 | 9 | 3 / 3 | 1 / 1 | ok | ok | swiftui |
| podcasts | gpt | swiftui | 13.7 | 23.7 | 3.22 | 5.85 | 6 | 8 | 6 | 7 | 6 / 5 | 1 / 0 | smooth.stall tell | smooth.stall tell, momentum.lift tell, momentum.jumps tell, scrub.hitch bad, scrub.stall bad | swiftui |
| notes | claude | uikit | 11.8 | 5.2 | 3.81 | 1.10 | 7 | 9 | 7 | 9 | 4 / 2 | 1 / 0 | ok | ok | uikit |
| watchlist | gpt | uikit | 14.8 | 17.7 | 3.89 | 2.71 | 8 | 7 | 8 | 6 | 5 / 5 | 1 / 2 | ok | smooth.stall tell | Exact |
| chat | claude | swiftui | 12.8 | 3.5 | 2.64 | 0.68 | 7 | 8 | 7 | 8 | 4 / 3 | 1 / 2 | ok | smooth.stall tell, scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 17.2 | 24.1 | 5.14 | 6.04 | 7 | 6 | 7 | 5 | 5 / 6 | 1 / 0 | scrollToTop.reached tell | smooth.stall tell | Exact |
| inbox | gpt | uikit | 12.1 | 10.8 | 3.84 | 1.56 | 7 | 8 | 7 | 6 | 6 / 5 | 3 / 2 | smooth.stall tell, rubberBand.ratio tell | ok | uikit |
| tasks | claude | uikit | 17.9 | 10.8 | 6.46 | 2.47 | 6 | 9 | 6 | 9 | 8 / 3 | 0 / 0 | ok | ok | uikit |
| settings | claude | swiftui | 22.6 | 8.1 | 5.21 | 1.50 | 8 | 9 | 8 | 9 | 4 / 3 | 2 / 2 | tapToStop.stop bad | smooth.hitch tell, smooth.stall bad, tapToStop.stop bad | swiftui |
| feed | gpt | swiftui | 23.7 | 14.0 | 6.83 | 3.60 | 8 | 8 | 8 | 6 | 4 / 5 | 1 / 0 | ok | momentum.lift tell, momentum.jumps tell, scrub.hitch bad, scrub.stall tell | Exact |
| photos | claude | uikit | 16.8 | 8.9 | 4.94 | 1.41 | 8 | 7 | 6 | 8 | 6 / 4 | 2 / 1 | tracking.glide bad | scrollToTop.reached tell | Exact |
| timetable | gpt | uikit | 17.0 | 16.5 | 5.21 | 3.15 | 8 | 6 | 8 | 5 | 4 / 6 | 0 / 3 | ok | scrollToTop.reached tell | Exact |
