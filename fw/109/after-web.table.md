| clock (ms) | `snap` text (tree) | `pending` text | state `snap.n` | state pending ticket |
|---:|---|---|---:|---:|
| 200 | (empty) | pending | 0 | 1 |
| 400 | (empty) | pending | 0 | 1 |
| 600 | reply to ask 1 (asks so far: 1) | pending | 1 | 2 |
| 800 | reply to ask 1 (asks so far: 1) | pending | 1 | 2 |
| 1000 | reply to ask 2 (asks so far: 2) | pending | 2 | 3 |
| 1200 | reply to ask 2 (asks so far: 2) | pending | 2 | 3 |
| 1400 | reply to ask 3 (asks so far: 3) | pending | 3 | 4 |
| 1600 | reply to ask 3 (asks so far: 3) | pending | 3 | 4 |
| 1800 | reply to ask 4 (asks so far: 4) | idle | 4 | - |
| 2000 | reply to ask 4 (asks so far: 4) | idle | 4 | - |
| 3000 | reply to ask 4 (asks so far: 4) | idle | 4 | - |
