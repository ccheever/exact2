#!/bin/bash
# cold-series.sh — the crypto list's cold start without DYLD_INSERT_LIBRARIES: bench/heavy-list/cold-series.sh with
# this benchmark's bundle ids (dev.exact.cryptobench.<app>cold, made by ../heavy-list/probe/makecold.sh), apps,
# cold copies (BENCH_COLD, default <checkout>/target/bench/crypto-list/cold) and results (BENCH_RESULTS, default
# <checkout>/target/bench/crypto-list/results; SERIES=cold). coldsum.py there summarizes them.
#   BENCH_DEVICE=<udid> required   APPS="swiftui uikit svgi" (also expo, gpu)   ROUNDS=3   BENCH_LOCK=<command> optional
H=$(cd "$(dirname "$0")" && pwd); ROOT=$(cd "$H/../.." && pwd)
export BENCH_ID_PREFIX=dev.exact.cryptobench. APPS=${APPS:-swiftui uikit svgi} \
  BENCH_COLD=${BENCH_COLD:-$ROOT/target/bench/crypto-list/cold} BENCH_RESULTS=${BENCH_RESULTS:-$ROOT/target/bench/crypto-list/results}
exec "$H/../heavy-list/cold-series.sh" "$@"
