# Bake-off r31

exact2 82fdd765d4. Exact over the other, per pair: minutes **1.34×**, tokens **2.08×** (geometric means, timeouts excluded). Preferred: Exact 7, other 8, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **15 of 15**, the other 13 of 15. Mean hitch ratio: Exact 0.6 ms/s, the other 3.0. Scrubbing (the scroll indicator dragged end to end and back, 14 pairs): Exact 0.9 ms/s, the other 35.1.

Scrub: hitch ms/s / longest ms while the indicator is dragged. Carousel: a horizontal list in the list, its hitch ms/s, and the diagonal drags' verdicts when either is not UIKit's.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel | Other hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 1.3 | 33 | 0 | 0/4 | 0 | – | – | 0.7 | 33 | 0 | 0/4 | 0 | 83.7 / 162 | – |
| recipes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 2.6 | 49 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| expenses | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| podcasts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 34.1 | 98 | 20 | 1/4 | 0 | 421.5 / 97 | – |
| notes | uikit | 1.5 | 50 | 0 | 0/4 | 1 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| watchlist | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| chat | swiftui | 1.0 | 42 | 0 | 0/4 | 0 | 12.5 / 48 | – | 1.4 | 49 | 7 | 1/4 | 0 | 0.0 / 17 | – |
| moodboard | swiftui | 0.0 | 17 | 0 | 0/4 | 1 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| inbox | uikit | 4.4 | 55 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 5.1 / 33 | – |
| tasks | uikit | 0.8 | 36 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| settings | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 2.6 | 36 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| feed | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | 0.0 | 2.0 | 35 | 0 | 1/4 | 0 | 64.6 / 52 | 0.0 |
| photos | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| timetable | uikit | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.8 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |

## The pairs

Means, Exact vs other: overall 7.4 vs 7.8; platform 7.5 vs 7.2; tells an app 4.7 vs 4.5; bugs an app 1.4 vs 1.3.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 23.7 | 18.1 | 7.44 | 2.24 | 7 | 7 | 8 | 6 | 6 / 6 | 3 / 2 | ok | scrub.hitch bad, scrub.stall bad | swiftui |
| recipes | claude | uikit | 8.8 | 6.3 | 2.51 | 1.13 | 8 | 7 | 8 | 7 | 4 / 4 | 2 / 3 | ok | smooth.stall tell | Exact |
| expenses | gpt | uikit | 19.3 | 15.6 | 5.16 | 2.35 | 7 | 9 | 7 | 8 | 5 / 4 | 2 / 0 | ok | ok | uikit |
| workout-log | claude | swiftui | 14.2 | 6.8 | 3.68 | 1.71 | 8 | 8 | 8 | 8 | 3 / 4 | 2 / 2 | ok | ok | swiftui |
| podcasts | gpt | swiftui | 12.3 | 26.6 | 2.06 | 6.62 | 6 | 9 | 7 | 8 | 6 / 5 | 2 / 0 | ok | smooth.hitch bad, smooth.stall bad, momentum.lift tell, momentum.jumps bad, scrub.hitch bad, scrub.stall bad | swiftui |
| notes | claude | uikit | 8.9 | 5.9 | 2.45 | 1.51 | 7 | 9 | 8 | 9 | 3 / 2 | 1 / 0 | smooth.stall tell, tracking.lag tell | ok | uikit |
| watchlist | gpt | uikit | 5.1 | 23.9 | 1.16 | 4.72 | 8 | 7 | 8 | 5 | 5 / 7 | 2 / 2 | ok | ok | Exact |
| chat | claude | swiftui | 12.0 | 3.4 | 4.23 | 0.79 | 7 | 8 | 7 | 8 | 5 / 3 | 0 / 2 | smooth.stall tell, scrub.hitch bad, scrub.stall tell | smooth.stall tell, momentum.lift tell, momentum.jumps bad, scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 24.9 | 13.8 | 6.36 | 2.65 | 8 | 7 | 7 | 6 | 4 / 6 | 0 / 1 | tracking.lag tell | scrollToTop.reached tell | Exact |
| inbox | gpt | uikit | 19.2 | 13.5 | 4.46 | 1.61 | 7 | 6 | 7 | 5 | 4 / 6 | 2 / 4 | smooth.stall tell, rubberBand.ratio tell | scrub.hitch tell | Exact |
| tasks | claude | uikit | 21.6 | 8.5 | 9.89 | 1.67 | 7 | 9 | 7 | 9 | 6 / 4 | 1 / 0 | smooth.stall tell | ok | uikit |
| settings | claude | swiftui | 9.3 | 4.2 | 2.23 | 0.77 | 8 | 8 | 8 | 8 | 5 / 3 | 0 / 1 | ok | smooth.stall tell | Exact |
| feed | gpt | swiftui | 23.8 | 16.2 | 6.62 | 1.94 | 8 | 7 | 8 | 6 | 3 / 6 | 1 / 2 | ok | smooth.stall tell, momentum.lift tell, scrub.hitch bad, scrub.stall tell, tracking.lag tell | Exact |
| photos | claude | uikit | 16.1 | 7.8 | 7.84 | 1.30 | 7 | 9 | 6 | 9 | 6 / 2 | 3 / 0 | ok | scrollToTop.reached tell | uikit |
| timetable | gpt | uikit | 21.3 | 27.9 | 6.09 | 5.73 | 8 | 7 | 8 | 6 | 5 / 6 | 0 / 1 | ok | ok | Exact |
