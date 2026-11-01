# Budget

A shared household budget backed by the frozen REST service at `http://127.0.0.1:{SERVER_PORT}`.

Data: `data/transactions.json`, 120 seeded transactions. The service serves these from `GET /transactions`; it
accepts `POST /transactions` and `PATCH /transactions/:id`, and rejects a write whose amount is over 500 with a
structured error. Ship the data only as a local empty-state fallback; reads come from the service.

1. The main screen loads transactions newest first, grouped by month, with the month total and a running budget total.
   Show a native loading state while reading and a retryable error state when the service is unavailable.
2. Add and edit a transaction in a sheet. A rejected write stays in the form, names the server error and can be fixed
   and retried. A successful write updates the list without a full-screen reset.
3. Include a clear Offline mode control. Writes made offline are queued with a pending marker, survive relaunching,
   and are sent when Offline mode is turned off; show a per-write failure if the server rejects one.
4. Pull to refresh fetches the service again. Delete a transaction with the platform's recoverable row action.
5. Changes from a successful server write survive relaunching; an empty or failed first load never replaces existing
   cached rows with a blank screen.
