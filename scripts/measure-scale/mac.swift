// Stage 11 B's measured cells, the Mac (scripts/measure-scale.mjs runs it): AppKit's own spacing and corners,
// read from its API where it says them and from the inked shape where it only draws them. Prints one
// `measure <id> <value>` line each.
import AppKit
import SwiftUI

func say(_ id: String, _ v: CGFloat, _ note: String = "") { print("measure \(id) \(String(format: "%.2f", Double(v)))\(note.isEmpty ? "" : " # \(note)")"); fflush(stdout) }

/// The window server's picture of our own window (the SDK marks the call unavailable, so it is looked up at
/// run time, as AgentMac.swift's `ownWindowImage` does; the window server still answers it for our own window).
func windowImage(_ window: NSWindow) -> CGImage? {
  typealias Create = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
  guard let symbol = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGWindowListCreateImage") else { return nil }
  return unsafeBitCast(symbol, to: Create.self)(.null, 1 << 3, UInt32(window.windowNumber), 1 << 0 | 1 << 3)?.takeRetainedValue()
}

/// The equivalent circular radius of the top-left corner of what `view` draws, in points, from the window
/// server's composite on a flat white ground: ink is the distance from the ground's colour; the box by any
/// ink, the edge at half the level of the top edge's middle, along the 45° diagonal: r = d / (√2 − 1).
func cornerRadius(_ view: NSView, label: String) {
  window.orderFrontRegardless(); view.needsDisplay = true; view.displayIfNeeded()
  RunLoop.current.run(until: Date().addingTimeInterval(0.4))
  guard let image = windowImage(window) else { print("measure \(label) none # no capture"); return }
  if let dir = ProcessInfo.processInfo.environment["MEASURE_PNG_DIR"], let dest = CGImageDestinationCreateWithURL(URL(fileURLWithPath: "\(dir)/\(label).png") as CFURL, "public.png" as CFString, 1, nil) { CGImageDestinationAddImage(dest, image, nil); CGImageDestinationFinalize(dest) }
  let scale = CGFloat(image.width) / window.frame.width
  let w = image.width, h = image.height
  var data = [UInt8](repeating: 0, count: w * h * 4)
  let c = CGContext(data: &data, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
  c.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
  // The view's frame in the image's pixels (the image's top is the window's top, its content below the title bar).
  let f = view.convert(view.bounds, to: nil)
  let content = window.contentRect(forFrameRect: window.frame), titleBar = window.frame.height - content.height
  let x0 = Int((f.minX - 4) * scale), x1 = Int((f.maxX + 4) * scale)
  let y0 = Int((titleBar + content.height - f.maxY - 4) * scale), y1 = Int((titleBar + content.height - f.minY + 4) * scale)
  let ground = (data[(y0 * w + x0) * 4], data[(y0 * w + x0) * 4 + 1], data[(y0 * w + x0) * 4 + 2])
  func a(_ x: Int, _ y: Int) -> CGFloat {
    let p = (y * w + x) * 4
    return CGFloat(max(abs(Int(data[p]) - Int(ground.0)), abs(Int(data[p + 1]) - Int(ground.1)), abs(Int(data[p + 2]) - Int(ground.2)))) / 255
  }
  var minX = x1, minY = y1, maxX = -1, maxY = -1
  for y in max(0, y0)..<min(h, y1) { for x in max(0, x0)..<min(w, x1) where a(x, y) > 0.04 { minX = min(minX, x); maxX = max(maxX, x); minY = min(minY, y); maxY = max(maxY, y) } }
  guard maxX >= 0 else { print("measure \(label) none"); return }
  let level = a((minX + maxX) / 2, minY + Int(scale)), half = level / 2
  var k: CGFloat = 0, prev: CGFloat = 0
  for i in 0..<min(maxX - minX, maxY - minY) {
    let v = a(minX + i, minY + i)
    if v >= half { k = CGFloat(i) - 1 + (half - prev) / max(v - prev, 0.001); break }
    prev = v
  }
  let d = (k + 0.5) * 2.0.squareRoot()
  say(label, d / (2.0.squareRoot() - 1) / scale, "inked \(String(format: "%.1f", Double(maxX - minX + 1) / scale))x\(String(format: "%.1f", Double(maxY - minY + 1) / scale)) pt, edge level \(String(format: "%.2f", Double(level)))")
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
print("os macOS \(ProcessInfo.processInfo.operatingSystemVersionString)")
let window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 800, height: 600), styleMask: [.titled], backing: .buffered, defer: false)
window.backgroundColor = NSColor(srgbRed: 1, green: 1, blue: 1, alpha: 1)
window.appearance = NSAppearance(named: .aqua)
let host = window.contentView!
// System spacing between two views, horizontally and vertically.
func spacing(_ a: NSView, _ b: NSView, vertical: Bool) -> CGFloat {
  [a, b].forEach { $0.translatesAutoresizingMaskIntoConstraints = false; host.addSubview($0) }
  var cs = [a.leadingAnchor.constraint(equalTo: host.leadingAnchor, constant: 40), a.topAnchor.constraint(equalTo: host.topAnchor, constant: 100)]
  cs += vertical ? [b.topAnchor.constraint(equalToSystemSpacingBelow: a.bottomAnchor, multiplier: 1), b.leadingAnchor.constraint(equalTo: a.leadingAnchor)]
                 : [b.leadingAnchor.constraint(equalToSystemSpacingAfter: a.trailingAnchor, multiplier: 1), b.centerYAnchor.constraint(equalTo: a.centerYAnchor)]
  NSLayoutConstraint.activate(cs); host.layoutSubtreeIfNeeded(); defer { a.removeFromSuperview(); b.removeFromSuperview() }
  // By alignment rects, as the constraint is: a label's frame holds its cell's inset.
  let ra = a.alignmentRect(forFrame: a.frame), rb = b.alignmentRect(forFrame: b.frame)
  return vertical ? ra.minY - rb.maxY : rb.minX - ra.maxX
}
say("system-spacing-labels", spacing(NSTextField(labelWithString: "A"), NSTextField(labelWithString: "B"), vertical: false))
say("system-spacing-buttons", spacing(NSButton(title: "A", target: nil, action: nil), NSButton(title: "B", target: nil, action: nil), vertical: false))
say("system-spacing-below-labels", spacing(NSTextField(labelWithString: "A"), NSTextField(labelWithString: "B"), vertical: true))
do {
  // A view at the system spacing from its container's edge (Interface Builder's standard margin to the window).
  let v = NSTextField(labelWithString: "A"); v.translatesAutoresizingMaskIntoConstraints = false; host.addSubview(v)
  NSLayoutConstraint.activate([v.leadingAnchor.constraint(equalToSystemSpacingAfter: host.leadingAnchor, multiplier: 1), v.topAnchor.constraint(equalToSystemSpacingBelow: host.topAnchor, multiplier: 1)])
  host.layoutSubtreeIfNeeded(); let r = v.alignmentRect(forFrame: v.frame); say("system-spacing-to-container-leading", r.minX); say("system-spacing-to-container-top", host.frame.height - r.maxY); v.removeFromSuperview()
}
say("stack-default-spacing", NSStackView().spacing)
say("grid-default-row-spacing", NSGridView().rowSpacing)
say("grid-default-column-spacing", NSGridView().columnSpacing)
say("box-content-margin", NSBox().contentViewMargins.width)
say("table-intercell-width", NSTableView().intercellSpacing.width)
say("table-intercell-height", NSTableView().intercellSpacing.height)
// Corners as drawn: push buttons by size, a checkbox's box, a text field's bezel, a segmented control.
for (name, size) in [("mini", NSControl.ControlSize.mini), ("small", .small), ("regular", .regular), ("large", .large), ("extra-large", .extraLarge)] {
  let b = NSButton(title: "Button", target: nil, action: nil); b.bezelStyle = .push; b.controlSize = size
  b.sizeToFit(); b.frame.origin = NSPoint(x: 100, y: 300); host.addSubview(b); b.layoutSubtreeIfNeeded()
  cornerRadius(b, label: "push-\(name)-corner"); b.removeFromSuperview()
}
do {
  let c = NSButton(checkboxWithTitle: "", target: nil, action: nil); c.sizeToFit(); c.frame.origin = NSPoint(x: 100, y: 300); host.addSubview(c)
  cornerRadius(c, label: "checkbox-regular-corner"); c.removeFromSuperview()
  let f = NSTextField(string: "Field"); f.bezelStyle = .roundedBezel; f.frame = NSRect(x: 100, y: 300, width: 160, height: 22); host.addSubview(f)
  cornerRadius(f, label: "field-rounded-corner"); f.removeFromSuperview()
  let s = NSSegmentedControl(labels: ["One", "Two"], trackingMode: .selectOne, target: nil, action: nil); s.sizeToFit(); s.frame.origin = NSPoint(x: 100, y: 300); host.addSubview(s)
  cornerRadius(s, label: "segmented-regular-corner"); s.removeFromSuperview()
  let p = NSPopUpButton(frame: .zero, pullsDown: false); p.addItem(withTitle: "Choice"); p.sizeToFit(); p.frame.origin = NSPoint(x: 100, y: 300); host.addSubview(p)
  cornerRadius(p, label: "popup-regular-corner"); p.removeFromSuperview()
}
// SwiftUI's grouped Form: the gap between two sections' rows (iOS's inset-grouped section gap's role).
final class Frames: ObservableObject { var rows: [Int: CGRect] = [:] }
let frames = Frames()
struct Row: View {
  let i: Int
  var body: some View { Text("Row \(i)").background(GeometryReader { g in Color.clear.onAppear { frames.rows[i] = g.frame(in: .global) }.onChange(of: g.frame(in: .global)) { _, f in frames.rows[i] = f } }) }
}
struct Sections: View { var body: some View { Form { Section { Row(i: 0) }; Section { Row(i: 1) } }.formStyle(.grouped) } }
do {
  let hosting = NSHostingView(rootView: Sections()); hosting.frame = NSRect(x: 0, y: 0, width: 600, height: 400)
  host.addSubview(hosting); window.orderFrontRegardless(); hosting.layoutSubtreeIfNeeded()
  RunLoop.current.run(until: Date().addingTimeInterval(0.6))
  if let a = frames.rows[0], let b = frames.rows[1] {
    // Row to row; less the Mac's 36-point grouped row, the gap between headerless sections (10).
    say("form-grouped-row-to-row", b.minY - a.minY, "row text \(a.height) pt")
  }
  hosting.removeFromSuperview()
}
print("done")
