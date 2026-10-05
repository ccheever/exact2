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
    output.keyDown(with: event)
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

// The timestamp itself is a focusable text wrapper inside the disclosure.
let timestamp = NodeView(id: 4, kind: "view", presenter: presenter)
timestamp.props = ["aria-label": "October 6, 2026, 10:30 AM"]
timestamp.handlers = ["focus", "blur"]
timestamp.frame = NSRect(x: 250, y: 0, width: 100, height: 20)
before.container.addSubview(timestamp)
presenter.views[timestamp.id] = timestamp
let dismiss = NodeView(id: 5, kind: "view", presenter: presenter)
dismiss.props = ["aria-hidden": "true"]
dismiss.handlers = ["press"]
dismiss.frame = .zero
timestamp.container.addSubview(dismiss)
presenter.views[dismiss.id] = dismiss
presenter.syncKeyViewLoop()
focused.removeAll()
blurred.removeAll()
precondition(window.makeFirstResponder(before))
window.selectNextKeyView(nil)
precondition(window.firstResponder === timestamp, "Tab reaches the timestamp trigger")
precondition(focused == [timestamp.id], "timestamp focus must deliver its action")
window.selectNextKeyView(nil)
let visitsDismiss = window.firstResponder === dismiss
print("Hidden dismissal hook is an extra Tab stop: \(visitsDismiss)")
if visitsDismiss { window.selectNextKeyView(nil) }
precondition(window.firstResponder === output, "Tab continues from timestamp to output")
precondition(blurred == [timestamp.id], "timestamp blur must deliver its action")
window.selectPreviousKeyView(nil)
if window.firstResponder === dismiss { window.selectPreviousKeyView(nil) }
precondition(window.firstResponder === timestamp, "Shift-Tab restores timestamp focus")
print("PASS: timestamp is reachable by Tab and Shift-Tab; focus/blur delivered")
