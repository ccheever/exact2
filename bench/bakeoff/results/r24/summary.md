# Bake-off r24

exact2 8dbafe148f. Exact over the other, per pair: minutes **1.59×**, tokens **2.59×** (geometric means, timeouts excluded). Preferred: Exact 5, other 10, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **12 of 15**, the other 11 of 15. Mean hitch ratio: Exact 3.8 ms/s, the other 1.0. Scrubbing (the scroll indicator dragged end to end and back, 7 pairs): Exact 145.0 ms/s, the other 24.9.

Scrub: hitch ms/s / longest ms while the indicator is dragged. Carousel: a horizontal list in the list, its hitch ms/s, and the diagonal drags' verdicts when either is not UIKit's.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel | Other hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | – | – | 3.4 | 33 | 1 | 1/4 | 0 | – | – |
| recipes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | – | – | 1.4 | 36 | 0 | 0/4 | 0 | – | – |
| expenses | uikit | 0.0 | 17 | 0 | 0/4 | 0 | – | – | 0.0 | 17 | 0 | 0/4 | 0 | – | – |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | – | – | 0.0 | 17 | 0 | 0/4 | 0 | – | – |
| podcasts | swiftui | 1.2 | 33 | 0 | 0/4 | 0 | – | – | 3.6 | 36 | 0 | 1/4 | 0 | – | – |
| notes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | – | – | 1.3 | 33 | 0 | 0/4 | 0 | – | – |
| watchlist | uikit | 0.0 | 17 | 0 | 0/4 | 0 | – | – | 0.0 | 17 | 0 | 0/4 | 0 | – | – |
| chat | swiftui | 2.5 | 41 | 0 | 0/4 | 0 | – | – | 2.6 | 35 | 2 | 0/4 | 0 | – | – |
| moodboard | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| inbox | uikit | 15.8 | 69 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| tasks | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| settings | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.0 | 36 | 2 | 0/4 | 0 | 0.0 / 17 | – |
| feed | swiftui | 26.6 | 88 | 19 | 2/4 | 0 | 622.5 / 308 | 0.0 | 1.0 | 33 | 2 | 1/4 | 0 | 174.0 / 72 | 0.0 |
| photos | uikit | 7.4 | 66 | 0 | 0/4 | 0 | 384.0 / 131 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| timetable | uikit | 4.0 | 36 | 0 | 0/4 | 0 | 8.7 / 38 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |

## The pairs

Means, Exact vs other: overall 6.7 vs 7.7; platform 6.9 vs 7.5; tells an app 5.6 vs 4.2; bugs an app 1.8 vs 1.1.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 21.4 | 23.0 | 8.38 | 7.24 | 6 | 8 | 7 | 6 | 5 / 6 | 3 / 2 | ok | momentum.lift tell, momentum.jumps tell | swiftui |
| recipes | claude | uikit | 10.6 | 8.9 | 2.88 | 1.84 | 8 | 9 | 9 | 9 | 4 / 4 | 0 / 0 | ok | smooth.stall tell | uikit |
| expenses | gpt | uikit | 14.9 | 14.8 | 3.73 | 3.01 | 5 | 8 | 6 | 8 | 6 / 3 | 2 / 2 | ok | ok | uikit |
| workout-log | claude | swiftui | 22.0 | 8.6 | 3.57 | 1.84 | 7 | 9 | 7 | 9 | 6 / 2 | 2 / 1 | ok | ok | swiftui |
| podcasts | gpt | swiftui | 18.5 | 17.4 | 3.70 | 4.51 | 5 | 9 | 5 | 8 | 10 / 5 | 3 / 0 | ok | smooth.stall tell, smooth.main tell, momentum.lift tell | swiftui |
| notes | claude | uikit | 12.4 | 5.5 | 3.62 | 0.89 | 6 | 9 | 6 | 9 | 6 / 2 | 4 / 0 | scrollToTop.reached tell | scrollToTop.reached tell | uikit |
| watchlist | gpt | uikit | 17.8 | 15.5 | 5.44 | 3.45 | 7 | 6 | 6 | 5 | 7 / 7 | 2 / 2 | ok | ok | Exact |
| chat | claude | swiftui | 17.8 | 4.3 | 6.71 | 0.79 | 7 | 9 | 7 | 9 | 5 / 3 | 0 / 0 | smooth.stall tell, scrollToTop.reached tell | smooth.stall tell, momentum.jumps tell, scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 27.2 | 17.5 | 8.21 | 3.63 | 6 | 7 | 6 | 6 | 6 / 5 | 0 / 1 | ok | ok | swiftui |
| inbox | gpt | uikit | 20.7 | 10.7 | 7.59 | 1.56 | 8 | 7 | 7 | 6 | 5 / 6 | 2 / 1 | smooth.hitch bad, smooth.stall bad, rubberBand.ratio tell, rubberBand.settle bad | rubberBand.ratio tell | Exact |
| tasks | claude | uikit | 26.4 | 12.6 | 10.89 | 2.82 | 6 | 7 | 7 | 8 | 7 / 4 | 1 / 2 | ok | ok | uikit |
| settings | claude | swiftui | 9.3 | 7.1 | 2.96 | 1.16 | 7 | 9 | 8 | 9 | 3 / 2 | 2 / 0 | ok | smooth.stall tell, momentum.jumps tell | swiftui |
| feed | gpt | swiftui | 19.3 | 11.0 | 9.48 | 1.72 | 9 | 7 | 9 | 7 | 4 / 7 | 1 / 2 | smooth.hitch bad, smooth.stall bad, momentum.lift bad, momentum.jumps bad, rubberBand.ratio tell, scrub.hitch bad, scrub.stall bad | momentum.lift tell, momentum.jumps tell, scrub.hitch bad, scrub.stall bad | Exact |
| photos | claude | uikit | 16.7 | 8.1 | 9.30 | 1.23 | 7 | 6 | 7 | 9 | 5 / 2 | 2 / 1 | smooth.hitch tell, smooth.stall tell, scrollToTop.reached tell, scrub.hitch bad, scrub.stall bad | scrollToTop.reached tell | Exact |
| timetable | gpt | uikit | 21.7 | 20.4 | 6.65 | 3.81 | 6 | 5 | 7 | 5 | 5 / 5 | 3 / 2 | smooth.stall tell, scrub.hitch tell, scrub.stall tell | scrollToTop.reached tell | Exact |
