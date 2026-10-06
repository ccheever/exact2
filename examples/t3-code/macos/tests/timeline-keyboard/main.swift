// Actual ExactKit focus/event regression for timeline-work.contract's ToolOutput.
// Compile with ExactKit sources and the app's native archive, as in the original
// failing keyboard evidence. Contract compilation separately validates bindings.
import AppKit

_ = NSApplication.shared
let presenter = Presenter()
let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 500),
                      styleMask: [.titled], backing: .buffered, defer: false)
window.contentView = presenter.viewport
let before = NodeView(id: 1, kind: "button", presenter: presenter)
before.handlers = ["press"]
let output = NodeView(id: 2, kind: "view", presenter: presenter)
output.props = ["aria-label": "Tool output"]
output.handlers = ["focus", "blur", "key", "scroll"]
output.applyStyle(["overflow_x": "hidden", "overflow_y": "scroll"])
let after = NodeView(id: 3, kind: "button", presenter: presenter)
after.handlers = ["press"]
for (i, node) in [before, output, after].enumerated() {
    node.frame = NSRect(x: 0, y: CGFloat(i * 100), width: 400, height: 80)
    presenter.root.addSubview(node)
    presenter.views[node.id] = node
}
var focused: [UInt32] = []
var blurred: [UInt32] = []
var keys: [String] = []
presenter.onFocus = { focused.append($0) }
presenter.onBlur = { blurred.append($0) }
presenter.onKey = { id, name in
    precondition(id == output.id)
    keys.append(name)
}
presenter.syncKeyViewLoop()
precondition(output.scroll != nil, "fixture must create actual scroll")
precondition(window.makeFirstResponder(before), "must focus disclosure button")
window.selectNextKeyView(nil)
precondition(window.firstResponder === output, "Tab must reach the result region")
precondition(focused == [output.id], "focus action must show its indicator")
for (characters, keyCode) in [("\u{F701}", UInt16(125)), ("\u{F700}", UInt16(126)), (" ", UInt16(49))] {
    let event = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
                                windowNumber: window.windowNumber, context: nil,
                                characters: characters, charactersIgnoringModifiers: characters,
                                isARepeat: false, keyCode: keyCode)!
    // The session's local key monitor routes a keydown at the first responder.
    _ = presenter.routeKey(event, focused: true, in: window)
}
precondition(keys == ["ArrowDown", "ArrowUp", " "], "scroll keys must reach the Contract action")
window.selectNextKeyView(nil)
precondition(window.firstResponder === after, "Tab must leave the result region")
precondition(blurred == [output.id], "blur action must clear its indicator")
window.selectPreviousKeyView(nil)
precondition(window.firstResponder === output, "Shift-Tab must return to output")
window.selectPreviousKeyView(nil)
precondition(window.firstResponder === before, "Shift-Tab must return to disclosure")
print("PASS: Tab and Shift-Tab traverse disclosure/output/next control; focus, blur and scroll keys delivered")

// The mounted tool row (timeline-work.contract WorkRow): the disclosure button
// holds the tool-icon hook (t3-tool-icon) and the timestamp, which holds the
// tooltip-dismiss hook (t3-timeline-tip). Both hooks carry a `press` handler
// for their native owner, so without `tabindex=-1` each is an invisible Tab
// stop between the disclosure and the output: Tab appears to skip the output.
// The reference timestamp is a plain span (TimelineRowTimestamp, Base UI's
// trigger adds no tabIndex), so it is not a stop either.
func mountedRow(base: UInt32, hookTabIndex: String?, timestampFocusable: Bool) -> (NodeView, NodeView, NodeView, [NodeView]) {
    let row = NodeView(id: base, kind: "button", presenter: presenter)
    row.handlers = ["press", "focus", "blur"]
    let icon = NodeView(id: base + 1, kind: "view", presenter: presenter)
    icon.props = ["aria-hidden": "true"].merging(hookTabIndex.map { ["tabIndex": $0] } ?? [:]) { a, _ in a }
    icon.handlers = ["press"]
    let stamp = NodeView(id: base + 2, kind: "view", presenter: presenter)
    stamp.props = ["aria-label": "October 6, 2026, 10:30 AM"]
    stamp.handlers = timestampFocusable ? ["hover", "focus", "blur"] : ["hover"]
    let tip = NodeView(id: base + 3, kind: "view", presenter: presenter)
    tip.props = ["aria-hidden": "true"].merging(hookTabIndex.map { ["tabIndex": $0] } ?? [:]) { a, _ in a }
    tip.handlers = ["press"]
    let out = NodeView(id: base + 4, kind: "view", presenter: presenter)
    out.handlers = ["focus", "blur", "key", "scroll"]
    out.applyStyle(["overflow_x": "hidden", "overflow_y": "scroll"])
    let next = NodeView(id: base + 5, kind: "button", presenter: presenter)
    next.handlers = ["press"]
    for v in [row, icon, stamp, tip, out, next] { presenter.views[v.id] = v }
    row.frame = NSRect(x: 0, y: 0, width: 400, height: 24)
    icon.frame = NSRect(x: 4, y: 4, width: 16, height: 16)
    stamp.frame = NSRect(x: 300, y: 4, width: 60, height: 16)
    tip.frame = .zero
    out.frame = NSRect(x: 0, y: 30, width: 400, height: 80)
    next.frame = NSRect(x: 0, y: 120, width: 400, height: 24)
    row.container.addSubview(icon)
    row.container.addSubview(stamp)
    stamp.container.addSubview(tip)
    for v in [row, out, next] { presenter.root.addSubview(v) }
    return (row, out, next, [icon, stamp, tip])
}
func tabWalk(from start: NodeView, steps: Int) -> [NSResponder?] {
    precondition(window.makeFirstResponder(start))
    var seen: [NSResponder?] = []
    for _ in 0..<steps { window.selectNextKeyView(nil); seen.append(window.firstResponder) }
    return seen
}
for v in presenter.root.subviews { v.removeFromSuperview() }
presenter.views.removeAll()

// Before: the hooks and the timestamp sit between the disclosure and the output.
let (oldRow, oldOut, _, oldExtras) = mountedRow(base: 10, hookTabIndex: nil, timestampFocusable: true)
presenter.syncKeyViewLoop()
let oldWalk = tabWalk(from: oldRow, steps: 4)
let oldExtraStops = oldWalk.prefix { $0 !== oldOut }.filter { r in oldExtras.contains { $0 === r } }.count
print("Base row: \(oldExtraStops) invisible or non-reference Tab stop(s) before the output")
for v in presenter.root.subviews { v.removeFromSuperview() }
presenter.views.removeAll()

// After: hooks are tabindex=-1 and the timestamp is not focusable.
let (row, out, next, extras) = mountedRow(base: 20, hookTabIndex: "-1", timestampFocusable: false)
presenter.syncKeyViewLoop()
let walk = tabWalk(from: row, steps: 2)
precondition(walk[0] === out, "one Tab from the disclosure must reach the output")
precondition(walk[1] === next, "the next Tab must leave the output")
for v in extras { precondition(!v.canBecomeKeyView, "hook or timestamp \(v.id) must not be a Tab stop") }
window.selectPreviousKeyView(nil)
precondition(window.firstResponder === out, "Shift-Tab returns to the output")
window.selectPreviousKeyView(nil)
precondition(window.firstResponder === row, "Shift-Tab returns to the disclosure")
print("PASS: mounted row: Tab goes disclosure -> output -> next control; hooks and timestamp are not Tab stops")
