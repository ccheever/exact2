import AppKit

// Real AppKit windows and Core Animation, no OS capture or permission request:
// a synthetic CGImage stands in for captured pixels.
private func spin(_ seconds: TimeInterval) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
private func check(_ value: Bool, _ name: String) { guard value else { fatalError(name) } }
private func syntheticImage() -> CGImage {
    let context = CGContext(data: nil, width: 64, height: 40, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.setFillColor(NSColor.systemTeal.cgColor); context.fill(CGRect(x: 0, y: 0, width: 64, height: 40))
    return context.makeImage()!
}
private func window(_ frame: CGRect) -> NSWindow {
    let window = NSWindow(contentRect: frame, styleMask: [.titled], backing: .buffered, defer: false)
    window.isReleasedWhenClosed = false; window.orderFrontRegardless(); return window
}

func runFeedbackChecks() {
    _ = NSApplication.shared; NSApp.setActivationPolicy(.accessory)
    let image = syntheticImage(), bounds = CGRect(x: 40, y: 60, width: 320, height: 200)
    let host = window(CGRect(x: 200, y: 200, width: 400, height: 300))
    let tile = NSView(frame: CGRect(x: 20, y: 20, width: 208, height: 112)); host.contentView!.addSubview(tile)
    let target = T3SnapshotFeedback.Destination(frame: host.convertToScreen(tile.convert(tile.bounds, to: nil)), radius: 8, border: 1, borderColor: .gray, background: .white)
    func feedback(timeout: TimeInterval = 6) -> T3SnapshotFeedback { let value = T3SnapshotFeedback(); value.reduceMotion = { false }; value.timeout = timeout; return value }
    var passed = 0
    func expect(_ value: Bool, _ name: String) { check(value, name); passed += 1 }

    // Landing tracks its panel until the animation completes, reveals first, completes once.
    var order: [String] = [], completions = 0
    let landing = feedback()
    expect(landing.begin(id: "a", image: image, bounds: bounds, flash: true), "flight begins with motion on")
    let panel = landing.flights["a"]!.panel
    expect(panel.isVisible, "overlay panel is ordered in")
    expect(!landing.isAnimating, "a card over its source is not a flight (SnapShotTransition.isAnimating)")
    landing.land(id: "a", target: target, reveal: { order.append("reveal") }) { landed in order.append("done:\(landed)"); completions += 1 }
    expect(landing.isAnimating, "the card flying to its attachment is animating")
    landing.land(id: "a", target: target, reveal: { order.append("reveal-2") }) { landed in order.append("again:\(landed)"); completions += 1 }
    expect(landing.flights["a"] != nil, "landing flight stays tracked until it completes")
    spin(0.05); expect(panel.isVisible && completions == 0, "panel survives while landing")
    spin(1.0)
    expect(order == ["reveal", "done:true", "again:true"], "reveal precedes teardown; a second land joins the same completion: \(order)")
    expect(!panel.isVisible && landing.flights.isEmpty && !landing.isAnimating, "landed panel is closed and released")

    // Destroy and disable close a panel that is mid-landing; completions report cancellation.
    for end in ["destroy", "cancel", "dismiss"] {
        let ending = feedback(); var result: [Bool] = []
        ending.begin(id: "b", image: image, bounds: bounds, flash: false)
        let panel = ending.flights["b"]!.panel
        ending.land(id: "b", target: target) { result.append($0) }
        spin(0.05)
        if end == "destroy" { ending.destroy() } else if end == "cancel" { ending.cancelFlights() } else { ending.dismiss(id: "b") }
        expect(result == [false] && !panel.isVisible && ending.flights.isEmpty, "\(end) closes the landing panel once")
        spin(0.9); expect(result == [false], "late animation completion after \(end) is ignored")
    }

    // Every begun overlay has a bounded deadline, independent of any acknowledgement.
    let bounded = feedback(timeout: 0.2)
    bounded.begin(id: "c", image: image, bounds: bounds, flash: false)
    let unacknowledged = bounded.flights["c"]!.panel
    spin(0.35); expect(!unacknowledged.isVisible && bounded.flights.isEmpty, "unacknowledged overlay ends at its deadline")
    var late: [Bool] = []
    bounded.land(id: "c", target: target) { late.append($0) }
    expect(late == [true], "acknowledging after the deadline completes immediately without an overlay")

    // Identity: a stale flight's deadline or completion never ends a newer flight with the same id.
    let replaced = feedback(timeout: 0.25)
    replaced.begin(id: "d", image: image, bounds: bounds, flash: false)
    let first = replaced.flights["d"]!
    replaced.dismiss(id: "d")
    replaced.timeout = 6; replaced.begin(id: "d", image: image, bounds: bounds, flash: false)
    let second = replaced.flights["d"]!
    spin(0.4); expect(replaced.flights["d"] === second && second.panel.isVisible && first !== second, "old deadline cannot end the replacement flight")
    replaced.begin(id: "e", image: image, bounds: bounds, flash: false)
    expect(replaced.flights["d"] == nil && !second.panel.isVisible, "a newer capture replaces the active transition, as the reference does")
    replaced.destroy()

    // Reduced motion: no flight; acknowledgement completes at once; static flash only.
    let still = feedback(); still.reduceMotion = { true }
    expect(!still.begin(id: "f", image: image, bounds: bounds, flash: true) && still.flights.isEmpty, "reduced motion begins no flight")
    var immediate = 0
    still.land(id: "f", target: target) { _ in immediate += 1 }
    expect(immediate == 1, "reduced-motion acknowledgement completes immediately")

    expect(abs(T3SnapshotFeedback.duration(from: bounds, to: bounds) - 0.28) < 0.001, "zero-distance flight takes the reference minimum 280 ms")
    let far = T3SnapshotFeedback.duration(from: bounds, to: bounds.offsetBy(dx: 100_000, dy: 0))
    expect(far > 0.679 && far <= 0.68, "long flight approaches the reference 680 ms ceiling")
    host.close()
    print("\(passed) capture flight lifetime/deadline/identity checks passed (synthetic image, no OS capture)")
}

func runComposerFocusChecks() {
    _ = NSApplication.shared; NSApp.setActivationPolicy(.accessory)
    let root = FileManager.default.currentDirectoryPath + "/target/t3-tests/snapshot/focus"
    let snap = T3SnapShot(directory: URL(fileURLWithPath: root), agent: true, changed: { _ in })
    let host = window(CGRect(x: 240, y: 240, width: 420, height: 300))
    let composer = NSView(frame: CGRect(x: 20, y: 20, width: 300, height: 80)); host.contentView!.addSubview(composer)
    var focused = 0, passed = 0
    func expect(_ value: Bool, _ name: String) { check(value, name); passed += 1 }
    let owner = "[\"o\",\"env\",\"p\",\"t\"]"
    snap.setComposer(key: ObjectIdentifier(composer), owner: owner, view: composer, focus: { focused += 1 })
    snap.focusComposer(owner: "[\"o\",\"env\",\"p\",\"other\"]"); expect(focused == 0, "another draft's capture never focuses this composer")
    snap.focusComposer(owner: owner); expect(focused == 1, "the painted composer of the original draft is focused")
    let sheet = NSView(frame: host.contentView!.bounds); host.contentView!.addSubview(sheet)
    snap.focusComposer(owner: owner); expect(focused == 1, "a covered composer (settings sheet, dialog) is not focused")
    sheet.removeFromSuperview()
    let field = NSTextView(frame: CGRect(x: 330, y: 20, width: 60, height: 40)); host.contentView!.addSubview(field)
    host.makeFirstResponder(field)
    snap.focusComposer(owner: owner); expect(focused == 1 && host.firstResponder === field, "another field being edited keeps its focus")
    snap.removeComposer(key: ObjectIdentifier(field)); snap.focusComposer(owner: owner)
    expect(focused == 1, "removing an unrelated element keeps the composer registration")
    host.makeFirstResponder(nil)
    snap.removeComposer(key: ObjectIdentifier(composer)); snap.focusComposer(owner: owner); expect(focused == 1, "an ended composer is never focused")
    for action in ["allow-screen-recording", "allow-accessibility", "test-mac-capture"] {
        var reply: [String: Any] = [:]
        snap.perform(["op": "snapshotSetup", "action": action]) { reply = $0 }
        expect(reply["ok"] as? Bool == false && ((reply["error"] as? [String: Any])?["message"] as? String)?.contains("disabled in isolated testing") == true, "isolated \(action) never prompts or captures")
    }
    var requested: [String: Any] = [:]
    snap.perform(["op": "snapshotRequestPermissions", "includeAccessibility": true]) { requested = $0 }
    expect(requested["ok"] as? Bool == true && (requested["value"] as? [String: Any])?["requested"] as? Bool == false, "isolated requestPermissions never prompts, opens Settings or docks a helper")
    var dismissed: [String: Any] = [:]
    snap.perform(["op": "snapshotDismiss", "id": "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA", "owner": owner]) { dismissed = $0 }
    expect(dismissed["ok"] as? Bool == false, "dismissing an unknown capture is refused")
    let wide = CGContext(data: nil, width: 1280, height: 840, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!.makeImage()!
    let thumb = T3SnapShot.composerThumbnail(wide)
    expect(thumb?.width == 256 && thumb?.height == 256, "composer thumbnail is the reference centred 256 px square")
    let small = CGContext(data: nil, width: 120, height: 60, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!.makeImage()!
    expect(T3SnapShot.composerThumbnail(small)?.width == 60, "a short side under 256 px is never upscaled")
    snap.destroy(); host.close()
    print("\(passed) composer-owner focus, setup-guard and thumbnail checks passed")
}

/// Real image layers on a never-shown window; no capture, OS grants, focus or input.
func runImageTileChecks() throws {
    _ = NSApplication.shared
    let root = URL(fileURLWithPath: FileManager.default.currentDirectoryPath + "/target/t3-tests/snapshot/tiles-\(UUID().uuidString)")
    let drafts = root.appendingPathComponent("snapshots/drafts", isDirectory: true)
    try FileManager.default.createDirectory(at: drafts, withIntermediateDirectories: true)
    let firstID = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA", secondID = "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB"
    func write(_ id: String, striped: Bool) throws {
        let pixels = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 120, pixelsHigh: 40, bitsPerSample: 8, samplesPerPixel: 4,
            hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 480, bitsPerPixel: 32)!
        for y in 0..<40 { for x in 0..<120 {
            let color: [UInt8] = striped ? (x < 40 ? [230, 30, 30] : x < 80 ? [30, 200, 30] : [30, 30, 230]) : [200, 30, 200]
            let at = y * 480 + x * 4
            for channel in 0..<3 { pixels.bitmapData![at + channel] = color[channel] }
            pixels.bitmapData![at + 3] = 255
        } }
        try pixels.representation(using: .png, properties: [:])!.write(to: drafts.appendingPathComponent("\(id).png"))
    }
    try write(firstID, striped: true); try write(secondID, striped: false)
    let host = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 500, height: 300), styleMask: .borderless, backing: .buffered, defer: false)
    host.isReleasedWhenClosed = false
    defer { host.close() }
    let snap = T3SnapShot(directory: root, agent: true, changed: { _ in })
    defer { snap.destroy() }
    var views: [NSView] = [], passed = 0
    func expect(_ value: Bool, _ name: String) { check(value, name); passed += 1 }
    func view() -> NSView {
        let value = NSView(frame: CGRect(x: (views.count % 5) * 90, y: (views.count / 5) * 80, width: 64, height: 64))
        host.contentView!.addSubview(value); views.append(value); return value
    }
    func picture(_ view: NSView) -> CGImage? {
        guard let contents = view.subviews.first?.layer?.contents, CFGetTypeID(contents as CFTypeRef) == CGImage.typeID else { return nil }
        return (contents as! CGImage)
    }
    func drain() {
        let deadline = Date().addingTimeInterval(5)
        while snap.tileLoadsInFlight > 0 && Date() < deadline { _ = RunLoop.main.run(mode: .default, before: deadline) }
        expect(snap.tileLoadsInFlight == 0, "all asynchronous image completions drained")
    }
    func green(_ view: NSView) -> Bool {
        guard let image = picture(view), image.width == 40, image.height == 40 else { return false }
        let rep = NSBitmapImageRep(cgImage: image)
        return [5, 20, 35].allSatisfy { x in
            guard let color = rep.colorAt(x: x, y: 20)?.usingColorSpace(.deviceRGB) else { return false }
            return color.greenComponent > 0.6 && color.redComponent < 0.25 && color.blueComponent < 0.25
        }
    }
    let shelf = view(), first = view(), second = view()
    for target in [shelf, first, second] { snap.installTile(id: firstID, view: target) }
    drain()
    expect([shelf, first, second].allSatisfy(green), "three distinct views can paint the same cropped image")
    snap.installTile(id: firstID, view: shelf)
    expect(shelf.subviews.count == 1, "reinstalling one view does not duplicate its image")
    snap.installTile(id: "thumb:\(firstID)", view: first); snap.installTile(id: "thumb:\(firstID)", view: second)
    let preview = view(), contain = view(), repeatedContain = view(), fit = view()
    snap.installTile(id: "preview:\(firstID)", view: preview)
    snap.installTile(id: "contain:\(firstID)", view: contain); snap.installTile(id: "contain:\(firstID)", view: repeatedContain)
    fit.setFrameSize(CGSize(width: 200, height: 100)); snap.installTile(id: "fit:\(firstID)", view: fit)
    drain()
    expect([shelf, first, second].allSatisfy(green), "inline thumbs match the shelf's centred green square")
    for target in [preview, contain, repeatedContain, fit] {
        expect(picture(target)?.width == 120 && picture(target)?.height == 40, "full image modes retain all three stripes")
    }
    expect(preview.subviews.first?.layer?.contentsGravity == .resizeAspect, "preview contains the whole picture")
    expect(fit.subviews.first?.layer?.contentsGravity == .center, "fit keeps small images at their natural size")
    expect(snap.captureTileView(id: firstID) === shelf, "a newer inline or preview never replaces the shelf flight destination")
    snap.removeTile(view: first)
    expect(first.subviews.isEmpty && green(second) && green(shelf) && picture(preview) != nil, "removing one inline view preserves every sibling")
    snap.removeTile(view: shelf)
    expect(snap.captureTileView(id: firstID) == nil, "inline and preview pictures are never fallback capture destinations")
    let replaced = view()
    snap.installTile(id: "thumb:\(firstID)", view: replaced)
    snap.installTile(id: "thumb:\(secondID)", view: replaced)
    let removed = view(); snap.installTile(id: firstID, view: removed); snap.removeTile(view: removed)
    drain()
    let color = picture(replaced).flatMap { NSBitmapImageRep(cgImage: $0).colorAt(x: 20, y: 20)?.usingColorSpace(.deviceRGB) }
    expect(color.map { $0.redComponent > 0.6 && $0.greenComponent < 0.25 && $0.blueComponent > 0.6 } == true,
        "a stale first decode cannot overwrite the replacement view's image")
    expect(replaced.subviews.count == 1 && removed.subviews.isEmpty, "queued replacement and removal do not resurrect old image views")
    let ending = view(); snap.installTile(id: firstID, view: ending); snap.destroy(); drain()
    expect(views.allSatisfy { $0.subviews.isEmpty }, "destroy removes every picture and ignores its pending decode")
    expect(!host.isVisible, "tile checks never order a window or disturb an active app")
    print("\(passed) off-screen AppKit repeated-image/crop/removal/stale-completion checks passed")
}
