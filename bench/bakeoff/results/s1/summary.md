# Bake-off s1

exact2 e70dd6f13c. Exact over the other, per pair: minutes **1.70×**, tokens **3.66×** (geometric means, timeouts excluded). Preferred: Exact 1, other 2, tie 0.

## Scrolling

Lists that scroll clean (hitch ratio under 5 ms/s and no offset jumps): Exact **2 of 3**, the other 2 of 3. Mean hitch ratio: Exact 78.9 ms/s, the other 2.3.

| Spec | Other | Exact hitch ms/s | longest ms | jumps | lift drops | lag frames | Other hitch ms/s | longest ms | jumps | lift drops | lag frames |
|---|---|---|---|---|---|---|---|---|---|---|---|
| feed | swiftui | 0.0 | 17 | 0 | 0/4 | 0 | 6.9 | 43 | 3 | 0/4 | 0 |
| photos | uikit | 236.6 | 564 | 23 | 1/4 | 0 | 0.0 | 17 | 0 | 0/4 | – |
| timetable | uikit | 0.0 | 17 | 0 | 0/4 | 0 | 0.0 | 17 | 0 | 0/4 | 0 |

## The pairs

Means, Exact vs other: overall 7.0 vs 7.3; platform 6.3 vs 7.7; tells an app 5.7 vs 4.3; bugs an app 2.3 vs 2.0.

| Spec | Model | Other | Exact min | Other min | Exact Mtok | Other Mtok | Exact overall | Other overall | Exact platform | Other platform | Tells E/O | Bugs E/O | Exact scroll | Other scroll | Better |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| feed | gpt | swiftui | 14.6 | 13.7 | 7.35 | 3.39 | 7 | 8 | 7 | 8 | 6 / 5 | 2 / 2 | ok | smooth.hitch tell, smooth.stall tell, momentum.jumps bad | swiftui |
| photos | claude | uikit | 25.4 | 9.7 | 9.65 | 1.59 | 6 | 8 | 5 | 9 | 7 / 2 | 3 / 1 | smooth.hitch bad, smooth.stall bad, smooth.main bad, tracking.ratio tell, tracking.glide bad, momentum.lift tell, momentum.handoff tell, momentum.jumps bad, scrollToTop.reached bad | tracking.glide bad, scrollToTop.reached tell | uikit |
| timetable | gpt | uikit | 28.6 | 16.1 | 11.53 | 3.11 | 8 | 6 | 7 | 6 | 4 / 6 | 2 / 3 | ok | scrollToTop.reached tell | Exact |
