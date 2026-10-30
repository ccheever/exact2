#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactMarkdown

/// LLP 1001 §1 on UIKit: CSS `direction` is a plain field's paragraph base
/// direction, as it is Apple text's and the Markdown editor's (LLP 1045
/// D11): an `input` (plain, password, search), a `textarea` of either
/// appearance, and a Markdown editor's text once its `markup` goes. No
/// first strong character picks it. An unsaid `text-align` is `start`,
/// which UIKit's fields already took (the guard here).
final class FieldDirectionIOSTests: XCTestCase {
    private var window: UIWindow?
    private var sessions: [ExactSession] = []
    override func setUp() { super.setUp(); ExactMarkdown.install() }
    override func tearDown() { window?.isHidden = true; window = nil; sessions.forEach { $0.destroy() }; sessions = []; super.tearDown() }

    /// A presenter whose node 2 is a `kind` with `props` and `style`.
    private func field(_ kind: String, _ value: String, props: [String: Any] = [:], style: [String: Any]) -> (Presenter, NodeView) {
        let p = Presenter()
        // A session for the text engine a field styles with.
        let session = ExactApp.shared.makeSession(label: "field-direction")
        sessions.append(session)
        p.session = session
        let w = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        p.viewport.frame = w.bounds
        w.addSubview(p.viewport)
        w.makeKeyAndVisible()
        window = w
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": kind, "props": props.merging(["value": value]) { a, _ in a }, "handlers": ["input", "change"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": kind == "input" ? 34.0 : 120.0],
            ["op": "style", "id": 2, "style": style],
        ]))
        p.viewport.layoutIfNeeded()
        return (p, p.views[2]!)
    }
    private func paragraph(_ attributes: [NSAttributedString.Key: Any]) -> NSParagraphStyle? { attributes[.paragraphStyle] as? NSParagraphStyle }
    private func base(_ tv: UITextView, at index: Int = 0) -> NSWritingDirection? {
        (tv.textStorage.attribute(.paragraphStyle, at: index, effectiveRange: nil) as? NSParagraphStyle)?.baseWritingDirection
    }
    /// Where character `index` is drawn, from the view's left.
    private func x(_ input: UITextInput & UIView, _ index: Int) throws -> CGFloat {
        let start = try XCTUnwrap(input.position(from: input.beginningOfDocument, offset: index))
        let end = try XCTUnwrap(input.position(from: start, offset: 1))
        return input.firstRect(for: try XCTUnwrap(input.textRange(from: start, to: end))).minX
    }

    /// Tests 1 and 2: each field kind takes the node's direction as its base.
    func testEachFieldTakesTheNodesDirectionAsItsBase() throws {
        for props: [String: Any] in [[:], ["type": "search"], ["type": "password"]] {
            for (direction, value, want) in [("rtl", "Hello مرحبا", NSWritingDirection.rightToLeft), ("ltr", "مرحبا Hello", .leftToRight)] {
                let (_, node) = field("input", value, props: props, style: ["direction": direction])
                let f = try XCTUnwrap(node.field)
                XCTAssertEqual(paragraph(f.defaultTextAttributes)?.baseWritingDirection, want, "\(props) \(direction)")
                // The base UIKit lays the text out with, and where it draws, while it edits.
                XCTAssertEqual(f.baseWritingDirection(for: f.beginningOfDocument, in: .forward), want, "\(props) \(direction): its text")
                if props["type"] == nil, f.becomeFirstResponder() {
                    f.layoutIfNeeded()
                    if want == .rightToLeft { XCTAssertGreaterThan(try x(f, 0), try x(f, 6), "\(direction): H right of م") }
                    else { XCTAssertLessThan(try x(f, 0), try x(f, 6), "\(direction): م left of H") }
                    f.resignFirstResponder()
                }
                window?.isHidden = true
            }
        }
        for look: [String: Any] in [[:], ["appearance": "none"]] {
            var (_, node) = field("textarea", "Hello مرحبا\nمرحبا Hello", style: look.merging(["direction": "rtl"]) { a, _ in a })
            var tv = try XCTUnwrap(node.textArea)
            XCTAssertEqual(base(tv, at: 0), .rightToLeft, "\(look)")
            XCTAssertEqual(base(tv, at: 12), .rightToLeft, "\(look): the second paragraph, opening in Arabic")
            XCTAssertEqual(paragraph(tv.typingAttributes)?.baseWritingDirection, .rightToLeft, "\(look): what is typed next")
            XCTAssertGreaterThan(try x(tv, 0), try x(tv, 6), "\(look): H right of م")
            window?.isHidden = true
            (_, node) = field("textarea", "مرحبا Hello", style: look.merging(["direction": "ltr"]) { a, _ in a })
            tv = try XCTUnwrap(node.textArea)
            XCTAssertEqual(base(tv), .leftToRight, "\(look)")
            XCTAssertLessThan(try x(tv, 0), try x(tv, 6), "\(look): م left of H")
            window?.isHidden = true
        }
    }

    /// Test 3 (a guard): an unsaid `text-align` is `start`, the right under
    /// `rtl`; one said stands.
    func testAnUnsaidAlignmentIsStart() throws {
        var (_, node) = field("input", "Hi", style: ["direction": "rtl"])
        XCTAssertEqual(node.field?.textAlignment, .right)
        window?.isHidden = true
        (_, node) = field("input", "Hi", style: ["direction": "rtl", "text_align": "left"])
        XCTAssertEqual(node.field?.textAlignment, .left)
        window?.isHidden = true
        (_, node) = field("textarea", "Hi", style: ["direction": "rtl"])
        XCTAssertEqual(node.textArea?.textAlignment, .right)
        window?.isHidden = true
    }

    /// Test 4: a Markdown editor whose `markup` goes keeps the field's base.
    func testAMarkdownEditorWhoseMarkupGoesKeepsTheBase() throws {
        let (p, node) = field("textarea", "مرحبا Hello", props: ["markup": "markdown"], style: ["direction": "ltr"])
        node.restyleMarkup()
        let tv = try XCTUnwrap(node.textArea)
        XCTAssertEqual(base(tv), .leftToRight, "the Markdown editor's")
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["markup": "none"]]]))
        XCTAssertNil((tv as? TextArea)?.markup)
        XCTAssertEqual(base(tv), .leftToRight, "the plain field's, once markup goes")
        XCTAssertEqual(paragraph(tv.typingAttributes)?.baseWritingDirection, .leftToRight)
        XCTAssertLessThan(try x(tv, 0), try x(tv, 6), "م left of H")
    }

    /// Test 5: a change of `direction` restyles the text there and what is
    /// typed next, and leaves the selection.
    func testADirectionChangeRestylesAndKeepsTheSelection() throws {
        let (p, node) = field("textarea", "Hello مرحبا", style: ["direction": "rtl"])
        let tv = try XCTUnwrap(node.textArea)
        tv.selectedRange = NSRange(location: 3, length: 2)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "ltr"]]]))
        XCTAssertEqual(base(tv), .leftToRight)
        XCTAssertEqual(paragraph(tv.typingAttributes)?.baseWritingDirection, .leftToRight)
        XCTAssertEqual(tv.selectedRange, NSRange(location: 3, length: 2))
        window?.isHidden = true
        let (q, input) = field("input", "Hello مرحبا", style: ["direction": "rtl"])
        q.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "ltr"]]]))
        XCTAssertEqual(paragraph(try XCTUnwrap(input.field).defaultTextAttributes)?.baseWritingDirection, .leftToRight, "an input")
    }

    /// A `direction` that comes while an input method composes applies once
    /// the composition ends; the marked text is left alone meanwhile.
    func testADirectionThatComesWhileComposingAppliesAtItsEnd() throws {
        let (p, node) = field("textarea", "abc", style: ["direction": "rtl"])
        let tv = try XCTUnwrap(node.textArea)
        XCTAssertTrue(tv.becomeFirstResponder())
        tv.selectedRange = NSRange(location: 3, length: 0)
        tv.setMarkedText("か", selectedRange: NSRange(location: 1, length: 0))
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "ltr"]]]))
        XCTAssertNotNil(tv.markedTextRange)
        tv.unmarkText()
        XCTAssertEqual(base(tv), .leftToRight, "a textarea, at the composition's end")
        XCTAssertEqual(paragraph(tv.typingAttributes)?.baseWritingDirection, .leftToRight)
        tv.resignFirstResponder()
        window?.isHidden = true
        let (q, input) = field("input", "abc", style: ["direction": "rtl"])
        let f = try XCTUnwrap(input.field)
        XCTAssertTrue(f.becomeFirstResponder())
        f.setMarkedText("か", selectedRange: NSRange(location: 1, length: 0))
        q.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "ltr"]]]))
        XCTAssertEqual(paragraph(f.defaultTextAttributes)?.baseWritingDirection, .rightToLeft, "an input leaves its marked text alone")
        f.unmarkText() // a commit that changes no text: UIKit sends no editing change
        XCTAssertEqual(paragraph(f.defaultTextAttributes)?.baseWritingDirection, .leftToRight, "an input, at the composition's end")
        f.resignFirstResponder()
    }

    /// The field standing in for an input (a search bar's, an alert
    /// prompt's: `headerField`) takes its direction and a change of it.
    func testTheFieldStandingInForAnInputTakesItsDirection() throws {
        let (p, node) = field("input", "Hello", style: ["direction": "ltr"])
        let standIn = UITextField(frame: CGRect(x: 0, y: 40, width: 300, height: 34))
        window?.addSubview(standIn)
        node.headerField = standIn
        node.applyFieldDirection()
        XCTAssertEqual(paragraph(standIn.defaultTextAttributes)?.baseWritingDirection, .leftToRight)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "rtl"]]]))
        XCTAssertEqual(paragraph(standIn.defaultTextAttributes)?.baseWritingDirection, .rightToLeft)
        XCTAssertEqual(standIn.textAlignment, .right, "CSS's start, not UIKit's own side")
        XCTAssertEqual(paragraph(standIn.defaultTextAttributes)?.alignment, .right)
        // Marked, it is left alone; what its delegate does at the composition's end applies it.
        XCTAssertTrue(standIn.becomeFirstResponder())
        standIn.setMarkedText("か", selectedRange: NSRange(location: 1, length: 0))
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["direction": "ltr"]]]))
        XCTAssertEqual(paragraph(standIn.defaultTextAttributes)?.baseWritingDirection, .rightToLeft, "marked: left alone")
        standIn.unmarkText()
        node.applyFieldDirection() // what PromptField's and the search bar's delegates call as the composition ends
        XCTAssertEqual(paragraph(standIn.defaultTextAttributes)?.baseWritingDirection, .leftToRight)
        standIn.resignFirstResponder()
        node.headerField = nil
    }

    /// Test 6: a plain text area's placeholder is drawn at its start, the
    /// right under `rtl`, as the paragraph its text would take.
    func testAPlainTextAreasPlaceholderSitsAtItsStart() throws {
        for (direction, right) in [("rtl", true), ("ltr", false)] {
            let (_, node) = field("textarea", "", props: ["placeholder": "Type here"], style: ["direction": direction])
            let tv = try XCTUnwrap(node.textArea)
            tv.setNeedsDisplay()
            tv.layer.displayIfNeeded()
            let image = UIGraphicsImageRenderer(bounds: tv.bounds).image { context in tv.layer.render(in: context.cgContext) }
            let cg = try XCTUnwrap(image.cgImage)
            let width = cg.width, height = min(cg.height, Int(40 * image.scale))
            var pixels = [UInt8](repeating: 0, count: width * cg.height * 4)
            let ctx = try XCTUnwrap(CGContext(data: &pixels, width: width, height: cg.height, bitsPerComponent: 8, bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            ctx.draw(cg, in: CGRect(x: 0, y: 0, width: width, height: cg.height))
            // Ink: any channel, alpha too, apart from the ground below the first line.
            let px = { (x: Int, y: Int) -> [Int] in let i = (y * width + x) * 4; return (0..<4).map { Int(pixels[i + $0]) } }
            let ground = px(width / 2, cg.height - 4)
            var xs: [Int] = []
            for y in 2..<height {
                let ink = (4..<(width - 4)).filter { x in zip(px(x, y), ground).contains { abs($0 - $1) > 30 } }
                if ink.count < width / 2 { xs += ink }
            }
            XCTAssertFalse(xs.isEmpty, "\(direction): the placeholder drew")
            let middle = xs.isEmpty ? 0 : xs.reduce(0, +) / xs.count
            if right { XCTAssertGreaterThan(middle, width / 2, "rtl: at the right, \(middle) of \(width)") }
            else { XCTAssertLessThan(middle, width / 2, "ltr: at the left, \(middle) of \(width)") }
            window?.isHidden = true
        }
    }
}
#endif
