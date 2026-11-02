# Indexed history storage

The automatic-history unit left transcript residency open: each accepted page
was prepended into the shared projection, while the existing thread cache kept
one complete snapshot. This slice adds an isolated range store to the existing
SQLite cache owner. It keeps item identities, ordinals, opaque boundary cursors,
`hasMore`, and a monotonic store revision in separate tables. A ticket combines
the cache, environment, and thread invalidation epochs, so clear, thread removal,
kind clear, environment replacement, and a global clear retire older commits.

Range commits validate duplicate keys and bounded JSON item bodies before one
SQLite transaction writes the rows and metadata. Reads are bounded to 128 items
and can walk older or newer rows from a keyed ordinal anchor. The TypeScript
adapter rejects malformed or oversized requests before native and treats a stale
commit as a normal false result.

This is storage qualification only. The client still retains its full projection,
the display cache still skips expanded history, and no UI eviction or reader
window is enabled. Stable group summaries, live-tail reconciliation, Copy and
selection ownership, disk policy, large-item handling, and actual native memory
acceptance remain separate units.

The automatic mobile paging owner now publishes each accepted page to this
store asynchronously after the display merge. The same route, owner, cursor
and history-revision guard must still be current before the ticket and commit;
storage failure is isolated from the visible page and its Stop or Retry control.
Targeted citation reads and non-mobile clients remain outside this producer
boundary. The projection is still authoritative until a later reader-window
unit proves rehydration and eviction.

The first reader-window step now reopens one indexed older range after a live
thread selection. It merges only while the route, environment, thread and
subscription owner remain current; a stale response or cache failure leaves
the subscribed server snapshot untouched. This restores accepted older rows
without adding a scroll surface or evicting the display projection. Eviction,
live-tail reconciliation, selection pinning and a device memory bound remain
unqualified.

The focused TypeScript adapter tests, standalone SQLite cache tests, and cache
transport tests cover reopen, bidirectional reads, atomic metadata, stale tickets,
clear, malformed input, and transfer framing. No TestFlight, performance, or
complete-history claim follows from this unit.
