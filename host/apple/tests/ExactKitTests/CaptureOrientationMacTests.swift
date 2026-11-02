#if os(macOS)
import AppKit
import XCTest
import IOSurface
@testable import ExactKit

private class OrientationView: NSView {
    var flipsCoordinates = true
    override var isFlipped: Bool { flipsCoordinates }
}

private final class OrientationContainer: OrientationView {
    override var wantsUpdateLayer: Bool { true }
    override func updateLayer() {}
}

final class CaptureDisplayWindow: NSWindow {
    var displays = 0
    var displayTime = 0.0
    var duringDisplay: (() -> Void)?
    override func displayIfNeeded() {
        displays += 1
        duringDisplay?()
        let start = CACurrentMediaTime()
        super.displayIfNeeded()
        displayTime += CACurrentMediaTime() - start
    }
}

private final class CaptureFieldEditor: NSTextView {
    var backingWrites = 0
    override var wantsLayer: Bool {
        get { super.wantsLayer }
        set { backingWrites += 1; super.wantsLayer = newValue }
    }
}

private final class CaptureEditingWindow: NSWindow {
    let editor = CaptureFieldEditor()
    override func fieldEditor(_ createFlag: Bool, for object: Any?) -> NSText? {
        editor.isFieldEditor = true
        return editor
    }
}

private final class OrientationInk: OrientationView {
    var draws = 0
    var drawnCTM = CGAffineTransform.identity
    override func draw(_ dirtyRect: NSRect) {
        draws += 1
        drawnCTM = NSGraphicsContext.current!.cgContext.ctm
        NSColor.white.setFill(); bounds.fill()
        NSColor.red.setFill()
        NSRect(x: 0, y: isFlipped ? 0 : 48, width: 20, height: 8).fill()
        NSColor.blue.setFill()
        NSRect(x: 0, y: isFlipped ? 48 : 0, width: 20, height: 8).fill()
        ("Flip Fg" as NSString).draw(at: NSPoint(x: 26, y: isFlipped ? 3 : 29), withAttributes: [
            .font: NSFont.systemFont(ofSize: 20), .foregroundColor: NSColor.black,
        ])
    }
}

private final class OrientationUpdate: OrientationView {
    var image: Any?
    override var wantsUpdateLayer: Bool { true }
    override func updateLayer() { layer?.contents = image }
}

