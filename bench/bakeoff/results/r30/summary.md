# Bake-off r30

exact2 4963c75490. Exact over the other, per pair: minutes **1.40×**, tokens **1.98×** (geometric means, timeouts excluded). Preferred: Exact 7, other 8, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **14 of 15**, the other 13 of 15. Mean hitch ratio: Exact 1.0 ms/s, the other 2.3. Scrubbing (the scroll indicator dragged end to end and back, 13 pairs): Exact 23.2 ms/s, the other 26.3.

Scrub: hitch ms/s / longest ms while the indicator is dragged. Carousel: a horizontal list in the list, its hitch ms/s, and the diagonal drags' verdicts when either is not UIKit's.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel | Other hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 0.7 | 33 | 0 | 0/4 | 0 | – | – | 3.3 | 49 | 0 | 0/4 | 0 | – | – |
| recipes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 3.1 | 40 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| expenses | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| workout-log | swiftui | 2.8 | 43 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.8 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| podcasts | swiftui | 5.2 | 48 | 2 | 2/4 | 0 | 250.1 / 74 | – | 1.4 | 33 | 0 | 0/4 | 0 | 286.6 / 79 | – |
| notes | uikit | 1.9 | 61 | 0 | 0/4 | 0 | 0.0 / 17 | – | 2.4 | 46 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| watchlist | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 13.3 / 43 | – |
| chat | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 8.2 / 41 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| moodboard | swiftui | 0.0 | 17 | 0 | 0/4 | 1 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| inbox | uikit | 2.3 | 52 | 0 | 0/4 | 0 | 0.0 / 17 | – | 10.5 | 163 | 0 | 0/4 | 0 | 38.3 / 145 | – |
| tasks | uikit | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| settings | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | – | – | 11.3 | 82 | 0 | 0/4 | 0 | – | – |
| feed | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 43.2 / 114 | 0.0 | 0.0 | 17 | 0 | 0/4 | 0 | 4.1 / 33 | 0.0 |
| photos | uikit | 1.5 | 41 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | – | 0.0 / 17 | – |
| timetable | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |

## The pairs

Means, Exact vs other: overall 7.3 vs 7.5; platform 7.3 vs 7.4; tells an app 4.9 vs 4.7; bugs an app 1.5 vs 1.3.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 13.4 | 17.2 | 3.48 | 6.27 | 8 | 7 | 9 | 6 | 4 / 6 | 1 / 4 | blank.blank bad | smooth.stall tell | Exact |
| recipes | claude | uikit | 10.9 | 6.9 | 1.90 | 1.17 | 6 | 8 | 7 | 8 | 6 / 4 | 2 / 0 | ok | smooth.stall tell | uikit |
| expenses | gpt | uikit | 14.7 | 26.4 | 3.66 | 6.72 | 7 | 9 | 8 | 8 | 4 / 6 | 2 / 0 | ok | ok | uikit |
| workout-log | claude | swiftui | 11.4 | 7.1 | 3.13 | 1.59 | 8 | 9 | 8 | 8 | 4 / 4 | 0 / 1 | smooth.stall tell | ok | swiftui |
| podcasts | gpt | swiftui | 14.4 | 18.0 | 4.27 | 4.61 | 7 | 6 | 5 | 8 | 8 / 5 | 1 / 2 | smooth.hitch tell, smooth.stall tell, momentum.lift bad, momentum.jumps tell, blank.blank bad, scrub.hitch bad, scrub.stall bad, scrub.blank bad | scrub.hitch bad, scrub.stall bad | Exact |
| notes | claude | uikit | 8.7 | 4.1 | 3.64 | 0.83 | 7 | 9 | 8 | 9 | 4 / 3 | 2 / 0 | smooth.stall tell | smooth.stall tell | uikit |
| watchlist | gpt | uikit | 18.2 | 14.7 | 5.33 | 2.17 | 7 | 6 | 7 | 6 | 6 / 7 | 3 / 3 | scrollToTop.reached bad | scrub.hitch bad, scrub.stall tell | Exact |
| chat | claude | swiftui | 9.7 | 3.9 | 4.13 | 0.84 | 7 | 8 | 7 | 8 | 4 / 4 | 1 / 1 | scrollToTop.reached tell, blank.blank tell, scrub.hitch tell, scrub.stall tell, scrub.blank tell | scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 21.0 | 13.5 | 6.02 | 3.48 | 7 | 7 | 7 | 6 | 6 / 6 | 0 / 0 | tracking.lag tell | ok | Exact |
| inbox | gpt | uikit | 19.7 | 12.5 | 7.48 | 3.68 | 9 | 6 | 9 | 5 | 4 / 7 | 2 / 3 | smooth.stall tell, rubberBand.ratio tell | smooth.hitch bad, smooth.stall bad, rubberBand.settle bad, scrub.hitch bad, scrub.stall bad | Exact |
| tasks | claude | uikit | 17.7 | 8.8 | 8.03 | 2.31 | 8 | 6 | 7 | 9 | 5 / 3 | 0 / 2 | ok | ok | Exact |
| settings | claude | swiftui | 19.8 | 4.7 | 4.43 | 0.86 | 7 | 9 | 8 | 9 | 4 / 3 | 2 / 1 | ok | smooth.hitch bad, smooth.stall bad, tapToStop.stop bad | swiftui |
| feed | gpt | swiftui | 11.7 | 9.4 | 4.29 | 1.81 | 8 | 7 | 8 | 6 | 4 / 5 | 2 / 1 | scrub.hitch bad, scrub.stall bad | tracking.glide bad, momentum.tau bad | Exact |
| photos | claude | uikit | 16.0 | 9.0 | 7.18 | 1.74 | 6 | 8 | 5 | 9 | 7 / 2 | 3 / 1 | smooth.stall tell | tracking.glide bad, scrollToTop.reached tell | uikit |
| timetable | gpt | uikit | 13.1 | 22.2 | 4.55 | 4.72 | 7 | 8 | 7 | 6 | 4 / 6 | 1 / 0 | tracking.ratio bad | scrollToTop.reached tell | uikit |
