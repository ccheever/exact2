# Bake-off r22

exact2 37b662fc24. Exact over the other, per pair: minutes **1.84×**, tokens **3.05×** (geometric means, timeouts excluded). Preferred: Exact 3, other 9, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **6 of 12**, the other 11 of 12. Mean hitch ratio: Exact 4.3 ms/s, the other 1.8.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | Other hitch ms/s | longest ms | jumps | lift drops | lag frames |
|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 1.1 | 33 | 4 | 0/4 | 3 | 4.7 | 33 | 0 | 1/4 | 0 |
| recipes | uikit | 1.2 | 33 | 10 | 0/4 | 0 | 1.3 | 35 | 0 | 0/4 | 0 |
| expenses | uikit | 14.7 | 38 | 29 | 1/4 | 3 | 1.1 | 33 | 0 | 0/4 | 0 |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 3 | 0.0 | 17 | 0 | 0/4 | 0 |
| podcasts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 2.4 | 35 | 0 | 0/4 | 0 |
| notes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |
| watchlist | uikit | 16.6 | 52 | 10 | 0/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |
| chat | swiftui | 0.0 | 17 | 58 | 0/4 | 0 | 11.1 | 98 | 3 | 0/4 | 0 |
| moodboard | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |
| inbox | uikit | 16.3 | 98 | 11 | 0/4 | 3 | 1.2 | 33 | 0 | 0/4 | 0 |
| tasks | uikit | 2.2 | 39 | 0 | 1/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |
| settings | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |

## The pairs

Means, Exact vs other: overall 6.8 vs 7.8; platform 6.8 vs 7.6; tells an app 5.4 vs 4.5; bugs an app 2.2 vs 1.3.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 25.7 | 18.4 | 7.93 | 3.57 | 6 | 7 | 7 | 6 | 6 / 6 | 3 / 3 | tracking.ratio tell, tracking.lag bad, momentum.jumps bad | momentum.lift tell | swiftui |
| recipes | claude | uikit | 17.3 | 8.6 | 3.20 | 1.60 | 7 | 9 | 7 | 9 | 6 / 3 | 2 / 0 | tracking.ratio tell, momentum.jumps bad | smooth.stall tell | uikit |
| expenses | gpt | uikit | 24.6 | 21.3 | 7.38 | 4.48 | 7 | 8 | 8 | 7 | 4 / 5 | 2 / 2 | smooth.hitch bad, smooth.stall tell, tracking.ratio tell, tracking.lag bad, momentum.lift tell, momentum.jumps bad | ok | uikit |
| workout-log | claude | swiftui | 21.2 | 8.5 | 5.06 | 1.78 | 7 | 8 | 8 | 8 | 3 / 4 | 1 / 0 | tracking.ratio tell, tracking.lag bad | ok | swiftui |
| podcasts | gpt | swiftui | 22.7 | 13.5 | 6.11 | 2.50 | 5 | 7 | 4 | 7 | 7 / 5 | 4 / 1 | ok | smooth.stall tell | swiftui |
| notes | claude | uikit | 12.9 | 5.2 | 2.62 | 0.66 | 6 | 9 | 6 | 9 | 7 / 3 | 2 / 0 | tracking.ratio tell | ok | uikit |
| watchlist | gpt | uikit | 27.4 | 16.8 | 6.73 | 3.10 | 7 | 6 | 7 | 5 | 4 / 6 | 2 / 2 | smooth.hitch bad, smooth.stall tell, smooth.main tell, momentum.jumps bad | ok | Exact |
| chat | claude | swiftui | 11.9 | 5.8 | 3.38 | 0.89 | 6 | 9 | 5 | 9 | 7 / 3 | 3 / 0 | momentum.tau tell, momentum.jumps bad, scrollToTop.reached tell | smooth.hitch bad, smooth.stall bad, momentum.jumps bad, scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 15.1 | 9.5 | 4.25 | 1.62 | 8 | 8 | 8 | 7 | 3 / 6 | 1 / 2 | ok | ok | Exact |
| inbox | gpt | uikit | 28.6 | 11.5 | 8.92 | 1.51 | 7 | 6 | 7 | 6 | 5 / 8 | 2 / 2 | smooth.hitch bad, smooth.stall bad, tracking.lag bad, momentum.jumps bad, rubberBand.ratio tell | ok | Exact |
| tasks | claude | uikit | 17.9 | 11.2 | 7.82 | 1.78 | 7 | 9 | 6 | 9 | 8 / 3 | 2 / 2 | smooth.stall tell, momentum.lift tell | ok | uikit |
| settings | claude | swiftui | 12.7 | 5.8 | 4.53 | 0.78 | 8 | 8 | 8 | 9 | 5 / 2 | 2 / 1 | tapToStop.stop bad | ok | swiftui |
