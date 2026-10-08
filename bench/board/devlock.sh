#!/bin/bash
# devlock.sh <device> take|give|who|queue <who> — a queued lock on one shared device (bench lane, 2026-09-29).
# <device> is any name (iphone, ipad, a UDID): one lock per name, under BENCH_LOCK_DIR (default /tmp/exact-bench-lock,
# shared by every process on this Mac).
# A taker files a ticket and may take the lock only when its ticket is first in line, so the fastest poller does not
# win. Tickets are ordered by priority (LOCK_PRIO, default 5; quick A/Bs 3, landing soaks 0), then by filing time.
# A ticket whose waiting process has died is ignored and removed (when it reaches the head, or by `queue`). A holder
# may set LOCK_HOLDER_PID=$$ (its script's PID) when taking; if that process dies while holding, the next waiter
# reclaims the lock.
# As a harness's BENCH_LOCK: BENCH_LOCK="$PWD/bench/board/devlock.sh iphone" makes `$BENCH_LOCK take heavy` a take.
DIR=${BENCH_LOCK_DIR:-/tmp/exact-bench-lock}
case "$1" in ""|*/*) echo "usage: devlock.sh <device> take|give|who|queue <who>" >&2; exit 2 ;; esac
L=$DIR/$1.lock.d; N=$1
Q=$L.queue; mkdir -p "$Q"
head_ticket() {
  for t in $(ls "$Q" 2>/dev/null | sort); do
    pid=$(echo "$t" | cut -d- -f3)
    if kill -0 "$pid" 2>/dev/null; then echo "$t"; return; fi
    rm -f "$Q/$t"
  done
}
case "$2" in
  take)
    T="${LOCK_PRIO:-5}-$(date +%s%N | cut -c1-13)-$$-$3"; : > "$Q/$T"
    trap 'rm -f "$Q/$T"' EXIT; trap 'exit 1' INT TERM HUP
    while :; do
      # A holder that named its script's PID (LOCK_HOLDER_PID=$$ before take) and has died leaves a stale lock: reclaim it.
      hp=$(awk '{print $3}' "$L/owner" 2>/dev/null)
      if [ -n "$hp" ] && ! kill -0 "$hp" 2>/dev/null; then echo "reclaimed stale lock: $(cat "$L/owner")" >&2; rm -rf "$L"; fi
      if [ "$(head_ticket)" = "$T" ] && mkdir "$L" 2>/dev/null; then
        echo "$3 $(date +%T) ${LOCK_HOLDER_PID:-}" > "$L/owner"; rm -f "$Q/$T"; trap - EXIT; echo "$N held by $3"; exit 0
      fi
      sleep 0.3
    done ;;
  give) if [ "$(cut -d' ' -f1 "$L/owner" 2>/dev/null)" = "$3" ]; then rm -rf "$L"; echo "$N free"; else echo "not held by $3: $(cat "$L/owner" 2>/dev/null)"; fi ;;
  who) cat "$L/owner" 2>/dev/null || echo free ;;
  queue) for t in $(ls "$Q" | sort); do kill -0 "$(echo "$t" | cut -d- -f3)" 2>/dev/null && echo "$t" || rm -f "$Q/$t"; done ;;
  *) echo "usage: devlock.sh <device> take|give|who|queue <who>" >&2; exit 2 ;;
esac
