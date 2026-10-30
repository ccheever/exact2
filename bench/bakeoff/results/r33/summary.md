# Bake-off r33

exact2 05b2942866. Exact over the other, per pair: minutes **1.23×**, tokens **1.97×** (geometric means, timeouts excluded). Preferred: Exact 5, other 10, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **15 of 15**, the other 13 of 15. Mean hitch ratio: Exact 0.4 ms/s, the other 0.6. Scrubbing (the scroll indicator dragged end to end and back, 14 pairs): Exact 0.0 ms/s, the other 49.7.

Scrub: hitch ms/s / longest ms while the indicator is dragged. Carousel: a horizontal list in the list, its hitch ms/s, and the diagonal drags' verdicts when either is not UIKit's.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel | Other hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | – | – | 0.7 | 33 | 0 | 0/4 | 0 | – | – |
| recipes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.8 | 35 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| expenses | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.5 | 33 | 0 | 1/4 | 0 | 0.0 / 17 | – |
| podcasts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.7 | 33 | 1 | 1/4 | 0 | 389.2 / 98 | – |
| notes | uikit | 2.2 | 58 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.8 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| watchlist | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| chat | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 2.0 | 42 | 3 | 0/4 | 0 | 0.0 / 17 | – |
| moodboard | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| inbox | uikit | 2.3 | 54 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| tasks | uikit | 0.8 | 36 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| settings | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.8 | 33 | 0 | 0/4 | 1 | 0.0 / 17 | – |
| feed | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | 0.0 | 1.0 | 33 | 0 | 0/4 | 0 | 307.2 / 66 | 0.0 |
| photos | uikit | 0.0 | 17 | 0 | 0/4 | – | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| timetable | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |

## The pairs

Means, Exact vs other: overall 7.3 vs 7.7; platform 7.7 vs 7.4; tells an app 4.7 vs 4.4; bugs an app 1.5 vs 0.9.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 12.7 | 31.3 | 4.16 | 7.76 | 7 | 8 | 9 | 7 | 4 / 5 | 1 / 1 | ok | ok | swiftui |
| recipes | claude | uikit | 18.6 | 8.4 | 5.49 | 1.80 | 7 | 9 | 7 | 8 | 4 / 2 | 1 / 0 | rubberBand.ratio bad, rubberBand.settle bad | smooth.stall tell | uikit |
| expenses | gpt | uikit | 14.5 | 32.3 | 3.73 | 6.85 | 7 | 8 | 8 | 7 | 4 / 6 | 2 / 1 | ok | ok | uikit |
| workout-log | claude | swiftui | 11.2 | 7.0 | 3.27 | 1.33 | 8 | 8 | 8 | 8 | 3 / 5 | 0 / 1 | ok | momentum.lift tell | Exact |
| podcasts | gpt | swiftui | 16.5 | 12.0 | 4.80 | 1.64 | 6 | 8 | 6 | 7 | 7 / 6 | 2 / 0 | ok | momentum.lift tell, momentum.jumps tell, scrub.hitch bad, scrub.stall bad | swiftui |
| notes | claude | uikit | 13.4 | 5.8 | 2.66 | 1.27 | 7 | 9 | 7 | 9 | 6 / 2 | 1 / 0 | smooth.stall tell | ok | uikit |
| watchlist | gpt | uikit | 20.6 | 18.4 | 4.10 | 2.96 | 9 | 7 | 9 | 6 | 4 / 7 | 1 / 2 | ok | ok | Exact |
| chat | claude | swiftui | 10.9 | 6.1 | 4.82 | 0.87 | 7 | 8 | 8 | 8 | 4 / 4 | 1 / 1 | ok | smooth.stall tell, momentum.jumps bad, scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 13.9 | 12.1 | 3.61 | 2.35 | 7 | 6 | 7 | 6 | 5 / 4 | 1 / 0 | ok | ok | Exact |
| inbox | gpt | uikit | 14.2 | 14.3 | 3.28 | 2.14 | 8 | 6 | 9 | 5 | 4 / 6 | 2 / 1 | smooth.stall tell | ok | Exact |
| tasks | claude | uikit | 15.5 | 11.8 | 6.51 | 3.64 | 7 | 8 | 7 | 9 | 6 / 4 | 0 / 0 | smooth.stall tell | ok | uikit |
| settings | claude | swiftui | 11.7 | 8.3 | 4.16 | 1.77 | 8 | 9 | 9 | 9 | 4 / 3 | 3 / 1 | ok | tracking.lag tell | swiftui |
| feed | gpt | swiftui | 16.6 | 14.6 | 5.78 | 2.28 | 6 | 7 | 7 | 7 | 6 / 4 | 3 / 2 | ok | scrub.hitch bad, scrub.stall tell, tracking.glide bad, momentum.tau bad | swiftui |
| photos | claude | uikit | 22.1 | 8.4 | 10.05 | 1.65 | 8 | 7 | 7 | 9 | 4 / 3 | 2 / 2 | tracking.glide bad | scrollToTop.reached tell | Exact |
| timetable | gpt | uikit | 14.9 | 17.6 | 5.16 | 2.98 | 7 | 7 | 7 | 6 | 6 / 5 | 2 / 2 | ok | ok | uikit |
