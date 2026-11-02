#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class InlineContextIOSTests: XCTestCase {
    private var window: UIWindow!
    private var session: ExactSession?

    override func tearDown() { session?.destroy(); session = nil; super.tearDown() }

    private func presenter() throws -> Presenter {
        let session = ExactApp.shared.makeSession(label: "inline-menu")
        self.session = session
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.buttonFace = { _ in var face = ButtonFace(); face.title = "See tag posts"; return face }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text", "style": ["font_size": 18.0]],
            ["op": "create", "id": 3, "kind": "view", "props": ["popover": "auto", "id": "tag-menu"]],
            ["op": "create", "id": 4, "kind": "button", "handlers": ["press"]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 800],
            ["op": "frame", "id": 2, "x": 20, "y": 100, "w": 300, "h": 60],
        ]))
        var style = InlineStyle()
        try style.set("font_size", .number(18))
        p.applyParagraph(2, [
            InlineText(id: 10, parent: 2, props: ["text": "#tag", "contextPopover": "tag-menu"], style: style, handlers: ["press", "contextmenu"], paints: true),
            InlineText(id: 11, parent: 2, props: ["text": " plain text"], style: style, handlers: [], paints: true),
        ])
        p.textViews[2] = p.views[2]
        p.menus.context.sync()
        return p
    }

    func testAnInlineMenuUsesTheRunAndCancelsItsTap() throws {
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        XCTAssertNotNil(node.contextRecognizer, "a paragraph installs recognition for its inline runs")
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let rect = try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(10))).first)
        node.inlinePressed = 10
        var contexts: [UInt32] = [], presses: [UInt32] = []
        p.onClipboard = { id, kind, _ in if kind == 10 { contexts.append(id) } }
        p.onPress = { presses.append($0) }
        let config = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: rect.midX, y: rect.midY)))
        XCTAssertEqual((config.identifier as? NSNumber)?.uint32Value, 10)
        let owner = try XCTUnwrap(p.menus.context.open)
        XCTAssertEqual(try XCTUnwrap(p.menus.context.menu(owner)).children.compactMap { ($0 as? UIAction)?.title }, ["See tag posts"])
        XCTAssertEqual(contexts, [10])
        XCTAssertNil(node.inlinePressed)
        XCTAssertEqual(presses, [])
        let reply = Agent(session: try XCTUnwrap(session)).tap(["id": NSNumber(value: 10), "contextmenu": true])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(contexts, [10, 10], "the recognized agent path dispatches the run too")
        XCTAssertEqual(presses, [], "an inline context request never takes its tap shortcut")
        p.forgetParagraph(node)
        p.menus.context.sync()
        XCTAssertNil(p.menus.context.open, "removing the run retires its open menu")
        XCTAssertNil(p.menus.context.menu(owner), "a retained configuration cannot read a removed run")
        XCTAssertFalse(node.interactions.contains { $0 is UIContextMenuInteraction })
    }

    private final class LongPress: UILongPressGestureRecognizer {
        var point = CGPoint.zero
        override var state: UIGestureRecognizer.State { get { .began } set {} }
        override func location(in view: UIView?) -> CGPoint { point }
    }

    func testAHandlerOnlyRunKeepsRecognitionBesideANativeRunMenu() throws {
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        var style = InlineStyle()
        try style.set("font_size", .number(18))
        let original = node.inlineText[0]
        p.applyParagraph(2, [original, InlineText(id: 11, parent: 2, props: ["text": " Other"], style: style, handlers: ["contextmenu"], paints: true)])
        p.menus.context.sync()
        XCTAssertEqual(node.contextRecognizer?.isEnabled, true)
        let run = try XCTUnwrap(p.inlineText(11)), rect = try XCTUnwrap(node.inlineRects(run).first)
        var contexts: [UInt32] = []
        p.onClipboard = { id, kind, _ in if kind == 10 { contexts.append(id) } }
        let gesture = LongPress()
        gesture.point = CGPoint(x: rect.midX, y: rect.midY)
        node.openContext(gesture)
        XCTAssertEqual(contexts, [11])
        let reply = Agent(session: try XCTUnwrap(session)).tap(["id": NSNumber(value: 11), "contextmenu": true])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(contexts, [11, 11])
    }

    func testAParagraphHandlerRecognizesOutsideItsInlineNativeMenu() throws {
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        node.handlers.insert("contextmenu")
        node.updateContextGestures()
        p.menus.context.sync()
        XCTAssertEqual(node.contextRecognizer?.isEnabled, true)
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let plain = try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(11))).first)
        let point = CGPoint(x: plain.midX, y: plain.midY)
        XCTAssertNil(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: point))
        var heard: [UInt32] = []
        p.onClipboard = { id, kind, _ in if kind == 10 { heard.append(id) } }
        let gesture = LongPress()
        gesture.point = point
        node.openContext(gesture)
        XCTAssertEqual(heard, [2])
        let tag = try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(10))).first)
        _ = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: tag.midX, y: tag.midY)))
        _ = p.menus.context.menu(try XCTUnwrap(p.menus.context.open))
        XCTAssertEqual(heard, [2, 10])
    }

    func testAnInlineMenuLeavesTheRowsMenuOnSurroundingText() throws {
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        p.apply(wireBatch([
            ["op": "props", "id": 1, "set": ["contextPopover": "row-menu"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["popover": "auto", "id": "row-menu"]],
            ["op": "create", "id": 6, "kind": "button", "handlers": ["press"]],
            ["op": "children", "id": 5, "ids": [6]],
            ["op": "children", "id": 1, "ids": [2, 3, 5]],
        ]))
        let row = try XCTUnwrap(p.views[1]), node = try XCTUnwrap(p.views[2])
        let outer = try XCTUnwrap(row.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let inner = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let plain = try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(11))).first)
        let point = CGPoint(x: plain.midX, y: plain.midY)
        XCTAssertNil(p.menus.context.contextMenuInteraction(inner, configurationForMenuAtLocation: point))
        let config = try XCTUnwrap(p.menus.context.contextMenuInteraction(outer, configurationForMenuAtLocation: row.convert(point, from: node)))
        XCTAssertEqual((config.identifier as? NSNumber)?.uint32Value, row.id)
        XCTAssertEqual(p.menus.context.open?.name, "row-menu")
        let tag = try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(10))).first)
        let tagPoint = CGPoint(x: tag.midX, y: tag.midY)
        XCTAssertNil(p.menus.context.contextMenuInteraction(outer, configurationForMenuAtLocation: row.convert(tagPoint, from: node)))
        let tagConfig = try XCTUnwrap(p.menus.context.contextMenuInteraction(inner, configurationForMenuAtLocation: tagPoint))
        XCTAssertEqual((tagConfig.identifier as? NSNumber)?.uint32Value, 10)
        XCTAssertEqual(p.menus.context.open?.name, "tag-menu")
    }

    func testAnUnresolvedInlinePopoverFallsThroughToTheParagraphHandler() throws {
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        var style = InlineStyle()
        try style.set("font_size", .number(18))
        p.applyParagraph(2, [InlineText(id: 10, parent: 2, props: ["text": "#tag", "contextPopover": "gone"], style: style, handlers: [], paints: true)])
        node.handlers.insert("contextmenu")
        node.updateContextGestures()
        p.menus.context.sync()
        var heard: [UInt32] = []
        p.onContextmenu = { heard.append($0) }
        let agent = Agent(session: try XCTUnwrap(session))
        let request: [String: Any] = ["id": NSNumber(value: 10), "contextmenu": true]
        let reply = agent.tap(request)
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["tapped"] as? Int, 2)
        XCTAssertEqual(heard, [2])
        node.handlers.remove("contextmenu")
        XCTAssertNotNil(agent.tap(request)["error"], "a run with no handler and no live popover cannot claim delivery")
        XCTAssertEqual(heard, [2])
    }

    func testARoundedParagraphKeepsTheInlinePreviewOnItsRun() throws {
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        node.layer.cornerRadius = 12
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let rect = try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(10))).first)
        let config = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: rect.midX, y: rect.midY)))
        let preview = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, previewForHighlightingMenuWithConfiguration: config))
        XCTAssertEqual(try XCTUnwrap(preview.parameters.visiblePath).bounds, rect)
    }

    func testAWrappedRunsContextOffsetsKeepTheTargetsOrigin() throws {
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 20, "y": 100, "w": 120, "h": 220]]))
        var style = InlineStyle()
        try style.set("font_size", .number(18))
        p.applyParagraph(2, [InlineText(id: 10, parent: 2, props: ["text": "hashtag wraps across several lines", "contextPopover": "tag-menu"], style: style, handlers: ["contextmenu"], paints: true)])
        p.menus.context.sync()
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        XCTAssertGreaterThan(rects.count, 2)
        let first = try XCTUnwrap(rects.first), last = try XCTUnwrap(rects.last)
        let point = CGPoint(x: last.midX, y: last.midY)
        var sample: [Substring] = []
        p.onClipboard = { _, kind, line in if kind == 10 { sample = line.split(separator: ",", omittingEmptySubsequences: false) } }
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        _ = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: point))
        _ = p.menus.context.menu(try XCTUnwrap(p.menus.context.open))
        XCTAssertEqual(sample.count, 10)
        guard sample.count == 10 else { return }
        XCTAssertEqual(try XCTUnwrap(Double(sample[0])), point.x - first.minX, accuracy: 0.01)
        XCTAssertEqual(try XCTUnwrap(Double(sample[1])), point.y - first.minY, accuracy: 0.01)
        XCTAssertGreaterThan(try XCTUnwrap(Double(sample[1])), last.height, "offsetY does not restart at each wrapped fragment")
        let client = p.client(node.convert(point, to: nil))
        XCTAssertEqual(try XCTUnwrap(Double(sample[7])), client.x, accuracy: 0.01)
        XCTAssertEqual(try XCTUnwrap(Double(sample[8])), client.y, accuracy: 0.01)
    }

    func testPlainAndInertRunsDoNotOpenTheInlineMenu() throws {
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let plain = try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(11))).first)
        XCTAssertNil(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: plain.midX, y: plain.midY)))
        let tag = try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(10))).first)
        let run = try XCTUnwrap(p.inlineText(10))
        var props = run.props
        props["inert"] = "true"
        var style = InlineStyle()
        try style.set("font_size", .number(18))
        p.applyParagraph(2, [InlineText(id: run.id, parent: run.parent, props: props, style: style, handlers: run.handlers, paints: run.paints), node.inlineText[1]])
        XCTAssertNil(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: tag.midX, y: tag.midY)))
    }
}
#endif
