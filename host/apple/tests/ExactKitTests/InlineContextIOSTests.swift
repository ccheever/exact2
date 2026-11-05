#if os(iOS)
import UIKit
import XCTest
import IOSurface
import CoreVideo
@testable import ExactKit

final class InlineContextIOSTests: XCTestCase {
    private var window: UIWindow!
    private var session: ExactSession?

    override func tearDown() { session?.destroy(); session = nil; super.tearDown() }

    private func presenter() throws -> Presenter {
        let session = ExactApp.shared.makeSession(label: "inline-menu")
        self.session = session
        let p = session.presenter
        // Use the hosted scene; each pixel test declares that prerequisite for `xctest.mjs`.
        let scene = hostedWindowScene()
        window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow()
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
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

    func testARunLiftsAPictureOfItsLinesFromItsPlace() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        waitFor("the paragraph is drawn, as one a person presses is") { node.textRasterReady && node.textRaster != nil }
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let rect = try XCTUnwrap(rects.first)
        let config = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: rect.midX, y: rect.midY)))
        let landed = try XCTUnwrap(p.menus.context.open?.runPreview?.view, "a picture taken as the finger lands, before a press it starts has drawn")
        let preview = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configuration: config, highlightPreviewForItemWithIdentifier: config.identifier))
        let (outline, area) = textLines(rects, in: node)
        XCTAssertTrue(preview.view === landed, "the picture the finger landed on")
        XCTAssertEqual(preview.parameters.visiblePath?.cgPath, outline.cgPath, "outlined as UIKit outlines text lines, with its padding")
        XCTAssertEqual(preview.view.bounds.size, area.size, "a picture of the run's area: UIKit would hide all of a paragraph it lifted")
        XCTAssertEqual((preview.view.layer.mask as? CAShapeLayer)?.path, preview.parameters.visiblePath?.cgPath, "the picture cut to its outline")
        // On the page's one color: its padding is that color, with none of the text beside the run
        let drawn = onScreen(preview.view, previewOnly: true)
        var (cr, cg, cb): (CGFloat, CGFloat, CGFloat) = (0, 0, 0)
        UIColor(cgColor: try XCTUnwrap(p.viewport.layer.backgroundColor)).getRed(&cr, green: &cg, blue: &cb, alpha: nil)
        XCTAssertEqual(pixel(drawn, at: CGPoint(x: area.width / 2, y: 4)), [cr, cg, cb, 1].map { UInt8(($0 * 255).rounded()) }, "the canvas's color in the padding")
        let box = node.convert(rect, to: node.superview).offsetBy(dx: -area.minX, dy: -area.minY)
        XCTAssertEqual(ink(drawn, row: box.midY, from: box.maxX + 2, to: area.width - 2), 0, "none of the text beside the run")
        XCTAssertFalse(preview.view.subviews.contains { $0.isScreens }, "from the paragraph's own text raster, no snapshot of the screen")
        XCTAssertLessThan(difference(preview, over: rects, of: node), 8, "the run's words and what lies under them as the window shows them")
        XCTAssertTrue(preview.target.container === node.superview && preview.target.center == CGPoint(x: area.midX, y: area.midY),
                      "from the run's place in the paragraph's superview")
        p.menus.context.contextMenuInteraction(interaction, willDisplayMenuFor: config, animator: nil)
        XCTAssertEqual(node.alpha, 1, "the paragraph stays shown")
        // The run's text changes while the menu shows
        var style = InlineStyle()
        try style.set("font_size", .number(18))
        p.applyParagraph(2, [
            InlineText(id: 10, parent: 2, props: ["text": "#a-longer-tag", "contextPopover": "tag-menu"], style: style, handlers: ["press", "contextmenu"], paints: true),
            InlineText(id: 11, parent: 2, props: ["text": " plain text"], style: style, handlers: [], paints: true),
        ])
        let returned = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configuration: config, dismissalPreviewForItemWithIdentifier: config.identifier))
        XCTAssertTrue(returned.view === preview.view && returned.target.container === node.superview, "the same picture returns to the paragraph's place")
        XCTAssertEqual(returned.parameters.visiblePath?.cgPath, preview.parameters.visiblePath?.cgPath, "with the path that fits it")
        // UIKit leaves the picture in the window once it has returned it
        window.addSubview(returned.view)
        let animator = Animator()
        p.menus.context.contextMenuInteraction(interaction, willEndFor: config, animator: animator)
        animator.animations.forEach { $0() }
        let home = capture(returned.view)
        XCTAssertEqual(pixel(home, at: CGPoint(x: area.width / 2, y: 4)).last, 0, "it lands without its padding, which would cover the text beside the run")
        animator.completions.forEach { $0() }
        XCTAssertNil(returned.view.superview, "off the screen once the menu has ended")
    }

    func testATranslucentParagraphsRunLiftsItsWordsTranslucent() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        node.alpha = 0.5
        waitFor("the paragraph is drawn") { node.textRasterReady && node.textRaster != nil }
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let preview = try lift(p, node, at: try XCTUnwrap(rects.first))
        XCTAssertLessThan(difference(preview, over: rects, of: node), 8, "its words at the paragraph's opacity, as the window shows them")
    }

    func testATransformedParagraphsRunLiftsFromWhereItShows() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        waitFor("the paragraph is drawn") { node.textRasterReady && node.textRaster != nil }
        // Scale about the paragraph's center, as `PressFeedback` does; translate far enough to keep the run in the window.
        node.transform = CGAffineTransform(translationX: 90, y: 40).scaledBy(x: 1.5, y: 1.5)
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let rect = try XCTUnwrap(rects.first)
        XCTAssertTrue(rects.allSatisfy { window.bounds.contains(node.convert($0, to: window)) }, "the window shows every pixel compared with the lifted run")
        let config = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: rect.midX, y: rect.midY)))
        let preview = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configuration: config, highlightPreviewForItemWithIdentifier: config.identifier))
        let area = textLines(rects, in: node).area
        XCTAssertEqual(preview.target.center, CGPoint(x: area.midX, y: area.midY), "the run's place, where the screen shows it")
        XCTAssertEqual(preview.view.bounds.size, area.size, "a picture at the screen's scale")
        XCTAssertEqual(preview.target.transform, .identity, "so the target adds nothing")
        XCTAssertLessThan(difference(preview, over: rects, of: node), 8, "its words scaled as the window shows them")
    }

    func testAWrappedRunLiftsAllItsLines() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        var style = InlineStyle()
        try style.set("font_size", .number(18))
        func text(_ run: String) {
            p.applyParagraph(2, [
                InlineText(id: 10, parent: 2, props: ["text": run, "contextPopover": "tag-menu"], style: style, handlers: ["press", "contextmenu"], paints: true),
                InlineText(id: 11, parent: 2, props: ["text": " plain text"], style: style, handlers: [], paints: true),
            ])
        }
        text("#tag with enough words in it to wrap onto a second line")
        let node = try XCTUnwrap(p.views[2])
        // A panel under part of the paragraph: what lies beneath is not one color, so it is drawn
        let panel = UIView(frame: CGRect(x: node.frame.minX, y: node.frame.minY - 20, width: node.frame.width / 2, height: node.frame.height + 40))
        panel.backgroundColor = .systemYellow
        node.superview?.insertSubview(panel, belowSubview: node)
        waitFor("the paragraph is drawn") { node.textRasterReady && node.textRaster != nil }
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        XCTAssertGreaterThanOrEqual(rects.count, 2, "the run wraps")
        let rect = try XCTUnwrap(rects.first)
        let config = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: rect.midX, y: rect.midY)))
        let preview = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configuration: config, highlightPreviewForItemWithIdentifier: config.identifier))
        let (outline, area) = textLines(rects, in: node)
        XCTAssertEqual(preview.parameters.visiblePath?.cgPath, outline.cgPath, "UIKit's outline of every line")
        XCTAssertEqual(preview.view.bounds.size, area.size, "one picture over all of them")
        XCTAssertFalse(try XCTUnwrap(preview.view.subviews.first).layer.sublayers?.isEmpty ?? true, "what lies under it, copied from the layer tree")
        let drawn = onScreen(preview.view)
        let last = node.convert(try XCTUnwrap(rects.last), to: node.superview).offsetBy(dx: -area.minX, dy: -area.minY)
        XCTAssertEqual(ink(drawn, row: last.midY, from: last.maxX + 2, to: area.width - 2), 0, "none of the text after it on its last line")
        let yellow = pixel(drawn, at: CGPoint(x: panel.frame.minX + 20 - area.minX, y: 3))
        XCTAssertTrue(yellow[0] > 200 && yellow[1] > 150 && yellow[2] < 80, "the panel under its first line, copied: \(yellow)")
        // The run shortens to one line while the menu shows
        text("#tag")
        let now = textLines(node.inlineRects(try XCTUnwrap(p.inlineText(10))), in: node).area
        let returned = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configuration: config, dismissalPreviewForItemWithIdentifier: config.identifier))
        XCTAssertTrue(returned.view === preview.view, "the same picture")
        XCTAssertEqual(returned.target.center, CGPoint(x: now.midX, y: now.midY), "returns to where the run is now")
    }

    func testARunOnAnOpaqueCoverOverAPictureLiftsOnTheCoversColor() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        // A picture under the paragraph, then an opaque panel over it that covers the run's area: nothing beneath shows
        let picture = UIImageView(image: UIGraphicsImageRenderer(size: CGSize(width: 8, height: 8)).image { UIColor.systemRed.setFill(); $0.fill(CGRect(x: 0, y: 0, width: 4, height: 8)) })
        picture.frame = node.frame.insetBy(dx: -30, dy: -30)
        let cover = UIView(frame: node.frame.insetBy(dx: -24, dy: -24))
        cover.backgroundColor = UIColor(red: 0, green: 0.5, blue: 1, alpha: 1)
        node.superview?.insertSubview(picture, belowSubview: node)
        node.superview?.insertSubview(cover, belowSubview: node)
        waitFor("the paragraph is drawn") { node.textRasterReady && node.textRaster != nil }
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let rect = try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(10))).first)
        let config = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: rect.midX, y: rect.midY)))
        let preview = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configuration: config, highlightPreviewForItemWithIdentifier: config.identifier))
        let area = textLines([rect], in: node).area
        let drawn = onScreen(preview.view)
        XCTAssertTrue(try XCTUnwrap(preview.view.subviews.first).layer.sublayers?.isEmpty ?? true, "one flat color, no copies of layers")
        XCTAssertEqual(pixel(drawn, at: CGPoint(x: area.width / 2, y: 4)), [0, 128, 255, 255], "the cover's color in the padding")
        // A badge listed before the cover but raised over it (`z-index`): composited above, so the color is not flat
        let badge = UIView(frame: CGRect(x: rect.maxX + 22, y: node.frame.minY + 2, width: 6, height: 6))
        badge.backgroundColor = .systemRed
        badge.layer.zPosition = 1
        node.superview?.insertSubview(badge, belowSubview: cover)
        let raised = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: rect.midX, y: rect.midY)))
        let over = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configuration: raised, highlightPreviewForItemWithIdentifier: raised.identifier))
        XCTAssertFalse(try XCTUnwrap(over.view.subviews.first).layer.sublayers?.isEmpty ?? true, "copies of the layers there")
        let spot = badge.convert(CGPoint(x: 3, y: 3), to: node.superview), raisedArea = textLines([rect], in: node).area
        let red = pixel(onScreen(over.view), at: CGPoint(x: spot.x - raisedArea.minX, y: spot.y - raisedArea.minY))
        XCTAssertTrue(red[0] > 200 && red[1] < 120 && red[2] < 120, "the badge's copy over the cover's, by its `zPosition`: \(red)")
    }

    func testARunNearItsCardsEdgeLiftsOnTheCardExtended() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let (node, card) = try cardAround(p, margin: 4)
        card.backgroundColor = UIColor(red: 1, green: 0.84, blue: 0.04, alpha: 1)
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let preview = try lift(p, node, at: try XCTUnwrap(rects.first))
        let outline = try XCTUnwrap(UIPreviewParameters(textLineRects: rects.map { NSValue(cgRect: node.convert($0, to: preview.target.container)) }).visiblePath)
        let area = outline.bounds
        outline.apply(CGAffineTransform(translationX: -area.minX, y: -area.minY))
        XCTAssertEqual(preview.view.bounds.size, area.size, "UIKit's footprint, though its padding reaches past the card")
        XCTAssertEqual(preview.parameters.visiblePath?.cgPath, outline.cgPath, "outlined as UIKit outlines text lines")
        XCTAssertTrue(try XCTUnwrap(preview.view.subviews.first).layer.sublayers?.isEmpty ?? true, "one flat color, the card's, no copies of layers")
        let drawn = onScreen(preview.view)
        XCTAssertEqual(pixel(drawn, at: CGPoint(x: 2, y: area.height / 2)), [255, 214, 10, 255], "the card's color past its edge, none of the page")
    }

    func testARunNearAPictureCardsEdgeExtendsThePicture() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let (node, card) = try cardAround(p, margin: 4)
        // A card that clips a picture, red on the left and blue on the right, under the paragraph
        card.clipsToBounds = true
        let picture = UIImageView(image: UIGraphicsImageRenderer(size: CGSize(width: 8, height: 8)).image { c in
            UIColor.red.setFill(); c.fill(CGRect(x: 0, y: 0, width: 4, height: 8))
            UIColor.blue.setFill(); c.fill(CGRect(x: 4, y: 0, width: 4, height: 8))
        })
        picture.frame = card.bounds
        card.insertSubview(picture, belowSubview: node)
        // A blur far from the run, earlier in the tree, and many views away from it: neither reaches the picture
        let page = try XCTUnwrap(card.superview), away = UIVisualEffectView(effect: UIBlurEffect(style: .systemMaterial))
        away.frame = CGRect(x: 0, y: 600, width: 40, height: 40)
        page.insertSubview(away, at: 0)
        for i in 0..<200 {
            let far = UIView(frame: CGRect(x: CGFloat(i % 10) * 30, y: 500 + CGFloat(i / 10) * 8, width: 20, height: 4))
            far.addSubview(UIView(frame: far.bounds))
            page.insertSubview(far, at: 0)
        }
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let preview = try lift(p, node, at: try XCTUnwrap(rects.first))
        let area = textLines(rects, in: node).area
        XCTAssertEqual(preview.view.bounds.size, area.size, "UIKit's footprint")
        func count(_ layer: CALayer) -> Int { 1 + (layer.sublayers ?? []).map(count).reduce(0, +) }
        XCTAssertLessThan(count(preview.view.layer), 40, "copies of only what reaches the run's area")
        let drawn = onScreen(preview.view)
        XCTAssertEqual(pixel(drawn, at: CGPoint(x: 2, y: area.height / 2)), [255, 0, 0, 255], "the picture reflected past the card, none of the page")
        // A blur over the picture: live over the copies and over all of the padding, so the band past the card is blurred too
        let blur = UIVisualEffectView(effect: UIBlurEffect(style: .systemMaterial))
        blur.frame = card.bounds
        card.insertSubview(blur, belowSubview: node)
        _ = capture(window)
        let screen = try lift(p, node, at: try XCTUnwrap(rects.first))
        let shown = onScreen(screen.view)
        let band = pixel(shown, at: CGPoint(x: 2, y: area.height / 2)), inside = pixel(shown, at: CGPoint(x: 13.5, y: area.height / 2))
        XCTAssertTrue(band.count == 4 && zip(band, inside).allSatisfy { abs(Int($0) - Int($1)) <= 3 } && band[1] > 60,
                      "the blur past the card, blurred and as beside it (\(band) beside \(inside)), none of the page")
    }

    func testARunNearAScreenDrawnCardsEdgeStretchesTheScreensEdge() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let (node, card) = try cardAround(p, margin: 4)
        // A card that clips a layer only the render server composites (a blend mode), under the paragraph
        card.clipsToBounds = true
        let blend = CALayer()
        blend.frame = card.bounds
        blend.backgroundColor = UIColor.systemTeal.cgColor
        blend.compositingFilter = "multiplyBlendMode"
        card.layer.insertSublayer(blend, below: node.layer)
        // Include the blend in the rendered frame before the fallback takes its snapshot.
        _ = capture(window)
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let preview = try lift(p, node, at: try XCTUnwrap(rects.first))
        let area = textLines(rects, in: node).area
        let padding = try XCTUnwrap(preview.view.subviews.first)
        XCTAssertTrue(padding.isScreens && !(padding.layer.sublayers ?? []).filter { $0.contents != nil }.isEmpty, "the screen's picture, with strips of it over the band")
        let shown = onScreen(preview.view)
        let band = pixel(shown, at: CGPoint(x: 2, y: area.height / 2)), inside = pixel(shown, at: CGPoint(x: 13.5, y: area.height / 2))
        XCTAssertTrue(band.count == 4 && band[3] > 0 && zip(band, inside).allSatisfy { abs(Int($0) - Int($1)) <= 3 } && band != [255, 255, 255, 255],
                      "the card's edge stretched past it (\(band) beside \(inside)), none of the page")
    }

    func testARunInAScaledCardLiftsItsPictureInPlace() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let (node, card) = try cardAround(p, margin: 20)
        // An opaque card that clips a picture of stripes, scaled about its center: the root of the copies, whose scale places them
        (card.clipsToBounds, card.backgroundColor) = (true, .white)
        let picture = UIImageView(image: UIGraphicsImageRenderer(size: CGSize(width: 160, height: 8)).image { c in
            for i in 0..<40 { (i % 2 == 0 ? UIColor.red : UIColor.blue).setFill(); c.fill(CGRect(x: i * 4, y: 0, width: 4, height: 8)) }
        })
        picture.frame = card.bounds
        card.insertSubview(picture, belowSubview: node)
        // Moved right as it grows, so the run stays in the window
        card.transform = CGAffineTransform(translationX: 60, y: 20).scaledBy(x: 1.25, y: 1.25)
        waitFor("the card is drawn scaled") { card.layer.presentation()?.transform.m11 == 1.25 }
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let preview = try lift(p, node, at: try XCTUnwrap(rects.first))
        XCTAssertLessThan(difference(preview, over: rects, of: node), 8, "the copies where the stripes show, scaled")
    }

    /// The paragraph moved into a card with `margin` around it, less than UIKit's padding, under a root controller,
    /// whose view the preview rises over, as in an app
    private func cardAround(_ p: Presenter, margin: CGFloat) throws -> (NodeView, UIView) {
        let host = UIViewController()
        window.rootViewController = host
        host.view.frame = window.bounds
        host.view.addSubview(p.viewport)
        let node = try XCTUnwrap(p.views[2]), page = try XCTUnwrap(node.superview)
        let card = UIView(frame: node.frame.insetBy(dx: -margin, dy: -margin))
        card.layer.cornerRadius = 8
        page.insertSubview(card, belowSubview: node)
        card.addSubview(node)
        node.frame.origin = CGPoint(x: margin, y: margin)
        waitFor("the paragraph is drawn") { node.textRasterReady && node.textRaster != nil }
        return (node, card)
    }

    /// The preview UIKit lifts for a long press on `rect` of `node`
    private func lift(_ p: Presenter, _ node: NodeView, at rect: CGRect) throws -> UITargetedPreview {
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let config = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: rect.midX, y: rect.midY)))
        return try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configuration: config, highlightPreviewForItemWithIdentifier: config.identifier))
    }

    func testARunOverABlurShowsTheBlurLive() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        // A blur under the paragraph, which only the render server draws
        let blur = UIVisualEffectView(effect: UIBlurEffect(style: .systemMaterial))
        blur.frame = node.frame.insetBy(dx: -20, dy: -20)
        node.superview?.insertSubview(blur, belowSubview: node)
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let rect = try XCTUnwrap(rects.first)
        // A badge over the blur, in the run's padding: composited after it, so never under its live copy
        let area = textLines(rects, in: node).area
        let badge = UIView(frame: CGRect(x: node.frame.minX + rect.maxX + 3, y: area.minY + 2, width: 6, height: 6))
        badge.backgroundColor = .red
        node.superview?.insertSubview(badge, aboveSubview: blur)
        waitFor("the paragraph is drawn") { node.textRasterReady && node.textRaster != nil }
        let config = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: rect.midX, y: rect.midY)))
        let preview = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configuration: config, highlightPreviewForItemWithIdentifier: config.identifier))
        let padding = try XCTUnwrap(preview.view.subviews.first)
        XCTAssertFalse(padding.isScreens, "no snapshot of the screen")
        func live(_ view: UIView) -> [UIVisualEffectView] { view.subviews.flatMap { ($0 as? UIVisualEffectView).map { [$0] } ?? live($0) } }
        XCTAssertTrue(live(padding).contains { $0.effect is UIBlurEffect }, "the blur live over copies of what lies under it")
        XCTAssertEqual(preview.view.bounds.size, area.size)
        let red = pixel(onScreen(preview.view), at: CGPoint(x: badge.frame.midX - area.minX, y: badge.frame.midY - area.minY))
        XCTAssertTrue(red[0] > 220 && red[1] < 60 && red[2] < 60, "the badge's copy over the live blur, unblurred: \(red)")
        let animator = Animator()
        p.menus.context.contextMenuInteraction(interaction, willEndFor: config, animator: animator)
        animator.animations.forEach { $0() }
        func faded(_ view: UIView) -> Bool {
            if let effect = view as? UIVisualEffectView { return effect.effect == nil }
            return live(view).isEmpty ? view.alpha == 0 : view.alpha == 1 && view.subviews.allSatisfy(faded)
        }
        XCTAssertTrue(padding.subviews.allSatisfy(faded), "it lands without its padding, the live blur losing its effect as UIKit animates it, the copies fading, and nothing that holds the blur faded")
    }

    func testARunOverAViewThatDrawsItselfTakesTheScreen() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a fallback snapshot needs a scene's rendered window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        // A view under the paragraph whose backing store holds its drawing, which a committed copy must not share
        let drawn = Drawn(frame: node.frame.insetBy(dx: -20, dy: -20))
        node.superview?.insertSubview(drawn, belowSubview: node)
        waitFor("the view has drawn") { drawn.layer.contents != nil }
        _ = capture(window)
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let preview = try lift(p, node, at: try XCTUnwrap(rects.first))
        XCTAssertTrue(try XCTUnwrap(preview.view.subviews.first).isScreens, "the screen's picture, no copy of the view's backing store")
    }

    func testAParagraphsOwnContentsLeaveItsPaddingOneColor() throws {
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        waitFor("the paragraph is drawn") { node.textRasterReady && node.textRaster != nil }
        // Contents that are no bitmap, as a paragraph that draws its own text has: never shared, since only its fill is copied
        node.layer.contents = IOSurface(properties: [.width: 4, .height: 4, .bytesPerElement: 4, .pixelFormat: kCVPixelFormatType_32BGRA])
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let padding = try XCTUnwrap(lift(p, node, at: try XCTUnwrap(rects.first)).view.subviews.first)
        XCTAssertTrue(!padding.isScreens && (padding.layer.sublayers?.isEmpty ?? true), "one flat color, as the page is, with none of the text beside the run")
    }

    func testANodeCardsStyleMakesItTheBoxInsideItsBorder() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        // Under a root controller, whose view the preview rises over, as in an app
        let host = UIViewController()
        window.rootViewController = host
        host.view.frame = window.bounds
        host.view.addSubview(p.viewport)
        // The paragraph in a node with 4 pt of room: a yellow fill its layer does not hold (two corners rounded unequally,
        // so its corners' fill layer has it) and a 4 pt red border, wider than the corners' inset of `inner`. Its style makes it the box.
        p.apply(wireBatch([
            ["op": "create", "id": 5, "kind": "view", "style": ["background_color": [255, 214, 10, 255], "border_radius_top_left": 3, "border_radius_bottom_left": 5,
                                                              "border_width": 4, "border_color_top": [255, 0, 0, 255]]],
            ["op": "children", "id": 5, "ids": [2]], ["op": "children", "id": 1, "ids": [5, 3]],
            ["op": "frame", "id": 5, "x": 16, "y": 96, "w": 308, "h": 68],
            ["op": "frame", "id": 2, "x": 4, "y": 4, "w": 300, "h": 60],
        ]))
        p.menus.context.sync()
        let node = try XCTUnwrap(p.views[2]), card = try XCTUnwrap(p.views[5])
        waitFor("the paragraph is drawn") { node.textRasterReady && node.textRaster != nil }
        XCTAssertTrue(card.boxFill != nil && card.layer.backgroundColor == nil, "the fill on the corners' fill layer, not the card's")
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let preview = try lift(p, node, at: try XCTUnwrap(rects.first))
        let area = textLines(rects, in: node).area
        XCTAssertEqual(preview.view.bounds.size, area.size, "UIKit's footprint")
        // Just past the card's edge (in the card's own coordinates, as `area` is), which a reflection from the border's
        // inner side would make red, and the page white
        let edge = card.bounds.minX - area.minX
        let band = pixel(onScreen(preview.view), at: CGPoint(x: edge - 1, y: area.height / 2))
        XCTAssertTrue(band[0] > 220 && band[1] > 180 && band[2] < 80, "the card's yellow past its edge, not its border's red or the page: \(band)")
    }

    func testABorderedCardExtendsItsFillNotItsBorder() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let (node, card) = try cardAround(p, margin: 4)
        // A yellow card with a red border, 2 pt wide, inside the room UIKit's padding reaches past
        // Square, so no corner keeps `inner` off the border
        (card.backgroundColor, card.layer.cornerRadius) = (UIColor(red: 1, green: 0.84, blue: 0.04, alpha: 1), 0)
        (card.layer.borderWidth, card.layer.borderColor) = (2, UIColor.red.cgColor)
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let preview = try lift(p, node, at: try XCTUnwrap(rects.first))
        // Just past the card's edge, which a reflection from the border's inner side would make red
        let edge = card.convert(card.bounds, to: node.superview).minX - textLines(rects, in: node).area.minX
        let band = pixel(onScreen(preview.view), at: CGPoint(x: edge - 1, y: textLines(rects, in: node).area.height / 2))
        XCTAssertTrue(band[0] > 220 && band[1] > 180 && band[2] < 80, "the card's yellow past its edge, not its border's red: \(band)")
    }

    func testAnOverflowingChildOfATranslucentParentLiftsTranslucent() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a lifted preview's pixels need a scene's window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2]), page = try XCTUnwrap(node.superview)
        // A half-transparent parent away from the run, whose red child reaches under it
        let parent = UIView(frame: CGRect(x: 0, y: node.frame.maxY + 200, width: 20, height: 20))
        parent.alpha = 0.5
        let child = UIView(frame: CGRect(x: node.frame.minX - 10, y: node.frame.minY - parent.frame.minY - 6, width: node.frame.width + 20, height: node.frame.height + 12))
        child.backgroundColor = .red
        parent.addSubview(child)
        page.insertSubview(parent, belowSubview: node)
        waitFor("the paragraph is drawn") { node.textRasterReady && node.textRaster != nil }
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let preview = try lift(p, node, at: try XCTUnwrap(rects.first))
        XCTAssertLessThan(difference(preview, over: rects, of: node), 8, "the child's copy at its parent's opacity, as the window shows it")
    }

    func testARunOverAFilteredLayerLiftsTheScreen() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "a fallback snapshot needs a scene's rendered window") }
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        let node = try XCTUnwrap(p.views[2])
        // A layer under the paragraph that only the render server composites: a blend mode
        let blend = CALayer()
        blend.frame = node.frame.insetBy(dx: -20, dy: -20)
        blend.backgroundColor = UIColor.systemTeal.cgColor
        blend.compositingFilter = "multiplyBlendMode"
        node.superview?.layer.insertSublayer(blend, below: node.layer)
        waitFor("the paragraph is drawn") { node.textRasterReady && node.textRaster != nil }
        _ = capture(window)
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let rects = node.inlineRects(try XCTUnwrap(p.inlineText(10)))
        let rect = try XCTUnwrap(rects.first)
        let config = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: rect.midX, y: rect.midY)))
        let preview = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configuration: config, highlightPreviewForItemWithIdentifier: config.identifier))
        XCTAssertTrue(try XCTUnwrap(preview.view.subviews.first).isScreens, "the screen's own picture, not one the host made")
        XCTAssertEqual(preview.view.bounds.size, textLines(rects, in: node).area.size)
        let animator = Animator()
        p.menus.context.contextMenuInteraction(interaction, willEndFor: config, animator: animator)
        animator.animations.forEach { $0() }
        XCTAssertEqual(try XCTUnwrap(preview.view.subviews.first).alpha, 0, "it lands as the screen is then, the screen's picture of the area faded, as what lies there may have changed")
    }

    func testARunLiftsOverTheRootControllersView() throws {
        let p = try presenter()
        defer { p.menus.reset(); window.isHidden = true }
        // An app with no routes: its root controller holds the view
        let host = UIViewController()
        window.rootViewController = host
        host.view.frame = window.bounds
        host.view.addSubview(p.viewport)
        let node = try XCTUnwrap(p.views[2])
        waitFor("the paragraph is drawn") { node.textRasterReady && node.textRaster != nil }
        let interaction = try XCTUnwrap(node.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let rect = try XCTUnwrap(node.inlineRects(try XCTUnwrap(p.inlineText(10))).first)
        let config = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: rect.midX, y: rect.midY)))
        let preview = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configuration: config, highlightPreviewForItemWithIdentifier: config.identifier))
        XCTAssertTrue(preview.target.container === host.view, "over the root controller's view, above every clip in it")
    }

    /// UIKit's outline of a run's lines where the screen shows them, in the paragraph's superview, in its own
    /// coordinates, and its area
    private func textLines(_ rects: [CGRect], in node: UIView) -> (outline: UIBezierPath, area: CGRect) {
        let outline = UIPreviewParameters(textLineRects: rects.map { NSValue(cgRect: node.convert($0, to: node.superview)) }).visiblePath ?? UIBezierPath()
        let area = outline.bounds
        outline.apply(CGAffineTransform(translationX: -area.minX, y: -area.minY))
        return (outline, area)
    }

    /// How many pixels of `image` along the row at `y`, from `start` to `end` (points), are dark enough to be text: their
    /// luminance, not one channel, as a yellow fill has no blue
    private func ink(_ image: UIImage, row y: CGFloat, from start: CGFloat, to end: CGFloat) -> Int {
        guard let (bytes, width, height) = rgba(image) else { return -1 }
        let row = min(height - 1, Int(y * image.scale)), columns = max(0, Int(start * image.scale))..<min(width, Int(end * image.scale))
        return columns.filter { x in
            let i = (row * width + x) * 4
            return bytes[i + 3] > 128 && 0.299 * Double(bytes[i]) + 0.587 * Double(bytes[i + 1]) + 0.114 * Double(bytes[i + 2]) < 100
        }.count
    }

    /// `image`'s pixel at `point` (points), RGBA
    private func pixel(_ image: UIImage, at point: CGPoint) -> [UInt8] {
        guard let (bytes, width, height) = rgba(image) else { return [] }
        let i = (min(height - 1, Int(point.y * image.scale)) * width + min(width - 1, Int(point.x * image.scale))) * 4
        return Array(bytes[i..<i + 4])
    }

    private func rgba(_ image: UIImage) -> (bytes: [UInt8], width: Int, height: Int)? {
        guard let picture = image.cgImage else { return nil }
        let width = picture.width, height = picture.height
        var bytes = [UInt8](repeating: 0, count: width * height * 4)
        guard let context = CGContext(data: &bytes, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
                                      space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        context.draw(picture, in: CGRect(x: 0, y: 0, width: width, height: height))
        return (bytes, width, height)
    }

    /// The mean difference in luminance, 0 to 255, between the lifted picture and the window over the run's boxes,
    /// both as the render server draws them
    private func difference(_ preview: UITargetedPreview, over rects: [CGRect], of node: NodeView) -> Double {
        let container = preview.target.container, size = preview.view.bounds.size
        let area = CGRect(x: preview.target.center.x - size.width / 2, y: preview.target.center.y - size.height / 2, width: size.width, height: size.height)
        let shown = container.convert(area, to: window), image = capture(window)
        let truth = UIGraphicsImageRenderer(size: size).image { _ in
            image.draw(at: CGPoint(x: -shown.minX, y: -shown.minY))
        }
        guard let (a, width, height) = rgba(truth), let (b, w, h) = rgba(onScreen(preview.view)), width == w, height == h else { return .infinity }
        let scale = truth.scale
        var total = 0.0, count = 0
        for rect in rects {
            let box = node.convert(rect, to: container).offsetBy(dx: -area.minX, dy: -area.minY)
            for y in max(0, Int(box.minY * scale))..<min(height, Int(box.maxY * scale)) {
                for x in max(0, Int(box.minX * scale))..<min(width, Int(box.maxX * scale)) {
                    let i = (y * width + x) * 4
                    guard a[i + 3] > 0, b[i + 3] > 0 else { return .infinity }
                    func luminance(_ p: [UInt8]) -> Double { 0.299 * Double(p[i]) + 0.587 * Double(p[i + 1]) + 0.114 * Double(p[i + 2]) }
                    total += abs(luminance(a) - luminance(b))
                    count += 1
                }
            }
        }
        return count == 0 ? .infinity : total / Double(count)
    }

    /// Capture the whole window and crop to the preview; capture only the preview to inspect its alpha.
    private func onScreen(_ view: UIView, previewOnly: Bool = false) -> UIImage {
        XCTAssertNotNil(window.windowScene, "the capture needs the hosted app's scene")
        let frame = view.frame, parent = view.superview, index = parent?.subviews.firstIndex(of: view)
        window.addSubview(view)
        view.frame.origin = .zero
        defer {
            view.removeFromSuperview()
            if let parent, let index { parent.insertSubview(view, at: index) }
            view.frame = frame
        }
        if previewOnly { return capture(view) }
        let image = capture(window)
        return UIGraphicsImageRenderer(bounds: view.bounds).image { _ in image.draw(at: .zero) }
    }

    private func capture(_ view: UIView) -> UIImage {
        var complete = false
        let image = UIGraphicsImageRenderer(bounds: view.bounds).image { _ in complete = view.drawHierarchy(in: view.bounds, afterScreenUpdates: true) }
        XCTAssertTrue(complete, "UIKit captures every view's image data after pending screen updates")
        return image
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

private extension UIView {
    /// Whether this is a picture of the screen (`resizableSnapshotView`'s `_UIReplicantView`) or holds one first
    var isScreens: Bool { String(describing: type(of: self)).contains("Replicant") || subviews.first?.isScreens == true }
}

/// A view that draws itself, so its layer's contents are its backing store
private final class Drawn: UIView {
    override func draw(_ rect: CGRect) { UIColor.systemOrange.setFill(); UIRectFill(bounds) }
}

/// An animator that keeps what it is given, for a test to run as UIKit runs it: alongside its animation and after it
private final class Animator: NSObject, UIContextMenuInteractionAnimating {
    var animations: [() -> Void] = [], completions: [() -> Void] = []
    var previewViewController: UIViewController? { nil }
    func addAnimations(_ animations: @escaping () -> Void) { self.animations.append(animations) }
    func addCompletion(_ completion: @escaping () -> Void) { completions.append(completion) }
}
#endif
