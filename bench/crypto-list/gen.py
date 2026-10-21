#!/usr/bin/env python3
"""Deterministic dataset for the crypto list benchmark (see SPEC.md).
Writes data/coins.json. Run once; every app bundles the output (never regenerate per app)."""
import colorsys, json, math, os, random

OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'data')
os.makedirs(OUT, exist_ok=True)
rng = random.Random(49975)

N = 5000
POINTS = 48
first = ['Vol', 'Aur', 'Zen', 'Nov', 'Cry', 'Lum', 'Pol', 'Ter', 'Sol', 'Hex', 'Kai', 'Mer', 'Qua', 'Rho',
         'Sta', 'Tal', 'Ven', 'Xyl', 'Yor', 'Bel', 'Cor', 'Dra', 'Eon', 'Fal', 'Gal', 'Hyp', 'Ion', 'Jov']
middle = ['', '', 'a', 'o', 'i', 'e', 'ar', 'en', 'or', 'ix', 'um', 'ali', 'eri', 'ona']
last = ['ris', 'tum', 'nex', 'lon', 'dex', 'vra', 'tis', 'coin', 'chain', 'mint', 'bit', 'ium', 'sys', 'net',
        'ora', 'yx', 'is', 'ex', 'ano', 'ium']


def sig(x, n=8):
    """x rounded to n significant digits (keeps the JSON small, values exact enough)."""
    if x == 0:
        return 0.0
    return round(x, n - 1 - int(math.floor(math.log10(abs(x)))))


names, tickers = set(), set()
coins = []
for i in range(N):
    while True:
        name = rng.choice(first) + rng.choice(middle) + rng.choice(last)
        if name not in names:
            names.add(name)
            break
    letters = [c for c in name.upper() if c.isalpha()]
    while True:
        k = rng.randrange(3, 6)
        ticker = letters[0] + ''.join(rng.choice(letters[1:] + list('ABCDEFGHIJKLMNOPQRSTUVWXYZ')) for _ in range(k - 1))
        if ticker not in tickers:
            tickers.add(ticker)
            break
    r, g, b = colorsys.hls_to_rgb(rng.random(), 0.45, 0.6)
    color = '#%02x%02x%02x' % (round(r * 255), round(g * 255), round(b * 255))
    price = math.exp(rng.uniform(math.log(0.0001), math.log(60000)))
    change = rng.uniform(-18, 18)
    # The series is a random walk in log space from the price 24 h ago to the price now
    # (a Brownian bridge), so the chart's direction agrees with change24h.
    start = math.log(price / (1 + change / 100))
    end = math.log(price)
    steps = [rng.gauss(0, 0.012) for _ in range(POINTS - 1)]
    walk = [0.0]
    for s in steps:
        walk.append(walk[-1] + s)
    series = []
    for j in range(POINTS):
        t = j / (POINTS - 1)
        series.append(math.exp(start + (end - start) * t + walk[j] - t * walk[-1]))
    series[-1] = price
    coins.append({
        'id': f'c{i}', 'name': name, 'ticker': ticker, 'color': color,
        'price': sig(price), 'change24h': round(change, 4),
        'series': [sig(v) for v in series],
    })
    coins[-1]['series'][-1] = coins[-1]['price']

json.dump({'version': 1, 'coins': coins}, open(os.path.join(OUT, 'coins.json'), 'w'), separators=(',', ':'))
print('coins', len(coins))
