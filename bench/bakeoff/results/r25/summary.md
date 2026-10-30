# Bake-off r25

exact2 540821a86a. Exact over the other, per pair: minutes **1.75×**, tokens **2.72×** (geometric means, timeouts excluded). Preferred: Exact 3, other 12, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **14 of 15**, the other 13 of 15. Mean hitch ratio: Exact 0.4 ms/s, the other 2.9. Scrubbing (the scroll indicator dragged end to end and back, 13 pairs): Exact 22.0 ms/s, the other 52.1.

Scrub: hitch ms/s / longest ms while the indicator is dragged. Carousel: a horizontal list in the list, its hitch ms/s, and the diagonal drags' verdicts when either is not UIKit's.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel | Other hitch ms/s | longest ms | jumps | lift drops | lag frames | scrub | carousel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | – | – | 1.4 | 33 | 0 | 0/4 | 0 | 285.6 / 108 | – |
| recipes | uikit | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.8 | 34 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| expenses | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.8 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| podcasts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 35.0 | 184 | 8 | 2/4 | 0 | 546.9 / 168 | – |
| notes | uikit | 0.0 | 17 | 3 | 0/4 | 0 | 0.0 / 17 | – | 0.7 | 34 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| watchlist | uikit | 0.7 | 33 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| chat | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 49.7 / 61 | – | 2.2 | 42 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| moodboard | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 8.7 / 46 | – | 0.0 | 17 | 0 | 0/4 | 1 | 16.4 / 52 | – |
| inbox | uikit | 2.5 | 52 | 0 | 0/4 | 1 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| tasks | uikit | 2.8 | 40 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.0 | 17 | 0 | 0/4 | 0 | 10.1 / 49 | – |
| settings | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | – | – | 1.7 | 41 | 0 | 0/4 | 0 | 0.0 / 17 | – |
| feed | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 223.1 / 172 | 0.0 | 1.0 | 33 | 2 | 0/4 | 0 | 104.3 / 63 | 0.0 |
| photos | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 4.6 / 35 | – | 0.0 | 17 | 0 | 0/4 | – | 0.0 / 17 | – |
| timetable | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 / 17 | – | 0.4 | 26 | 0 | 0/4 | 0 | 0.0 / 17 | – |

## The pairs

Means, Exact vs other: overall 6.7 vs 7.8; platform 6.9 vs 7.3; tells an app 5.7 vs 5.0; bugs an app 2.1 vs 1.1.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 21.0 | 18.2 | 7.06 | 5.03 | 6 | 8 | 6 | 7 | 5 / 6 | 4 / 2 | ok | scrub.hitch bad, scrub.stall bad | swiftui |
| recipes | claude | uikit | 16.4 | 8.0 | 3.84 | 1.47 | 6 | 8 | 7 | 8 | 5 / 3 | 3 / 1 | ok | smooth.stall tell | uikit |
| expenses | gpt | uikit | 20.1 | 18.1 | 7.68 | 2.85 | 8 | 7 | 8 | 6 | 6 / 8 | 0 / 0 | ok | ok | Exact |
| workout-log | claude | swiftui | 20.0 | 7.8 | 4.13 | 1.91 | 8 | 8 | 8 | 8 | 3 / 4 | 3 / 3 | ok | ok | swiftui |
| podcasts | gpt | swiftui | 14.5 | 14.8 | 4.02 | 4.06 | 5 | 8 | 5 | 8 | 7 / 6 | 3 / 3 | ok | smooth.hitch bad, smooth.stall bad, smooth.main tell, momentum.lift bad, momentum.jumps bad, scrub.hitch bad, scrub.stall bad | swiftui |
| notes | claude | uikit | 9.6 | 7.6 | 2.64 | 1.45 | 6 | 9 | 6 | 9 | 9 / 3 | 4 / 0 | momentum.jumps bad | ok | uikit |
| watchlist | gpt | uikit | 17.2 | 13.8 | 5.87 | 2.54 | 6 | 7 | 7 | 6 | 6 / 10 | 2 / 1 | ok | ok | uikit |
| chat | claude | swiftui | 19.2 | 3.9 | 6.09 | 0.66 | 6 | 7 | 6 | 8 | 5 / 2 | 1 / 1 | blank.blank tell, scrub.hitch bad, scrub.stall tell, scrub.blank tell | smooth.stall tell, scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 21.9 | 18.4 | 7.70 | 6.12 | 7 | 8 | 7 | 6 | 8 / 6 | 1 / 1 | scrub.hitch tell, scrub.stall tell | tracking.lag tell, scrub.hitch bad, scrub.stall tell | swiftui |
| inbox | gpt | uikit | 27.8 | 10.8 | 10.32 | 2.40 | 9 | 6 | 9 | 5 | 4 / 7 | 1 / 2 | smooth.stall tell, tracking.lag tell, rubberBand.ratio tell | ok | Exact |
| tasks | claude | uikit | 20.3 | 9.8 | 5.73 | 1.67 | 6 | 9 | 6 | 8 | 7 / 4 | 1 / 0 | smooth.stall tell | scrub.hitch bad, scrub.stall tell | uikit |
| settings | claude | swiftui | 23.9 | 7.0 | 3.97 | 0.95 | 7 | 9 | 8 | 9 | 4 / 2 | 3 / 2 | tapToStop.stop bad | smooth.stall tell, tapToStop.stop tell | swiftui |
| feed | gpt | swiftui | 24.5 | 25.5 | 8.43 | 6.51 | 8 | 6 | 8 | 6 | 5 / 7 | 0 / 1 | blank.blank bad, scrub.hitch bad, scrub.stall bad, scrub.blank bad | momentum.jumps tell, scrub.hitch bad, scrub.stall tell, tracking.glide tell, momentum.tau bad, momentum.handoff bad | Exact |
| photos | claude | uikit | 23.9 | 10.3 | 10.83 | 1.12 | 7 | 9 | 7 | 9 | 5 / 2 | 2 / 0 | scrub.stall tell | tracking.glide bad, scrollToTop.reached tell | uikit |
| timetable | gpt | uikit | 37.3 | 20.9 | 14.41 | 3.60 | 5 | 8 | 5 | 7 | 6 / 5 | 4 / 0 | ok | ok | uikit |
