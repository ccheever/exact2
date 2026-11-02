#!/usr/bin/env bash
# run.sh [harness options]: one round of agents (default: five Exact agents at exact2's
# origin/main), measured and reported. N, OUT, FRAMEWORKS, EXACT2_REF, HARNESS and RUNS
# override the defaults below.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
HARNESS=${HARNESS:-$HOME/bench-afb/harness} RUNS=${RUNS:-$HOME/bench-afb/runs}
export EXACT2_DIR=${EXACT2_DIR:-$(cd "$HERE/../.." && pwd)} EXACT2_REF=${EXACT2_REF:-origin/main}
export PATH=$HERE/bin:$PATH DOCKER_CONTEXT=${DOCKER_CONTEXT:-colima-bench}

# Without an API key or a setup-token, agents use the host's Claude Code login: each gets
# only its access token, read again per agent, and the harness refuses one with under an
# hour left. Five agents take about 50 minutes, so the round waits until the token has
# 110; Claude Code running on the host refreshes it. The refresh token never leaves the host.
if [ -z "${ANTHROPIC_API_KEY:-}${CLAUDE_CODE_OAUTH_TOKEN:-}" ]; then
  export CLAUDE_CREDENTIALS_FILE=${CLAUDE_CREDENTIALS_FILE:-$HOME/.claude/.credentials.json}
  left() { node -e 'const f = process.argv[1]; console.log(Math.floor((JSON.parse(require("fs").readFileSync(f, "utf8")).claudeAiOauth.expiresAt - Date.now()) / 60000))' "$CLAUDE_CREDENTIALS_FILE"; }
  until [ "$(left)" -gt "${MIN_TOKEN_MINUTES:-110}" ]; do sleep 60; done
fi

# Each round builds new images (~10 GB); untagged ones would fill the VM's disk.
docker image prune -f >/dev/null
git -C "$EXACT2_DIR" fetch -q
cd "$HARNESS"
exec node bench/run.mjs --frameworks="${FRAMEWORKS:-exact}" --iterations="${N:-5}" --effort=high --out="$RUNS/${OUT:-exact-agents}" "$@"
