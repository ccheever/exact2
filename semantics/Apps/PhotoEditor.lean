import Contract

def photoEditor : Contract.Program := {
  shapes := [],
  fns := [],
  states := [{ name := "turns", ty := .number, init := (.num 0x0000000000000000), owner := .none, late := false },
    { name := "resets", ty := .number, init := (.num 0x0000000000000000), owner := .none, late := false },
    { name := "edit", ty := .string, init := (.str "scale 1.00× · rotation 0.0° · pan x 0.00 y 0.00 · crop x 0.00 y 0.00 w 1.00 h 1.00"), owner := .none, late := false },
    { name := "edits", ty := .number, init := (.num 0x0000000000000000), owner := .none, late := false },
    { name := "ready", ty := .bool, init := (.bool false), owner := .none, late := false }],
  derives := [],
  resources := [],
  mutations := [],
  actions := [{ name := "rotate", params := [], body := [(.assign "turns" (.binary .add (.var "turns") (.num 0x3ff0000000000000)))] },
    { name := "reset", params := [], body := [(.assign "turns" (.num 0x0000000000000000)), (.assign "resets" (.binary .add (.var "resets") (.num 0x3ff0000000000000)))] },
    { name := "changed", params := [("value", .string)], body := [(.assign "edit" (.var "value"))] },
    { name := "told", params := [("message", .string)], body := [(.ifS (.binary .eq (.var "message") (.str "edited")) [(.assign "edits" (.binary .add (.var "edits") (.num 0x3ff0000000000000)))] [])] },
    { name := "loaded", params := [], body := [(.assign "ready" (.bool true))] }],
  tasks := [],
  view := [(.element "main" [] [("testId", (.str "photo-editor-app")), ("width", (.str "100%")), ("height", (.str "100%")), ("display", (.str "flex")), ("flex-direction", (.str "column")), ("background-color", (.str "#111318")), ("color", (.str "#f2f3f5")), ("font-family", (.str "system-ui"))] [] [(.element "row" [] [("flex-shrink", (.num 0x0000000000000000)), ("align-items", (.str "center")), ("gap", (.num 0x4028000000000000)), ("padding-left", (.num 0x4030000000000000)), ("padding-right", (.num 0x4030000000000000)), ("padding-top", (.num 0x4028000000000000)), ("padding-bottom", (.num 0x4028000000000000))] [] [(.element "text" [(.str "Photo Editor")] [("flex", (.num 0x3ff0000000000000)), ("font-size", (.num 0x4033000000000000)), ("font-weight", (.num 0x4082c00000000000))] [] []), (.element "text" [(.ternary (.var "ready") (.str "Ready") (.str "Loading…"))] [("testId", (.str "status")), ("font-size", (.num 0x402a000000000000)), ("color", (.str "#9aa1ad"))] [] [])]), (.element "photo-editor" [] [("testId", (.str "editor")), ("flex", (.num 0x3ff0000000000000)), ("min-height", (.num 0x0000000000000000)), ("width", (.str "100%")), ("photo", (.str "assets/sample.jpg")), ("turns", (.var "turns")), ("reset", (.var "resets")), ("aria-label", (.str "Photo. Pinch to zoom, turn two fingers to rotate, drag to move, drag the crop corners or edges, double-tap to reset; the buttons below do the same without gestures"))] [("load", "loaded", []), ("change", "changed", []), ("message", "told", [])] []), (.element "column" [] [("flex-shrink", (.num 0x0000000000000000)), ("gap", (.num 0x4024000000000000)), ("padding", (.num 0x4030000000000000)), ("background-color", (.str "#1a1d24"))] [] [(.element "text" [(.str "Edit")] [("font-size", (.num 0x4028000000000000)), ("font-weight", (.num 0x4082c00000000000)), ("color", (.str "#9aa1ad"))] [] []), (.element "text" [(.var "edit")] [("testId", (.str "edit")), ("aria-live", (.str "polite")), ("font-size", (.num 0x402e000000000000)), ("font-variant-numeric", (.str "tabular-nums"))] [] []), (.element "text" [(.binary .add (.binary .add (.str "Edited ") (.call "toString" [(.var "edits")])) (.ternary (.binary .eq (.var "edits") (.num 0x3ff0000000000000)) (.str " time") (.str " times")))] [("testId", (.str "edits")), ("font-size", (.num 0x402a000000000000)), ("color", (.str "#9aa1ad"))] [] []), (.element "row" [] [("gap", (.num 0x4024000000000000))] [] [(.element "button" [] [("testId", (.str "rotate")), ("aria-label", (.str "Rotate the photo 90 degrees clockwise")), ("padding-left", (.num 0x4030000000000000)), ("padding-right", (.num 0x4030000000000000)), ("padding-top", (.num 0x4024000000000000)), ("padding-bottom", (.num 0x4024000000000000)), ("border-radius", (.num 0x4020000000000000)), ("background-color", (.str "#2d6cdf")), ("color", (.str "#ffffff"))] [("press", "rotate", [])] [(.element "text" [(.str "Rotate 90°")] [("font-size", (.num 0x402e000000000000)), ("font-weight", (.num 0x4082c00000000000))] [] [])]), (.element "button" [] [("testId", (.str "reset")), ("aria-label", (.str "Reset zoom, rotation, position and crop")), ("padding-left", (.num 0x4030000000000000)), ("padding-right", (.num 0x4030000000000000)), ("padding-top", (.num 0x4024000000000000)), ("padding-bottom", (.num 0x4024000000000000)), ("border-radius", (.num 0x4020000000000000)), ("background-color", (.str "#2b2f38")), ("color", (.str "#ffffff"))] [("press", "reset", [])] [(.element "text" [(.str "Reset")] [("font-size", (.num 0x402e000000000000)), ("font-weight", (.num 0x4082c00000000000))] [] [])])])])])],
  routes := [],
  router := .none,
  strings := [],
  locale := .none,
  sources := []
}
