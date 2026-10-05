import AppKit
_ = NSApplication.shared
let presenter = Presenter()
let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 500), styleMask: [.titled], backing: .buffered, defer: false)
window.contentView = presenter.viewport
let before = NodeView(id: 1, kind: "button", presenter: presenter)
before.handlers = ["press"]
let output = NodeView(id: 2, kind: "view", presenter: presenter)
output.props = ["aria-label": "Tool output"]
output.applyStyle(["overflow_x": "hidden", "overflow_y": "scroll"])
let after = NodeView(id: 3, kind: "button", presenter: presenter)
after.handlers = ["press"]
for (i, node) in [before, output, after].enumerated() {
  node.frame = NSRect(x: 0, y: CGFloat(i * 100), width: 400, height: 80)
  presenter.root.addSubview(node)
  presenter.views[node.id] = node
}
presenter.syncKeyViewLoop()
precondition(output.scroll != nil, "fixture must create actual scroll")
precondition(window.makeFirstResponder(before), "fixture must focus disclosure button")
window.selectNextKeyView(nil)
print("output.acceptsFirstResponder=\(output.acceptsFirstResponder)")
print("output.canBecomeKeyView=\(output.canBecomeKeyView)")
print("output.Presenter.tabbable=\(Presenter.tabbable(output))")
print("actual Tab from disclosure reaches node=\((window.firstResponder as? NodeView)?.id ?? 0)")
if window.firstResponder !== output {
  print("FAIL: task requires output scroll reachable by Tab; AppKit skips output to next button")
  exit(1)
}
print("PASS: output scroll reachable by Tab")
