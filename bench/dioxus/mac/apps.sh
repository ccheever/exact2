# apps.sh — sourced by series.sh: the three macOS apps build.sh made, newest build of each.
#   EX, EXA   exact2 whole feed: the standalone executable and its captured assets (EXACT_ASSETS; without it no image loads)
#   BX, BXA   exact2 bounded: the same
#   DX_APP    Dioxus Native (vello)
# Any of them may be set beforehand to measure another build.
_B=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
_dev() { # <app dir> <bundle id> -> the newest embedded development directory
  ls -dt "$1"/target/clients/*/"$2"/macos/*/embedded/development 2>/dev/null | head -1
}
_exe() { # <development dir> -> its standalone executable (the one non-library Mach-O there)
  local f; for f in "$1"/standalone/*; do
    case "$f" in *.dylib|*.plist|*.json) continue ;; esac
    [ -f "$f" ] && [ -x "$f" ] && { echo "$f"; return; }
  done
}
_d=$(_dev "$_B/../heavy-list/exact-heavylist" dev.exact.heavybench.exact); : "${EX:=$(_exe "$_d")}" "${EXA:=$_d/capture}"
_d=$(_dev "$_B/exact-bounded" dev.exact.heavybench.bounded); : "${BX:=$(_exe "$_d")}" "${BXA:=$_d/capture}"
: "${DX_APP:=$_B/heavy-dx/target/dx/heavy-dx/release/macos/HeavyDx.app/Contents/MacOS/heavy-dx}"
