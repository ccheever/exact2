# Expenses

An iPhone expense tracker.

Data: `data/transactions.json`, 240 transactions over the last 90 days (`id`, `date`, `merchant`, `category`, `amount` in US dollars, `note`). Ship them as the starting history.

1. The main screen lists transactions newest first, grouped by day, each day headed by its date and that day's total.
2. A header or summary shows this month's total spending.
3. Add an expense: amount, merchant, category (from the seven used in the data), date (defaults to today) and an optional note. The amount must be a positive number.
4. Tap a transaction to edit it. Delete with a swipe in the list.
5. A Categories screen shows this month's spending per category, largest first, with a share of the total for each; tapping one lists its transactions.
6. Amounts are shown as US currency. Everything survives relaunching the app.
