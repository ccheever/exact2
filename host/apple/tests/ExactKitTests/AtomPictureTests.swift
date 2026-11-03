// LLP 1045.000 D2, §4: an atom's picture from an image source while it is
// read, when it cannot be, and the room its pill takes in the text area's
// measured text (`field-sizing: content` sizes the field by it).
import XCTest
import CoreText
@testable import ExactKit
@testable import ExactMarkdown
import CExact

private let kinds = #"[{"name":"citation","link":"t3-citation:"}]"#
private let cite = "[Quote](t3-citation:1)"

/// A face whose label is drawn black on white beside `icon`.
private func face(_ icon: String) -> String {
    ##"[{"source":"[Quote](t3-citation:1)","label":"Ab","icon":"\##(icon)","title":"","color":"#000000","backgroundColor":"#ffffff"}]"##
}

/// A red square, as an SVG `data:` URL.
private let redSquare = "data:image/svg+xml;base64," + Data(##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><rect width="4" height="4" fill="#ff0000"/></svg>"##.utf8).base64EncodedString()

/// The RGBA rows `draw` paints into a `width` by 20 picture.
private func pixels(width: Int, _ draw: (CGContext, CGRect) -> Void) throws -> (data: [UInt8], row: Int) {
    let srgb = try XCTUnwrap(CGColorSpace(name: CGColorSpace.sRGB))
    let context = try XCTUnwrap(CGContext(data: nil, width: width, height: 20, bitsPerComponent: 8, bytesPerRow: 0, space: srgb, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    draw(context, CGRect(x: 0, y: 0, width: width, height: 20))
    let data = try XCTUnwrap(context.data).assumingMemoryBound(to: UInt8.self)
    return (Array(UnsafeBufferPointer(start: data, count: context.bytesPerRow * 20)), context.bytesPerRow)
}

/// The leftmost column holding the label's black ink, and whether any of
/// the picture's red is drawn.
private func inked(_ picture: (data: [UInt8], row: Int), width: Int) -> (label: Int?, red: Bool) {
    var label: Int?, red = false
    for y in 0..<20 {
        for x in 0..<width {
            let p = picture.data[(y * picture.row + x * 4)...].prefix(4).map(Int.init)
            guard p[3] > 200 else { continue }
            if p[0] < 90, p[1] < 90, p[2] < 90 { label = min(label ?? x, x) }
            if p[0] > 200, p[1] < 60, p[2] < 60 { red = true }
        }
    }
    return (label, red)
}

/// The paragraph a text area's Markdown `text` measures as at `width`
/// (`markup` 1 a reader's, 2 the field's own), in a 16 px system font.
private func measured(_ engine: TextEngine, _ text: String, markup: UInt8, width: Float, faces: String) -> ExactMetrics {
    var source = text, kinds = kinds, faces = faces
    return source.withUTF8 { s in kinds.withUTF8 { k in faces.withUTF8 { f in
        var run = ExactTextRun()
        run.text = s.baseAddress; run.len = s.count; run.font_size = 16; run.font_weight = 400
        var strut = run; strut.text = nil; strut.len = 0
        return withUnsafePointer(to: &run) { run in
            var request = ExactMeasureRequest()
            request.runs = run; request.count = 1; request.strut = strut
            request.width = width; request.height = -1; request.white_space = 1; request.markup = markup
            request.atoms = k.baseAddress; request.atoms_len = k.count; request.faces = f.baseAddress; request.faces_len = f.count
            return engine.measure(request)
        }
    } } }
}

final class AtomPictureTests: XCTestCase {
    override func setUp() { super.setUp(); ExactMarkdown.install(); AtomIcons.reset() }
    override func tearDown() { AtomIcons.reset(); super.tearDown() }

    /// A picture that cannot be read is logged with the step that failed:
    /// a file the app does not have, an SVG the parser refuses.
    func testAPictureThatCannotBeReadIsLoggedWithItsStep() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("atom-pictures-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: root.appendingPathComponent("assets"), withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        try Data("<svg xmlns=\"http://www.w3.org/2000/svg\"><path d=\"M0 0".utf8).write(to: root.appendingPathComponent("assets/broken.svg"))
        try Data(#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 4 4"><rect width="4" height="4"/></svg>"#.utf8).write(to: root.appendingPathComponent("assets/fine.svg"))
        let resolver = AssetResolver(root: root)
        var lines: [String] = [], ready = 0
        for name in ["assets/missing.svg", "assets/broken.svg", "assets/fine.svg"] {
            XCTAssertNil(AtomIcons.image(name, pixels: 30, resolver: resolver, log: { lines.append($0) }) { ready += 1 }, "\(name): read off main")
        }
        waitFor("each field is told", facts: { "\(ready) told" }) { ready == 3 }
        XCTAssertNotNil(AtomIcons.image("assets/fine.svg", pixels: 30, resolver: resolver) {}, "an SVG asset is drawn")
        XCTAssertTrue(AtomIcons.failed("assets/missing.svg", pixels: 30) && AtomIcons.failed("assets/broken.svg", pixels: 30))
        XCTAssertFalse(AtomIcons.failed("assets/fine.svg", pixels: 30))
        XCTAssertEqual(lines.count, 2, "\(lines)")
        XCTAssertTrue(lines.contains { $0.hasPrefix("atom icon refused: assets/missing.svg: read: ") }, "\(lines)")
        XCTAssertTrue(lines.contains { $0.hasPrefix("atom icon refused: assets/broken.svg: ") }, "\(lines)")
    }

    /// A text area's Markdown is measured as its editor lays it out: an
    /// atom is its pill (its padding and its icon's room), wider than the
    /// reader's label, so a line the reader keeps whole wraps; a link's
    /// destination takes no room.
    func testATextAreasMarkdownMeasuresItsPills() throws {
        let session = ExactApp.shared.makeSession(label: "atom-measure")
        defer { session.destroy() }
        let engine = session.text
        let text = "Fix the flaky test in " + cite + " now", icon = face("symbol:sf/circle.fill")
        let reader = measured(engine, text, markup: 1, width: -1, faces: icon)
        let editor = measured(engine, text, markup: 2, width: -1, faces: icon)
        XCTAssertGreaterThan(editor.width, reader.width + 12, "the icon's room and the pill's padding")
        XCTAssertEqual(editor.height, reader.height, "a pill keeps its line's height")
        let width = reader.width + 2
        XCTAssertEqual(measured(engine, text, markup: 1, width: width, faces: icon).height, reader.height, "the reader's one line")
        XCTAssertEqual(measured(engine, text, markup: 2, width: width, faces: icon).height, 2 * reader.height, accuracy: 1, "the field's two")
        let link = measured(engine, "Fix [the docs](https://example.com/a/long/path) now", markup: 2, width: -1, faces: "[]")
        let shown = measured(engine, "Fix [the docs now", markup: 2, width: -1, faces: "[]")
        XCTAssertEqual(link.width, shown.width, accuracy: 0.5, "the destination is hidden, its `[` shown")
    }
}

#if os(macOS)
import AppKit

final class AtomPictureMacTests: XCTestCase {
    private var session: ExactSession?
    override func setUp() { super.setUp(); ExactMarkdown.install(); AtomIcons.reset() }
    /// The pictures' queue is held: a test that fails holding it lets it go.
    private var holding = false
    override func tearDown() {
        if holding { AtomIcons.queue.resume(); holding = false }
        AtomIcons.reset(); session?.destroy(); session = nil; super.tearDown()
    }

    private func editor(faces: String, atoms: String = kinds) -> (TextArea, NSWindow) {
        _ = NSApplication.shared
        let presenter = Presenter()
        session = ExactApp.shared.makeSession(label: "atom-pictures")
        presenter.session = session
        let node = NodeView(id: 3, kind: "textarea", presenter: presenter)
        node.frame = NSRect(x: 0, y: 0, width: 500, height: 300)
        node.makeTextArea()
        node.props["markup"] = "markdown"
        node.props["value"] = "Ask " + cite + " now"
        node.props["atoms"] = atoms
        node.props["atomFaces"] = faces
        node.applyTextArea()
        node.layoutTextArea()
        let window = NSWindow(contentRect: node.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = node
        node.restyleMarkup()
        return (node.textArea as! TextArea, window)
    }

    private func pill(_ f: TextArea) throws -> AtomAttachment {
        let manager = try XCTUnwrap(f.textLayoutManager)
        manager.ensureLayout(for: manager.documentRange)
        var found: AtomAttachment?
        manager.enumerateTextLayoutFragments(from: manager.documentRange.location, options: [.ensuresLayout]) { fragment in
            guard let paragraph = fragment.textElement as? NSTextParagraph else { return true }
            let text = paragraph.attributedString
            text.enumerateAttribute(.attachment, in: NSRange(location: 0, length: text.length)) { value, _, _ in
                if found == nil { found = value as? AtomAttachment }
            }
            return found == nil
        }
        return try XCTUnwrap(found, "a pill")
    }

    private func drawn(_ f: TextArea) throws -> (label: Int?, red: Bool) {
        let pill = try pill(f), location = try XCTUnwrap(f.textLayoutManager?.documentRange.location)
        let image = try XCTUnwrap(pill.image(for: CGRect(x: 0, y: 0, width: 60, height: 20), attributes: [.font: NSFont.systemFont(ofSize: 16)], location: location, textContainer: nil))
        return inked(try pixels(width: 60) { context, rect in
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = NSGraphicsContext(cgContext: context, flipped: false)
            image.draw(in: rect)
            NSGraphicsContext.restoreGraphicsState()
        }, width: 60)
    }

    private func width(_ f: TextArea) throws -> CGFloat {
        let location = try XCTUnwrap(f.textLayoutManager?.documentRange.location)
        return try pill(f).attachmentBounds(for: [.font: NSFont.systemFont(ofSize: 16)], location: location, textContainer: nil,
                                            proposedLineFragment: CGRect(x: 0, y: 0, width: 1000, height: 20), position: .zero).width
    }

    /// While its picture is read the pill keeps the icon's room and draws
    /// its label where the label stays once the picture is there.
    func testWhileItsPictureIsReadTheLabelIsWhereItStays() throws {
        AtomIcons.queue.suspend(); holding = true
        let (f, window) = editor(faces: face(redSquare))
        defer { window.close() }
        let waiting = try drawn(f)
        XCTAssertFalse(waiting.red, "not read yet")
        AtomIcons.queue.resume(); holding = false
        waitFor("the picture is drawn") { (try? drawn(f))?.red == true }
        let arrived = try drawn(f)
        let at = try XCTUnwrap(arrived.label), before = try XCTUnwrap(waiting.label)
        XCTAssertEqual(before, at, accuracy: 1, "the label does not move when the picture comes")
    }

    /// D4: a kind's `-exact-icon-size` and `gap` are its icon's side and the
    /// space before its label; the icon is centred on half the cap height
    /// above the baseline.
    func testAKindsIconSizeAndGapAreItsIcons() throws {
        let atoms = #"[{"name":"citation","link":"t3-citation:","iconSize":"14px","gap":"0px"}]"#
        let (f, window) = editor(faces: face(redSquare), atoms: atoms)
        defer { window.close() }
        let deadline = Date().addingTimeInterval(10)
        while try !drawn(f).red, Date() < deadline { RunLoop.current.run(until: Date().addingTimeInterval(0.05)) }
        let pill = try pill(f), location = try XCTUnwrap(f.textLayoutManager?.documentRange.location)
        let font = NSFont.systemFont(ofSize: 16)
        let image = try XCTUnwrap(pill.image(for: CGRect(x: 0, y: 0, width: 60, height: 20), attributes: [.font: font], location: location, textContainer: nil))
        let picture = try pixels(width: 60) { context, rect in
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = NSGraphicsContext(cgContext: context, flipped: false)
            image.draw(in: rect)
            NSGraphicsContext.restoreGraphicsState()
        }
        var red: (x: [Int], y: [Int]) = ([], [])
        for y in 0..<20 {
            for x in 0..<60 {
                let p = picture.data[(y * picture.row + x * 4)...].prefix(4).map(Int.init)
                if p[3] > 200, p[0] > 200, p[1] < 60, p[2] < 60 { red.x.append(x); red.y.append(y) }
            }
        }
        let left = try XCTUnwrap(red.x.min()), right = try XCTUnwrap(red.x.max()), top = try XCTUnwrap(red.y.min()), bottom = try XCTUnwrap(red.y.max())
        XCTAssertEqual(Double(right - left + 1), 14, accuracy: 1, "14 px wide")
        XCTAssertEqual(Double(bottom - top + 1), 14, accuracy: 1, "14 px tall")
        let baseline = 20 - (font.ascender - font.descender) + font.ascender
        XCTAssertEqual(Double(top + bottom + 1) / 2, Double(baseline - font.capHeight / 2), accuracy: 1, "centred on half the cap height")
        let label = try XCTUnwrap(inked(picture, width: 60).label)
        XCTAssertEqual(Double(label - right - 1), 0, accuracy: 1.5, "no gap before the label")
    }

    /// A picture that cannot be read leaves no room in its pill.
    func testAPictureThatCannotBeReadLeavesNoRoom() throws {
        let (plain, window) = editor(faces: face(""))
        let without = try width(plain)
        window.close()
        let (f, other) = editor(faces: face("data:image/svg+xml,%3Csvg"))
        defer { other.close() }
        // Drawing the pill asks for its picture.
        _ = try drawn(f)
        waitFor("the pill leaves no room for the picture it cannot read") { _ = try? drawn(f); return (try? width(f)).map { $0 <= without + 0.5 } ?? false }
        XCTAssertEqual(try width(f), without, accuracy: 0.5)
    }
}
#endif
