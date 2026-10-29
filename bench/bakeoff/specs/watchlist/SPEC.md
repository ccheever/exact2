# Watchlist

An iPhone stock watchlist with simulated live prices.

Data: `data/tickers.json`, 260 tickers (`symbol`, `name`, `price`, `open`, `volume`). Ship it.

1. The main screen lists every ticker: symbol, name, price, and change since `open` in percent, green when up and red when down.
2. Prices move: every second, about 20 random tickers change price by up to ±1 %. The list updates in place, including while it is being scrolled.
3. Sort by symbol, price or change; search by symbol or name.
4. A star adds a ticker to My List; a segmented control switches between All and My List.
5. A ticker's screen shows its price, change, open, volume and a small line chart of its last 60 prices, updating live.
6. My List and the chosen sort survive relaunching the app.
