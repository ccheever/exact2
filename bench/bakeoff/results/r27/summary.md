# Bake-off r27

exact2 ae7a466aab. Exact over the other, per pair: minutes **1.96×**, tokens **3.11×** (geometric means, timeouts excluded). Preferred: Exact 4, other 10, tie 1.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **11 of 15**, the other 11 of 15. Mean hitch ratio: Exact 2.5 ms/s, the other 1.6. Scrubbing (the scroll indicator dragged end to end and back, 15 pairs): Exact 15.0 ms/s, the other 72.9.

Scrub: hitch ms/s / longest ms while the indicator is dragged. Carousel: a horizontal list in the list, its hitch ms/s, and the diagonal drags' verdicts when either is not UIKit's.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel | Other hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 8.7 | 118 | 0 | 0/4 | 0 | 311.8 / 118 | – |
| recipes | uikit | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.6 | 35 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| expenses | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 49.0 / 46 | – |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.3 | 46 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| podcasts | swiftui | 0.9 | 36 | 0 | 0/4 | – | 5.1 / 33 | – | 7.1 | 116 | 1 | 1/4 | 0 | 465.2 / 125 | – |
| notes | uikit | 0.0 | 17 | 2 | 0/4 | 0 | 0.0 / 17 | – | 0.7 | 34 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| watchlist | uikit | 12.1 | 39 | 0 | 0/4 | 0 | 15.8 / 38 | – | 0.0 | 17 | 0 | 0/4 | 0 | 4.9 / 33 | – |
| chat | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 12.5 / 56 | – | 0.0 | 17 | 0 | 0/4 | 0 | 69.7 / 35 | – |
| moodboard | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| inbox | uikit | 6.4 | 79 | 0 | 0/4 | 0 | 10.7 / 36 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| tasks | uikit | 12.7 | 116 | 34 | 1/4 | 0 | 107.3 / 81 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| settings | swiftui | 0.9 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – | 1.7 | 39 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| feed | swiftui | 2.2 | 38 | 0 | 0/4 | 0 | 60.5 / 45 | 0.0 | 2.2 | 37 | 1 | 1/4 | 0 | 192.6 / 58 | 0.0 |
| photos | uikit | 0.9 | 33 | 0 | 0/4 | 0 | 12.9 / 50 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| timetable | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 1 | 0/4 | 0 | 0.0 / 17 | – |

## The pairs

Means, Exact vs other: overall 6.9 vs 7.8; platform 7.2 vs 7.6; tells an app 5.5 vs 4.0; bugs an app 1.7 vs 1.1.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 52.1 | 21.5 | 21.62 | 5.79 | 7 | 8 | 7 | 7 | 5 / 6 | 2 / 1 | ok | smooth.hitch tell, smooth.stall bad, scrub.hitch bad, scrub.stall bad | swiftui |
| recipes | claude | uikit | 22.8 | 11.7 | 5.04 | 2.46 | 8 | 9 | 8 | 8 | 5 / 3 | 0 / 0 | ok | smooth.stall tell, scrollToTop.reached tell | uikit |
| expenses | gpt | uikit | 16.2 | 16.5 | 4.55 | 2.61 | 8 | 7 | 9 | 7 | 4 / 4 | 2 / 1 | ok | scrub.hitch bad, scrub.stall tell | Exact |
| workout-log | claude | swiftui | 43.8 | 6.9 | 9.57 | 1.49 | 5 | 9 | 7 | 9 | 6 / 2 | 3 / 1 | ok | smooth.stall tell | swiftui |
| podcasts | gpt | swiftui | 14.4 | 15.4 | 3.15 | 3.76 | 6 | 8 | 6 | 8 | 7 / 5 | 1 / 2 | smooth.stall tell, scrub.hitch tell | smooth.hitch tell, smooth.stall bad, momentum.lift tell, momentum.jumps tell, scrub.hitch bad, scrub.stall bad | swiftui |
| notes | claude | uikit | 12.2 | 5.2 | 2.85 | 0.71 | 7 | 9 | 7 | 9 | 5 / 2 | 1 / 0 | momentum.jumps tell | ok | uikit |
| watchlist | gpt | uikit | 28.3 | 9.4 | 9.55 | 1.42 | 8 | 6 | 8 | 6 | 4 / 7 | 2 / 4 | smooth.hitch bad, smooth.stall tell, scrub.hitch bad, scrub.stall tell | ok | Exact |
| chat | claude | swiftui | 18.1 | 3.3 | 4.89 | 0.47 | 8 | 9 | 8 | 9 | 6 / 3 | 1 / 0 | blank.blank tell, scrub.hitch bad, scrub.stall tell, scrub.blank tell | scrollToTop.reached tell, scrub.hitch bad, scrub.stall tell | swiftui |
| moodboard | gpt | swiftui | 34.7 | 19.7 | 8.37 | 3.42 | 8 | 8 | 8 | 7 | 4 / 5 | 0 / 1 | scrollToTop.reached tell, blank.blank bad, scrub.blank bad | ok | tie |
| inbox | gpt | uikit | 21.5 | 21.1 | 5.14 | 5.13 | 6 | 8 | 6 | 7 | 8 / 5 | 3 / 0 | smooth.hitch tell, smooth.stall bad, scrub.hitch bad, scrub.stall tell | ok | uikit |
| tasks | claude | uikit | 21.4 | 12.6 | 10.90 | 3.39 | 6 | 5 | 5 | 9 | 8 / 3 | 3 / 3 | smooth.hitch bad, smooth.stall bad, smooth.main tell, momentum.lift tell, momentum.jumps bad, scrub.hitch bad, scrub.stall bad | ok | Exact |
| settings | claude | swiftui | 9.7 | 4.0 | 3.18 | 0.64 | 7 | 9 | 8 | 9 | 4 / 3 | 4 / 0 | rubberBand.ratio bad, rubberBand.settle bad | smooth.stall tell, tapToStop.stop bad | swiftui |
| feed | gpt | swiftui | 20.6 | 14.4 | 6.43 | 2.81 | 7 | 8 | 8 | 6 | 4 / 4 | 2 / 1 | smooth.stall tell, blank.blank bad, scrub.hitch bad, scrub.stall tell | smooth.stall tell, momentum.lift tell, momentum.jumps tell, scrub.hitch bad, scrub.stall tell, tracking.glide tell, momentum.tau bad | swiftui |
| photos | claude | uikit | 16.8 | 10.9 | 7.65 | 1.44 | 6 | 8 | 6 | 8 | 7 / 3 | 1 / 0 | tracking.ratio bad, scrub.hitch bad, scrub.stall tell | scrollToTop.reached tell | uikit |
| timetable | gpt | uikit | 22.4 | 15.5 | 8.45 | 3.33 | 7 | 6 | 7 | 5 | 6 / 5 | 1 / 2 | ok | momentum.jumps tell | Exact |
