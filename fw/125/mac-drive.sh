printf CLIP | pbcopy
bun exact.mjs agent macos --json "tap editor" "type editor key Meta+v" "tap line" "type line key Meta+v" "tap editor" "type editor key Meta+a" "type editor key Meta+c" "type editor key Meta+x" state "screenshot .exact/mac-$1.png" 2>&1 | grep -o '"slots":{[^}]*}'
echo "pasteboard after: [$(pbpaste)]"
pbcopy < /dev/null
