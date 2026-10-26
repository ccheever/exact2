# Bake-off r23

exact2 986e6cf144. Exact over the other, per pair: minutes **1.71×**, tokens **2.26×** (geometric means, timeouts excluded). Preferred: Exact 5, other 7, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **10 of 12**, the other 11 of 12. Mean hitch ratio: Exact 4.4 ms/s, the other 1.5.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | Other hitch ms/s | longest ms | jumps | lift drops | lag frames |
|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 3.4 | 41 | 0 | 0/4 | 0 |
| recipes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 1.2 | 33 | 0 | 0/4 | 0 |
| expenses | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |
| workout-log | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |
| podcasts | swiftui | 1.2 | 33 | 0 | 0/4 | 0 | 1.1 | 33 | 0 | 0/4 | 0 |
| notes | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 1.2 | 33 | 0 | 0/4 | 0 |
| watchlist | uikit | 24.9 | 48 | 9 | 0/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |
| chat | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 1.4 | 35 | 0 | 0/4 | 0 |
| moodboard | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |
| inbox | uikit | 24.2 | 72 | 0 | 0/4 | 0 | 1.2 | 33 | 0 | 0/4 | 0 |
| tasks | uikit | 2.2 | 38 | 0 | 0/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |
| settings | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 9.0 | 101 | 0 | 0/4 | 0 |

## The pairs

Means, Exact vs other: overall 7.0 vs 7.6; platform 6.8 vs 7.3; tells an app 5.5 vs 4.3; bugs an app 1.8 vs 1.4.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| contacts | gpt | swiftui | 27.4 | 19.1 | 9.08 | 7.29 | 6 | 7 | 7 | 7 | 6 / 5 | 2 / 1 | ok | smooth.stall tell | swiftui |
| recipes | claude | uikit | 18.6 | 9.1 | 4.14 | 2.09 | 7 | 9 | 7 | 8 | 7 / 3 | 1 / 0 | ok | ok | uikit |
| expenses | gpt | uikit | 19.2 | 12.4 | 6.41 | 2.44 | 7 | 8 | 8 | 7 | 5 / 5 | 1 / 0 | ok | ok | uikit |
| workout-log | claude | swiftui | 20.0 | 7.5 | 5.59 | 1.52 | 8 | 6 | 7 | 8 | 5 / 3 | 2 / 3 | ok | ok | Exact |
| podcasts | gpt | swiftui | 24.4 | 19.9 | 6.55 | 5.14 | 6 | 8 | 5 | 7 | 8 / 6 | 2 / 2 | ok | ok | swiftui |
| notes | claude | uikit | 11.6 | 6.5 | 2.00 | 1.13 | 6 | 6 | 6 | 7 | 7 / 2 | 3 / 2 | ok | ok | Exact |
| watchlist | gpt | uikit | 23.6 | 14.5 | 6.14 | 3.93 | 8 | 7 | 7 | 6 | 5 / 7 | 2 / 3 | smooth.hitch bad, smooth.stall tell, momentum.jumps bad | ok | Exact |
| chat | claude | swiftui | 15.9 | 5.2 | 4.88 | 1.01 | 7 | 9 | 7 | 9 | 4 / 2 | 1 / 1 | scrollToTop.reached tell | smooth.stall tell, scrollToTop.reached tell | swiftui |
| moodboard | gpt | swiftui | 21.1 | 17.1 | 7.86 | 4.37 | 6 | 8 | 6 | 7 | 5 / 4 | 2 / 1 | ok | ok | swiftui |
| inbox | gpt | uikit | 21.5 | 19.6 | 7.88 | 4.33 | 8 | 7 | 8 | 6 | 4 / 7 | 3 / 1 | smooth.hitch bad, smooth.stall bad, rubberBand.ratio tell | ok | Exact |
| tasks | claude | uikit | 27.4 | 11.0 | 11.49 | 2.26 | 8 | 8 | 7 | 8 | 5 / 5 | 0 / 0 | smooth.stall tell | ok | Exact |
| settings | claude | swiftui | 14.2 | 10.0 | 5.59 | 2.19 | 7 | 8 | 7 | 8 | 5 / 3 | 3 / 3 | ok | smooth.hitch tell, smooth.stall bad, tapToStop.stop bad | swiftui |
