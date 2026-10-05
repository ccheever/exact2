import AppKit
func settle() { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.01)) }
let hook = T3ToolActivityIcon()
let host = NSView(frame: NSRect(x: 0, y: 0, width: 16, height: 16))
let element = ExactElement(); element.hook = .t3ToolIcon; element.view = host
let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 2, pixelsHigh: 2, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
let png = bitmap.representation(using: .png, properties: [:])!
let valid = "data:image/png;base64," + png.base64EncodedString()
element.data = [.toolIconLight: valid, .toolIconDark: valid]
hook.install(element); settle()
precondition(element.clicks == 1, "success must notify Contract")
hook.install(element); settle()
precondition(element.clicks == 1, "unchanged success must not toggle again")
element.data = [.toolIconLight: "data:image/png;base64,broken", .toolIconDark: "data:image/png;base64,broken"]
hook.install(element); settle()
precondition(element.clicks == 2, "failed decode must restore fallback through Contract")
element.data = [.toolIconLight: valid, .toolIconDark: valid]
hook.install(element); settle()
precondition(element.clicks == 3, "cached success must notify Contract")
hook.remove(element)
precondition(host.subviews.isEmpty, "unmount cleans native overlay")
let tips = T3TimelineTooltip()
let scroll = NSScrollView(frame: NSRect(x: 0, y: 0, width: 200, height: 100))
let doc = NSView(frame: NSRect(x: 0, y: 0, width: 200, height: 1000)); scroll.documentView = doc
let trigger = NSView(frame: NSRect(x: 0, y: 0, width: 100, height: 20)); doc.addSubview(trigger)
let button = NSView(); trigger.addSubview(button)
let tip = ExactElement(); tip.hook = .t3TimelineTip; tip.view = button
tips.install(tip)
tips.scrolled(scroll.contentView); settle()
precondition(tip.clicks == 0, "unchanged origin cannot dismiss")
scroll.contentView.setBoundsOrigin(NSPoint(x: 0, y: 20)); tips.scrolled(scroll.contentView); settle()
precondition(tip.clicks == 1, "actual scroll dismisses")
tips.scrolled(scroll.contentView); settle()
precondition(tip.clicks == 1, "stationary pointer remains dismissed")
tips.pointerMoved(in: nil); settle()
precondition(tip.clicks == 2, "physical movement releases dismissal")
tips.destroy()
let app = NSApplication.shared
let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
window.contentView = scroll
let focus = NSTextField(frame: NSRect(x: 0, y: 0, width: 80, height: 20))
trigger.addSubview(focus)
precondition(window.makeFirstResponder(focus), "fixture must focus trigger descendant")
let focusedTips = T3TimelineTooltip()
focusedTips.install(tip)
let priorClicks = tip.clicks
scroll.contentView.setBoundsOrigin(NSPoint(x: 0, y: 40)); focusedTips.scrolled(scroll.contentView); settle()
precondition(tip.clicks == priorClicks, "focused trigger must not dismiss")
window.makeFirstResponder(nil)
scroll.contentView.setBoundsOrigin(NSPoint(x: 0, y: 60)); focusedTips.scrolled(scroll.contentView); settle()
precondition(tip.clicks == priorClicks + 1, "blurred trigger dismisses at next actual scroll")
focusedTips.destroy()
print("focused tooltip preservation: 2 checks passed")
print("tool image lifecycle: 5 checks passed; tooltip scroll lifecycle: 4 checks passed")
