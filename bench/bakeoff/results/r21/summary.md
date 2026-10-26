# Bake-off r21

exact2 f0eeade6b1. Exact over the other, per pair: minutes **1.48×**, tokens **2.69×** (geometric means, timeouts excluded). Preferred: Exact 2, other 10, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **5 of 11**, the other 10 of 11. Mean hitch ratio: Exact 21.7 ms/s, the other 2.1.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | Other hitch ms/s | longest ms | jumps | lift drops | lag frames |
|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 26.1 | 45 | 18 | 1/4 | 3 | 1.2 | 33 | 0 | 0/4 | 0 |
| recipes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 2.4 | 39 | 0 | 0/4 | 0 |
| expenses | uikit | 9.1 | 34 | 23 | 0/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |
| podcasts | swiftui | 26.6 | 61 | 23 | 1/4 | 0 | 10.2 | 62 | 0 | 0/4 | 0 |
| notes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 1.4 | 36 | 0 | 0/4 | 0 |
| watchlist | uikit | 79.7 | 98 | 15 | 1/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |
| chat | swiftui | 4.6 | 39 | 0 | 0/4 | – | 2.4 | 43 | 0 | 0/4 | – |
| moodboard | swiftui | 73.3 | 72 | 20 | 0/4 | 0 | 1.1 | 33 | 0 | 0/4 | 0 |
| inbox | uikit | 17.4 | 107 | 8 | 1/4 | 0 | 1.1 | 33 | 0 | 0/4 | 0 |
| settings | swiftui | 1.5 | 33 | 0 | 0/4 | 0 | 3.0 | 33 | 0 | 0/4 | 0 |

## The pairs

Means, Exact vs other: overall 6.8 vs 8.2; platform 6.8 vs 7.3; tells an app 6.2 vs 4.8; bugs an app 1.7 vs 1.0.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 31.2 | 14.6 | 12.77 | 2.11 | 7 | 7 | 7 | 4 | 6 / 10 | 2 / 1 | smooth.hitch bad, smooth.stall tell, smooth.main tell, tracking.lag bad, momentum.lift tell, momentum.jumps bad, blank.blank bad | ok | Exact |
| recipes | claude | uikit | 14.8 | 8.9 | 2.89 | 1.62 | 7 | 9 | 7 | 8 | 6 / 3 | 2 / 2 | ok | smooth.stall tell | uikit |
| expenses | gpt | uikit | 18.4 | 24.6 | 5.14 | 4.99 | 7 | 8 | 7 | 7 | 7 / 4 | 0 / 1 | smooth.hitch tell, momentum.jumps bad, blank.blank bad | ok | uikit |
| workout-log | claude | swiftui | 13.4 | 6.8 | 4.85 | 1.71 | 7 | 9 | 7 | 9 | 5 / 2 | 0 / 0 | tracking.ratio tell | ok | swiftui |
| podcasts | gpt | swiftui | 14.0 | 19.9 | 3.43 | 2.19 | 6 | 8 | 6 | 7 | 7 / 5 | 2 / 2 | smooth.hitch bad, smooth.stall tell, smooth.main tell, momentum.lift tell, momentum.jumps bad | smooth.hitch bad, smooth.stall tell, smooth.main tell | swiftui |
| notes | claude | uikit | 7.9 | 7.7 | 2.78 | 2.32 | 6 | 9 | 6 | 9 | 7 / 3 | 4 / 0 | tracking.ratio tell | smooth.stall tell | uikit |
| watchlist | gpt | uikit | 19.3 | 12.4 | 6.38 | 1.18 | 6 | 7 | 6 | 6 | 6 / 7 | 3 / 2 | smooth.hitch bad, smooth.stall bad, smooth.main tell, tracking.glide bad, momentum.lift tell, momentum.jumps bad | ok | uikit |
| chat | claude | swiftui | 15.8 | 6.1 | 8.19 | 0.97 | 7 | 9 | 7 | 9 | 5 / 3 | 1 / 0 | smooth.stall tell, tracking.glide bad, rubberBand.ratio bad, scrollToTop.reached tell | smooth.stall tell, tracking.glide bad, rubberBand.ratio bad, scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 15.2 | 17.3 | 3.97 | 3.77 | 7 | 8 | 7 | 6 | 7 / 7 | 2 / 1 | smooth.hitch bad, smooth.stall bad, momentum.jumps bad | blank.blank bad | swiftui |
| inbox | gpt | uikit | 19.5 | 16.0 | 4.78 | 2.24 | 8 | 7 | 7 | 5 | 7 / 7 | 0 / 1 | smooth.hitch bad, smooth.stall bad, momentum.lift tell, momentum.jumps bad | blank.blank bad | Exact |
| tasks | claude | uikit | 14.5 | 9.7 | 8.04 | 2.97 | 7 | 9 | 6 | 9 | 8 / 3 | 1 / 0 | no-scrollable-list | no-scrollable-list | uikit |
| settings | claude | swiftui | 20.0 | 4.4 | 7.89 | 0.90 | 7 | 8 | 8 | 8 | 3 / 3 | 3 / 2 | edgeBounce.settle tell, tapToStop.stop tell, blank.blank bad | blank.blank bad | swiftui |