// Copies keep each backing store's live flip, including a subtree whose
// ancestors are outside the capture and a newly mounted, detached layer.
final class CaptureOrientationMacTests: XCTestCase {
    func testCaptureDoesNotBackTheInstalledFieldEditor() throws {
        _ = NSApplication.shared
        let root = OrientationView(frame: NSRect(x: 0, y: 0, width: 480, height: 112))
        root.wantsLayer = true
        let window = CaptureEditingWindow(contentRect: root.bounds, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = root
        window.appearance = NSAppearance(named: .aqua)
        defer { window.close() }
        window.makeKeyAndOrderFront(nil); window.displayIfNeeded()
        let field = NSTextField(frame: NSRect(x: 10, y: 10, width: 200, height: 40))
        field.isBezeled = false; field.drawsBackground = false
        field.font = .systemFont(ofSize: 20); field.textColor = .black
        field.stringValue = "Palo Fg"
        root.addSubview(field)
        XCTAssertTrue(window.makeFirstResponder(field))
        let editor = try XCTUnwrap(field.currentEditor() as? NSTextView)
        let selection = NSRange(location: 1, length: 3)
        editor.selectedRange = selection
        XCTAssertTrue(editor.isFieldEditor)
        XCTAssertNil(editor.layer, "capture starts before the editor inherits backing")
        window.editor.backingWrites = 0
        _ = Capture.paintOrderLayer(of: root, root: try XCTUnwrap(root.layer))
        XCTAssertEqual(window.editor.backingWrites, 0, "copying must not change the installed editor's backing policy")
        let rep = try XCTUnwrap(Capture.bitmap(of: root, scale: 1))
        XCTAssertTrue(window.firstResponder === editor)
        XCTAssertTrue(field.currentEditor() === editor)
        XCTAssertEqual(editor.selectedRange, selection)
        let rect = root.convert(editor.bounds, from: editor)
        XCTAssertGreaterThan(glyphs(rep, in: rect).ink, 20, "editing text is still drawn")
    }

    func testWarmCaptureDoesNotDisplayTheWindow() throws {
        _ = NSApplication.shared
        let root = OrientationView(frame: NSRect(x: 0, y: 0, width: 600, height: 560))
        root.wantsLayer = true
        let window = CaptureDisplayWindow(contentRect: root.bounds, styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = root
        defer { window.close() }
        for i in 0..<50 {
            root.addSubview(OrientationInk(frame: NSRect(x: (i % 5) * 120, y: (i / 5) * 56, width: 120, height: 56)))
        }
        let field = NSTextField(frame: NSRect(x: 25, y: 25, width: 90, height: 24))
        field.stringValue = "Palo Fg"
        root.subviews[0].addSubview(field)
        window.makeKeyAndOrderFront(nil)
        XCTAssertTrue(window.makeFirstResponder(field))
        let editor = try XCTUnwrap(field.currentEditor())
        editor.selectedRange = NSRange(location: 2, length: 2)
        window.displayIfNeeded(); CATransaction.flush()
        for each in [false, true] {
            var times: [Double] = []
            root.alphaValue = each ? 1 : 0
            window.displays = 0; window.displayTime = 0
            for _ in 0..<30 {
                let start = CACurrentMediaTime()
                if each {
                    for child in root.subviews { _ = try XCTUnwrap(Capture.bitmap(of: child, scale: 1)) }
                } else { _ = try XCTUnwrap(Capture.bitmap(of: root, scale: 1)) }
                times.append((CACurrentMediaTime() - start) * 1000)
            }
            times.sort()
            print("CAPTURE 50 children each=\(each) median_ms=\(times[15]) p95_ms=\(times[28]) window_displays_per_frame=\(Double(window.displays) / 30) window_display_ms=\(window.displayTime * 1000 / 30)")
            XCTAssertTrue(window.firstResponder === editor)
            XCTAssertEqual(editor.selectedRange, NSRange(location: 2, length: 2))
            XCTAssertEqual(window.displays, 0, "an attached subtree needs no window display")
        }
    }

    func testNewRowsInHiddenOverlay() throws { try newRows(firstChild: false) }
    func testNewRowsFirstCapturedPerChild() throws { try newRows(firstChild: true) }

    private func newRows(firstChild: Bool) throws {
        _ = NSApplication.shared
        let shell = NSView(frame: NSRect(x: 0, y: 0, width: 240, height: 112))
        shell.wantsLayer = true
        let root = OrientationView(frame: shell.bounds); shell.addSubview(root)
        let scroll = NSScrollView(frame: root.bounds)
        let document = OrientationView(frame: root.bounds)
        scroll.documentView = document; root.addSubview(scroll)
        let header = OrientationInk(frame: NSRect(x: 0, y: 0, width: 120, height: 56))
        document.addSubview(header)
        let window = CaptureDisplayWindow(contentRect: shell.bounds, styleMask: [.borderless], backing: .buffered, defer: false)
        // Light, as the other captures here: the scroll view draws the window's background colour behind the rows,
        // which Dark Mode makes dark, and the placeholder's glyph test reads dark pixels as ink.
        window.appearance = NSAppearance(named: .aqua)
        window.isReleasedWhenClosed = false; window.contentView = shell
        defer { window.close() }
        window.orderFront(nil); shell.displayIfNeeded(); CATransaction.flush()
        root.alphaValue = 0
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let card = OrientationContainer(frame: NSRect(x: 120, y: 0, width: 120, height: 112))
        card.wantsLayer = true
        let row = OrientationInk(frame: NSRect(x: 0, y: 0, width: 120, height: 56))
        row.wantsLayer = true
        card.addSubview(row)
        let field = NSTextField(frame: NSRect(x: 0, y: 56, width: 120, height: 40))
        field.isBezeled = false; field.isBordered = false; field.drawsBackground = false
        field.placeholderAttributedString = NSAttributedString(string: "Search", attributes: [.foregroundColor: NSColor.black])
        card.addSubview(field)
        document.addSubview(card)
        // The views are mounted, but AppKit has not joined their layers yet.
        XCTAssertNil(card.layer?.superlayer); XCTAssertNil(row.layer?.superlayer)
        XCTAssertNil(field.layer)
        window.displays = 0
        window.duringDisplay = { XCTAssertEqual(root.alphaValue, 0, "attach layers before exposing the overlay") }
        let childFirst = firstChild ? try XCTUnwrap(Capture.bitmap(of: card, scale: 1)) : nil
        let rep = try XCTUnwrap(Capture.bitmap(of: root, scale: 1))
        let child = try childFirst ?? XCTUnwrap(Capture.bitmap(of: card, scale: 1))
        for (picture, x) in [(rep, 4), (rep, 124), (child, 4)] {
            let top = try XCTUnwrap(picture.colorAt(x: x, y: 4)), bottom = try XCTUnwrap(picture.colorAt(x: x, y: 52))
            XCTAssertGreaterThan(top.alphaComponent, 0.9)
            XCTAssertGreaterThan(top.redComponent, 0.9); XCTAssertLessThan(top.blueComponent, 0.1)
            XCTAssertGreaterThan(bottom.alphaComponent, 0.9)
            XCTAssertGreaterThan(bottom.blueComponent, 0.9); XCTAssertLessThan(bottom.redComponent, 0.1)
        }
        XCTAssertLessThanOrEqual(window.displays, 1, "only a new backing tree needs a window display")
        window.duringDisplay = nil
        let reference = try XCTUnwrap(field.bitmapImageRepForCachingDisplay(in: field.bounds))
        field.cacheDisplay(in: field.bounds, to: reference)
        let expected = glyphs(reference, in: field.bounds)
        for (picture, container) in [(rep, root), (child, card)] {
            let actual = glyphs(picture, in: container.convert(field.bounds, from: field))
            XCTAssertGreaterThan(actual.ink, 20, "the newly mounted field's placeholder is captured")
            XCTAssertLessThan(glyphDifference(actual.mask, expected.mask), glyphDifference(actual.mask, expected.mask, flipped: true),
                              "the placeholder has AppKit's upright orientation")
        }
    }

    func testHeaderBesideScrollRows() throws {
        _ = NSApplication.shared
        let shell = OrientationView(frame: NSRect(x: 0, y: 0, width: 480, height: 112))
        shell.wantsLayer = true
        let root = OrientationView(frame: shell.bounds); shell.addSubview(root)
        let header = OrientationInk(frame: NSRect(x: 0, y: 0, width: 120, height: 56))
        root.addSubview(header)
        let scroll = NSScrollView(frame: NSRect(x: 120, y: 0, width: 120, height: 112))
        let document = OrientationView(frame: NSRect(x: 0, y: 0, width: 120, height: 224))
        let first = OrientationInk(frame: NSRect(x: 0, y: 0, width: 120, height: 56))
        let second = OrientationInk(frame: NSRect(x: 0, y: 56, width: 120, height: 56))
        document.addSubview(first); document.addSubview(second)
        scroll.documentView = document; root.addSubview(scroll)
        let context = try XCTUnwrap(CGContext(data: nil, width: 120, height: 56, bitsPerComponent: 8, bytesPerRow: 480,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(NSColor.white.cgColor); context.fill(CGRect(x: 0, y: 0, width: 120, height: 56))
        context.setFillColor(NSColor.red.cgColor); context.fill(CGRect(x: 0, y: 48, width: 20, height: 8))
        context.setFillColor(NSColor.blue.cgColor); context.fill(CGRect(x: 0, y: 0, width: 20, height: 8))
        let image = try XCTUnwrap(context.makeImage())
        let train = NSImageView(frame: NSRect(x: 240, y: 0, width: 120, height: 56))
        train.image = NSImage(cgImage: image, size: train.bounds.size); root.addSubview(train)
        let direct = OrientationUpdate(frame: NSRect(x: 240, y: 56, width: 120, height: 56))
        direct.image = image; root.addSubview(direct)
        let field = NSTextField(frame: NSRect(x: 360, y: 0, width: 120, height: 56))
        field.stringValue = "Palo Fg"; field.font = .systemFont(ofSize: 20)
        root.addSubview(field)
        let window = NSWindow(contentRect: shell.bounds, styleMask: [.titled], backing: .buffered, defer: false)
        window.appearance = NSAppearance(named: .aqua)
        window.isReleasedWhenClosed = false; window.contentView = shell
        defer { window.close() }
        window.makeKeyAndOrderFront(nil)
        window.makeFirstResponder(field)
        let editor = try XCTUnwrap(field.currentEditor())
        XCTAssertTrue(window.firstResponder === editor)
        editor.selectedRange = NSRange(location: field.stringValue.count, length: 0)
        shell.layoutSubtreeIfNeeded(); shell.displayIfNeeded(); CATransaction.flush()
        func chain(_ view: NSView) -> String {
            var views: [String] = [], layers: [String] = []
            var v: NSView? = view
            while let item = v {
                views.append("\(type(of: item)):\(item.isFlipped)\(item === root ? "[capture]" : "")")
                v = item.superview
            }
            var layer = view.layer
            while let item = layer {
                layers.append("\(type(of: item)):\(item.isGeometryFlipped)\(item === root.layer ? "[capture]" : "")")
                layer = item.superlayer
            }
            return "views=\(views) layers=\(layers)"
        }
        for (name, view) in [("header", header), ("row", first), ("image", train), ("direct", direct), ("editor", editor)] {
            print("MIXED CHAIN \(name) \(chain(view))")
        }
        let backing = try XCTUnwrap(root.layer)
        let drawsBefore = [header.draws, first.draws, second.draws]
        let copy = try XCTUnwrap(Capture.paintOrderBitmap(of: root, scale: 1))
        let drawsAfterCopy = [header.draws, first.draws, second.draws]
        let original = NSBitmapImageRep(cgImage: try XCTUnwrap(Capture.picture(of: backing, size: root.bounds.size, scale: 1)))
        print("MIXED DRAWS before=\(drawsBefore) afterCopy=\(drawsAfterCopy) afterOriginal=\([header.draws, first.draws, second.draws])")
        let cache = try XCTUnwrap(root.bitmapImageRepForCachingDisplay(in: root.bounds))
        root.cacheDisplay(in: root.bounds, to: cache)
        // Crop the first word inside the editor, excluding the bezel and caret.
        var glyphRect = root.convert(editor.bounds, from: editor)
        glyphRect.size.width = min(glyphRect.width, 40)
        let reference = glyphs(cache, in: glyphRect)
        XCTAssertGreaterThan(reference.ink, 30, "AppKit reference contains editor glyphs")
        for (name, sample) in [("copy", glyphs(copy, in: glyphRect)), ("original", glyphs(original, in: glyphRect))] {
            XCTAssertGreaterThan(sample.ink, 30, "\(name) retains the field editor's text")
            let upright = glyphDifference(sample.mask, reference.mask)
            let flipped = glyphDifference(sample.mask, reference.mask, flipped: true)
            print("EDITOR \(name) ink=\(sample.ink) referenceInk=\(reference.ink) uprightError=\(upright) flippedError=\(flipped)")
            XCTAssertLessThan(min(upright, flipped), 0.2, "\(name) contains the reference glyphs")
            XCTAssertLessThan(upright, flipped, "\(name) editor orientation agrees with AppKit")
        }
        for (name, i) in [("header", 0), ("row1", 1), ("row2", 5), ("image", 2), ("direct", 6)] {
            print("MIXED \(name) copy=\(marker(copy, i)) original=\(marker(original, i)) cache=\(marker(cache, i))")
            XCTAssertEqual(marker(original, i), "upright", name)
            XCTAssertEqual(marker(cache, i), "upright", name)
            XCTAssertEqual(marker(copy, i), "upright", name)
            assertMarkerPairExists(copy, i)
        }
        try evidence([("mixed-copy", copy), ("mixed-original", original), ("mixed-cache", cache)])
    }

    func testContentsAndLateDrawing() throws {
        _ = NSApplication.shared
        let shell = OrientationView(frame: NSRect(x: 0, y: 0, width: 480, height: 112))
        shell.wantsLayer = true
        let root = OrientationView(frame: shell.bounds); shell.addSubview(root)
        let window = NSWindow(contentRect: shell.bounds, styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = shell
        defer { window.close() }
        func ink(_ i: Int) -> OrientationInk {
            let v = OrientationInk(frame: NSRect(x: (i % 4) * 120, y: (i / 4) * 56, width: 120, height: 56))
            root.addSubview(v); return v
        }
        let early = ink(0), redraw = ink(1)
        window.orderFront(nil); shell.displayIfNeeded(); CATransaction.flush()
        let late = ink(2)
        late.displayIfNeeded()
        let held = ink(3)
        root.alphaValue = 0
        root.displayIfNeeded()
        root.alphaValue = 1
        redraw.needsDisplay = true
        // A CGImage and an IOSurface carry the same asymmetric picture.
        let context = try XCTUnwrap(CGContext(data: nil, width: 120, height: 56, bitsPerComponent: 8, bytesPerRow: 480,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue))
        context.setFillColor(NSColor.white.cgColor); context.fill(CGRect(x: 0, y: 0, width: 120, height: 56))
        context.setFillColor(NSColor.red.cgColor); context.fill(CGRect(x: 0, y: 48, width: 20, height: 8))
        context.setFillColor(NSColor.blue.cgColor); context.fill(CGRect(x: 0, y: 0, width: 20, height: 8))
        let image = try XCTUnwrap(context.makeImage())
        let surface = try XCTUnwrap(IOSurface(properties: [.width: 120, .height: 56, .bytesPerElement: 4, .bytesPerRow: 480, .allocSize: 480 * 56, .pixelFormat: UInt32(0x42475241)]))
        surface.lock(options: [], seed: nil)
        memcpy(surface.baseAddress, context.data!, 480 * 56)
        surface.unlock(options: [], seed: nil)
        var views: [NSView] = [early, redraw, late, held]
        for i in 4..<8 {
            let v = OrientationUpdate(frame: NSRect(x: (i % 4) * 120, y: 56, width: 120, height: 56))
            v.image = i % 2 == 0 ? image : surface
            v.flipsCoordinates = i < 6
            root.addSubview(v); views.append(v)
        }
        root.displayIfNeeded(); CATransaction.flush()
        let deadline = Date().addingTimeInterval(1)
        while views.contains(where: { $0.layer?.presentation()?.contents == nil }) && Date() < deadline {
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
        }
        for (i, v) in views.enumerated() {
            let presented = try XCTUnwrap(v.layer?.presentation(), "presentation layer unavailable for contents case \(i)")
            let contents = try XCTUnwrap(presented.contents, "presentation contents unavailable for case \(i)")
            print("PRESENTATION \(i) available=\(type(of: contents))")
        }
        let backing = try XCTUnwrap(root.layer)
        let tree = Capture.paintOrderLayer(of: root, root: backing)
        let copy = NSBitmapImageRep(cgImage: try XCTUnwrap(Capture.picture(of: tree, size: root.bounds.size, scale: 1)))
        let drawsAfterCopy = [early, redraw, late, held].map(\.draws)
        let shownTree = Capture.paintOrderLayer(of: root, root: backing)
        func swap(_ source: CALayer, _ target: CALayer) throws {
            let presented = try XCTUnwrap(source.presentation(), "presentation layer unavailable while copying \(type(of: source))")
            if source.contents != nil {
                target.contents = try XCTUnwrap(presented.contents, "drawing layer has no presentation contents")
            } else { target.contents = presented.contents }
            for (s, t) in zip(source.sublayers ?? [], target.sublayers ?? []) { try swap(s, t) }
        }
        try swap(backing, shownTree)
        let shownCopy = NSBitmapImageRep(cgImage: try XCTUnwrap(Capture.picture(of: shownTree, size: root.bounds.size, scale: 1)))
        XCTAssertEqual([early, redraw, late, held].map(\.draws), drawsAfterCopy, "sample both copies before any original-tree redraw")
        let original = NSBitmapImageRep(cgImage: try XCTUnwrap(Capture.picture(of: backing, size: root.bounds.size, scale: 1)))
        for (i, v) in views.enumerated() {
            let layer = try XCTUnwrap(v.layer), contents = layer.contents as? NSObject
            let pixel = contents?.responds(to: NSSelectorFromString("pixelTransform")) == true ? String(describing: contents?.value(forKey: "pixelTransform")) : "none"
            print("TIMING \(i) copy=\(marker(copy, i)) original=\(marker(original, i)) shownCopy=\(marker(shownCopy, i)) viewFlip=\(v.isFlipped) geometry=\(layer.isGeometryFlipped) contentsFlip=\(layer.contentsAreFlipped()) update=\(v.wantsUpdateLayer) contents=\(String(describing: layer.contents.map { type(of: $0) })) pixel=\(pixel) ctm=\(String(describing: (v as? OrientationInk)?.drawnCTM))")
        }
        let cache = try XCTUnwrap(root.bitmapImageRepForCachingDisplay(in: root.bounds))
        root.cacheDisplay(in: root.bounds, to: cache)
        for i in 0..<8 {
            print("TIMING CACHE \(i) \(marker(cache, i))")
            for rep in [copy, shownCopy, original, cache] { assertMarkerPairExists(rep, i) }
            XCTAssertEqual(marker(copy, i), "upright", "model contents, case \(i)")
            XCTAssertEqual(marker(shownCopy, i), "upright", "presentation contents, case \(i)")
            XCTAssertEqual(marker(original, i), "upright", "original tree, case \(i)")
            XCTAssertEqual(marker(cache, i), "upright", "AppKit reference, case \(i)")
        }
        try evidence([("timing-copy", copy), ("timing-original", original), ("timing-cache", cache), ("timing-shown-copy", shownCopy)])
    }

    func testSubtreeBackingHistories() throws {
        _ = NSApplication.shared
        let shell = OrientationView(frame: NSRect(x: 0, y: 0, width: 480, height: 112))
        shell.wantsLayer = true
        let root = OrientationView(frame: shell.bounds)
        shell.addSubview(root)
        let window = NSWindow(contentRect: shell.bounds, styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = shell
        defer { window.close() }
        var views: [OrientationInk] = []
        for i in 0..<8 {
            let ink = OrientationInk(frame: NSRect(x: (i % 4) * 120, y: (i / 4) * 56, width: 120, height: 56))
            root.addSubview(ink); views.append(ink)
        }
        window.orderFront(nil); shell.displayIfNeeded(); CATransaction.flush()
        views[1].display()
        views[2].layer?.display()
        views[3].needsDisplay = true; views[3].displayIfNeeded()
        views[4].layer?.setNeedsDisplay(); views[4].layer?.displayIfNeeded()
        let previousBacking = try XCTUnwrap(views[5].layer)
        let previousContents = try XCTUnwrap(previousBacking.contents) as AnyObject
        // AppKit keeps inherited backing while the view has a backed ancestor.
        views[5].removeFromSuperview()
        views[5].wantsLayer = false; views[5].layer = nil
        XCTAssertNil(views[5].layer)
        views[5].wantsLayer = true
        root.addSubview(views[5]); views[5].display()
        let replacementBacking = try XCTUnwrap(views[5].layer)
        XCTAssertFalse(previousBacking === replacementBacking, "history case 5 must install a new backing layer")
        XCTAssertFalse(previousContents === (try XCTUnwrap(replacementBacking.contents) as AnyObject), "the new layer has a new drawing store")
        views[6].layer?.setNeedsDisplay(); root.displayIfNeeded()
        views[7].layer?.isGeometryFlipped = true; views[7].display()
        views[7].layer?.isGeometryFlipped = false
        CATransaction.flush()
        let drawn = views.map { ($0.draws, $0.drawnCTM) }
        let layer = try XCTUnwrap(root.layer)
        let copy = try XCTUnwrap(Capture.paintOrderBitmap(of: root, scale: 1))
        let original = NSBitmapImageRep(cgImage: try XCTUnwrap(Capture.picture(of: layer, size: root.bounds.size, scale: 1)))
        CATransaction.flush()
        let presented = try layer.presentation().map { NSBitmapImageRep(cgImage: try XCTUnwrap(Capture.picture(of: $0, size: root.bounds.size, scale: 1))) }
        let cache = try XCTUnwrap(root.bitmapImageRepForCachingDisplay(in: root.bounds))
        root.cacheDisplay(in: root.bounds, to: cache)
        for (i, ink) in views.enumerated() {
            let backing = try XCTUnwrap(ink.layer)
            print("HISTORY \(i) copy=\(marker(copy, i)) original=\(marker(original, i)) presentation=\(presented.map { marker($0, i) } ?? "nil") cache=\(marker(cache, i)) ctmBeforeCache=\(drawn[i]) geometry=\(backing.isGeometryFlipped)/\(layer.isGeometryFlipped)/\(shell.layer!.isGeometryFlipped) flipped=\(backing.contentsAreFlipped()) contents=\(String(describing: backing.contents)) presentedContents=\(String(describing: backing.presentation()?.contents))")
            XCTAssertEqual(marker(cache, i), "upright")
            XCTAssertEqual(marker(original, i), "upright")
            // Every copied branch preserves its backing store's orientation, including replacement store 5.
            XCTAssertEqual(marker(copy, i), "upright", "copy, case \(i)")
        }
        try evidence([("history-copy", copy), ("history-original", original), ("history-cache", cache)])
    }

    private func assertMarkerPairExists(_ rep: NSBitmapImageRep, _ i: Int, file: StaticString = #filePath, line: UInt = #line) {
        let top = marker(rep, i), bottom = marker(rep, i, bottom: true)
        XCTAssertTrue(["upright", "flipped"].contains(top), "missing marker in cell \(i): \(top)", file: file, line: line)
        XCTAssertEqual(top, bottom, "cell \(i) must contain both colored markers", file: file, line: line)
    }

    private func glyphs(_ rep: NSBitmapImageRep, in rect: CGRect) -> (ink: Int, mask: [Bool]) {
        var points: [(Int, Int)] = []
        for y in 0..<Int(rect.height) {
            for x in 0..<Int(rect.width) {
                let px = Int((rect.minX + CGFloat(x)) * CGFloat(rep.pixelsWide) / rep.size.width)
                let py = Int((rect.minY + CGFloat(y)) * CGFloat(rep.pixelsHigh) / rep.size.height)
                if let c = rep.colorAt(x: px, y: py), c.alphaComponent > 0.2,
                   max(c.redComponent, c.greenComponent, c.blueComponent) < 0.4 { points.append((x, y)) }
            }
        }
        guard let minX = points.map(\.0).min(), let minY = points.map(\.1).min(),
              let maxX = points.map(\.0).max(), let maxY = points.map(\.1).max() else {
            return (0, Array(repeating: false, count: 32 * 24))
        }
        let width = maxX - minX + 1, height = maxY - minY + 1
        let ink = Set(points.map { ($0.1 - minY) * width + $0.0 - minX })
        // Normalize the ink bounds to compare 1x and Retina captures without
        // including the editor's padding or depending on glyph placement.
        let mask = (0..<24).flatMap { y in (0..<32).map { x in ink.contains((y * height / 24) * width + x * width / 32) } }
        return (points.count, mask)
    }

    private func glyphDifference(_ a: [Bool], _ b: [Bool], flipped: Bool = false) -> Double {
        let mismatches = a.indices.filter { i in a[i] != b[(flipped ? 23 - i / 32 : i / 32) * 32 + i % 32] }.count
        return Double(mismatches) / Double(a.count)
    }

    private func marker(_ rep: NSBitmapImageRep, _ i: Int, bottom: Bool = false) -> String {
        let x = (i % 4) * 120 + 4, y = (i / 4) * 56 + (bottom ? 52 : 4)
        guard let c = rep.colorAt(x: x * rep.pixelsWide / 480, y: y * rep.pixelsHigh / 112), c.alphaComponent > 0.9 else { return "missing" }
        let red = c.redComponent > 0.8 && c.blueComponent < 0.2
        let blue = c.blueComponent > 0.8 && c.redComponent < 0.2
        return (bottom ? blue : red) ? "upright" : (bottom ? red : blue) ? "flipped" : "other"
    }

    private func evidence(_ pictures: [(String, NSBitmapImageRep)]) throws {
        if let dir = ProcessInfo.processInfo.environment["EXACT_CAPTURE_EVIDENCE"] {
            for (name, rep) in pictures {
                try rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: dir).appendingPathComponent("\(name).png"))
            }
        }
    }

    func testBackingStoreOrientationMatrix() throws {
        _ = NSApplication.shared
        let root = OrientationView(frame: NSRect(x: 0, y: 0, width: 480, height: 112))
        root.wantsLayer = true
        let window = NSWindow(contentRect: root.bounds, styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = root
        defer { window.close() }
        var cases: [(String, OrientationInk)] = []
        for parentFlipped in [true, false] {
            for flipped in [true, false] {
                for explicit in [false, true] {
                    let i = cases.count
                    let parent = OrientationView(frame: NSRect(x: (i % 4) * 120, y: (i / 4) * 56, width: 120, height: 56))
                    parent.flipsCoordinates = parentFlipped
                    let ink = OrientationInk(frame: parent.bounds)
                    ink.flipsCoordinates = flipped; ink.wantsLayer = explicit
                    parent.addSubview(ink); root.addSubview(parent)
                    cases.append(("parent=\(parentFlipped),view=\(flipped),explicit=\(explicit)", ink))
                }
            }
        }
        window.orderFront(nil); root.displayIfNeeded(); CATransaction.flush()
        let backing = try XCTUnwrap(root.layer)
        let copy = try XCTUnwrap(Capture.paintOrderBitmap(of: root, scale: 1))
        let original = NSBitmapImageRep(cgImage: try XCTUnwrap(Capture.picture(of: backing, size: root.bounds.size, scale: 1)))
        let cache = try XCTUnwrap(root.bitmapImageRepForCachingDisplay(in: root.bounds))
        root.cacheDisplay(in: root.bounds, to: cache)
        for (i, item) in cases.enumerated() {
            let (name, ink) = item, layer = try XCTUnwrap(ink.layer)
            var chain: [Bool] = [], ancestor: CALayer? = layer
            while let a = ancestor { chain.append(a.isGeometryFlipped); ancestor = a.superlayer }
            print("ORIENTATION \(name): cache=\(marker(cache, i)) copy=\(marker(copy, i)) original=\(marker(original, i)) layer=\(type(of: layer)) contents=\(String(describing: layer.contents.map { type(of: $0) })) geometry=\(chain) contentsFlipped=\(layer.contentsAreFlipped()) ctm=\(ink.drawnCTM) draws=\(ink.draws)")
            XCTAssertEqual(marker(cache, i), "upright")
            XCTAssertEqual(marker(copy, i), "upright")
            XCTAssertEqual(marker(original, i), "upright")
        }
        try evidence([("copy", copy), ("original", original), ("cache", cache)])
    }
}
#endif
