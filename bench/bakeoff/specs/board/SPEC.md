# Board

A shared kanban board whose frozen WebSocket service is at `ws://127.0.0.1:{SERVER_PORT}`.

Data: `data/board.json`, three columns with seeded cards. The service sends the board on connection, accepts a JSON
`move` message, and broadcasts every accepted move to all connected clients. It also sends a `presence` message when
another client connects or disconnects.

1. Show the three columns Backlog, Doing and Done with cards in their server order. A card shows its title, assignee
   and label. The connection status and the number of other clients are visible but compact.
2. Drag a card within a column or between columns. Update the local board immediately, send the move over the socket,
   and reconcile an incoming move without jumping the card being dragged.
3. Simulate another editor with a visible "Incoming change" button that asks the service to broadcast a move. An
   incoming change while a drag is in progress is applied after the drag ends.
4. Tapping a card opens a detail sheet where its title, assignee and label can be edited. Save over the socket and show
   a retryable error when the connection is lost.
5. Reconnect automatically after a socket loss, show a non-blocking offline state, and keep unsent moves until the
   connection returns. Relaunching restores the last settled board from the service.
