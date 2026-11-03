#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// While UIKit dims tints behind a sheet or an alert, the tint greys wherever it is drawn, as UIKit
/// greys a view's `tintColor`, and a colour of its own keeps it (HIG, Color: a colour that says a
/// control is interactive is not one for what is not). UIKit draws the colours Exact hands it in
/// the tint (`SystemColor.tint`); Exact paints its own from the view's `tintColor` again as UIKit
/// reports the change. UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class TintDimmingIOSTests: XCTestCase {
    private var window: UIWindow!
    private let red = [255, 59, 48, 255], green = [52, 199, 89, 255], blue = [0, 122, 255, 255]
    private let tint: [String: Any] = ["sys": "@tint", "c": [0, 122, 255, 255]]

    /// A colour of its own keeps it while dimmed, on a control's face too: a pressable box's fill
    /// and shadow, its label, a label in display-p3, a path icon in its colour and a hued HDR fill
    func testAColourOfItsOwnKeepsItWhileDimmed() throws {
        let p3: [String: Any] = ["c": [255, 0, 0, 255], "cs": [["s": "display-p3", "v": [1.0, 0.0, 0.0, 1.0]]]]
        let glow: [[String: Any]] = [["o": [0.0, 2.0], "b": 6.0, "c": green]]
        let p = presenter([
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press"],
             "style": ["background_color": blue, "box_shadow": glow, "written": ["background_color", "box_shadow"]]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Delete"], "style": ["text_color": red]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Wide"], "style": ["text_color": p3]],
        ] + icon(4, ["text_color": red]) + [
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "frame", "id": 1, "x": 10.0, "y": 10.0, "w": 240.0, "h": 44.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 22.0],
            ["op": "frame", "id": 3, "x": 100.0, "y": 0.0, "w": 100.0, "h": 22.0],
            ["op": "roots", "ids": [1]],
        ])
        let button = try XCTUnwrap(p.views[1]), label = try XCTUnwrap(p.views[2]), wide = try XCTUnwrap(p.views[3]), symbol = try symbolView(p.views[4])
        let hdr: [String: Any] = ["cs": [["s": "srgb-linear", "v": [4.0, 0.5, 0.5, 1.0]]], "c": [255, 188, 188, 255]]
        p.apply(wireBatch([["op": "create", "id": 5, "kind": "view", "handlers": ["press"], "style": ["background_color": hdr, "written": ["background_color"]]],
                           ["op": "frame", "id": 5, "x": 10.0, "y": 60.0, "w": 100.0, "h": 44.0], ["op": "roots", "ids": [1, 5]]]))
        let bright = try XCTUnwrap(p.views[5])
        dim(p, true)
        bright.layer.displayIfNeeded()
        XCTAssertEqual(try XCTUnwrap(bright.layer.backgroundColor).components, [4, 0.5, 0.5, 1], "a hued HDR fill, in its own space as written")
        assertChannels(try fill(button), blue, "the button's fill")
        assertChannels(try shadow(button), green, "the button's shadow")
        assertChannels(try runChannels(label), red, "the button's red label")
        XCTAssertEqual(Array(try runChannels(wide).dropFirst(4)), [1, 1, 0, 0, 1], "the display-p3 label in its own space (`textChannels`' code 1)")
        assertChannels(try components(symbol.tintColor.cgColor), red, "the icon's colour")
        XCTAssertEqual(symbol.tintAdjustmentMode, .normal, "UIKit leaves the icon's own colour as it is")
        dim(p, false)
    }

    /// The tint greys wherever it is drawn, as UIKit greys it, and comes back: a pressable box's fill
    /// and its label, text outside any control (a status), a shadow, and text first shown while dimmed
    func testTheTintGreysWhereverItIsDrawn() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"], "style": ["background_color": tint, "written": ["background_color"]]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Go"], "style": ["text_color": tint]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Parked"], "style": ["text_color": tint]],
            ["op": "create", "id": 4, "kind": "view",
             "style": ["background_color": green, "box_shadow": [["o": [0.0, 2.0], "b": 6.0, "c": tint]], "written": ["background_color", "box_shadow"]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 22.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 50.0, "w": 100.0, "h": 22.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 80.0, "w": 100.0, "h": 40.0],
            ["op": "roots", "ids": [1, 3, 4]],
        ])
        p.viewport.tintColor = color(red)
        let button = try XCTUnwrap(p.views[1]), label = try XCTUnwrap(p.views[2]), status = try XCTUnwrap(p.views[3]), box = try XCTUnwrap(p.views[4])
        func check(_ expected: [Int], _ when: String) throws {
            assertChannels(try fill(button), expected, "the button's fill \(when)")
            assertChannels(try runChannels(label), expected, "its label \(when)")
            assertChannels(try runChannels(status), expected, "the status \(when)")
            assertChannels(try shadow(box), expected, "the shadow \(when)")
        }
        try check(red, "before the dimming")
        dim(p, true)
        let grey = uikitGrey(red, uikitAlpha: true)
        try check(grey, "while dimmed")
        p.apply(wireBatch([
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Locked"], "style": ["text_color": tint]],
            ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 100.0, "h": 22.0],
            ["op": "children", "id": 4, "ids": [5]],
        ]))
        assertChannels(try runChannels(XCTUnwrap(p.views[5])), grey, "a status first shown while dimmed")
        dim(p, false)
        try check(red, "once the dimming ends")
    }

    /// A symbol in the tint has no tint of its own: UIKit draws it in the tint its node inherits and
    /// greys it. One in its own colour keeps it.
    func testASymbolInTheTintIsUIKitsToGrey() throws {
        let p = presenter(icon(1, ["text_color": tint]) + icon(2, ["text_color": red]) + [["op": "roots", "ids": [1, 2]]])
        p.viewport.tintColor = color(green)
        let inTint = try symbolView(p.views[1]), own = try symbolView(p.views[2])
        assertChannels(try components(inTint.tintColor.cgColor), green, "the symbol in the tint its node inherits")
        dim(p, true)
        XCTAssertEqual(inTint.tintAdjustmentMode, .dimmed, "UIKit dims the symbol in the tint")
        XCTAssertEqual(own.tintAdjustmentMode, .normal, "and leaves the one in its own colour")
        assertChannels(try components(inTint.tintColor.cgColor), uikitGrey(green, uikitAlpha: true), "the symbol in UIKit's grey")
        assertChannels(try components(own.tintColor.cgColor), red, "the symbol in its own colour")
        dim(p, false)
        assertChannels(try components(inTint.tintColor.cgColor), green, "the symbol in the tint once the dimming ends")
    }

    /// The colours Exact hands UIKit in the tint are UIKit's tint (`UIColor.tintColor`), a share of it
    /// for `Highlight`, one object each, which UIKit resolves where it draws: a native field's text
    /// and placeholder in the tint grey with it, and its written ones keep their colour
    func testTheColoursHandedToUIKitInTheTintAreItsTint() throws {
        // A session's presenter: a field takes its font and colour from its text engine
        let session = ExactApp.shared.makeSession(label: "tint-field-\(UUID().uuidString)")
        defer { session.destroy() }
        let p = session.presenter
        mount(p)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"]],
            ["op": "create", "id": 2, "kind": "input", "handlers": ["change"], "props": ["value": "Garage"], "style": ["text_color": red, "written": ["text_color"]]],
            ["op": "create", "id": 3, "kind": "input", "handlers": ["change"], "props": ["value": "Home"], "style": ["text_color": tint, "written": ["text_color"]]],
            ["op": "create", "id": 4, "kind": "input", "handlers": ["change"], "props": ["value": "Work"],
             "style": ["text_color": ["sys": "@tint/0.2", "c": [0, 122, 255, 51]], "written": ["text_color"]]],
            ["op": "create", "id": 5, "kind": "input", "handlers": ["change"], "props": ["placeholder": "Garage"], "style": ["placeholder_color": red]],
            ["op": "create", "id": 6, "kind": "input", "handlers": ["change"], "props": ["placeholder": "Home"], "style": ["placeholder_color": tint]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 200.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 34.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 40.0, "w": 200.0, "h": 34.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 80.0, "w": 200.0, "h": 34.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 120.0, "w": 200.0, "h": 34.0],
            ["op": "frame", "id": 6, "x": 0.0, "y": 160.0, "w": 200.0, "h": 34.0],
            ["op": "roots", "ids": [1]],
        ]))
        p.viewport.tintColor = color(red)
        let home = try XCTUnwrap(p.views[3]), work = try XCTUnwrap(p.views[4])
        XCTAssertEqual(home.uiColor("text_color"), UIColor.tintColor, "the tint itself")
        XCTAssertTrue(work.uiColor("text_color") === work.uiColor("text_color"), "a share of the tint, one object")
        let fields = try [2, 3, 4].map { try XCTUnwrap(p.views[UInt32($0)]?.field, "input \($0) is a native field") }
        let text = { (field: UITextField) in try self.components(XCTUnwrap(field.textColor).resolvedColor(with: field.traitCollection).cgColor) }
        assertChannels(try text(fields[1]), red, "the field's text in the tint")
        assertChannels(try text(fields[2]), [255, 59, 48, 51], "the field's text at a fifth of the tint")
        let placeholders = try [5, 6].map { try XCTUnwrap(p.views[UInt32($0)]?.field, "input \($0) is a native field") }
        let placeholder = { (field: UITextField) -> [Double] in
            let colour = try XCTUnwrap(field.attributedPlaceholder?.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? UIColor)
            return try self.components(colour.resolvedColor(with: field.traitCollection).cgColor)
        }
        dim(p, true)
        let grey = uikitGrey(red, uikitAlpha: true)
        assertChannels(try placeholder(placeholders[0]), red, "a written placeholder colour while dimmed")
        assertChannels(try placeholder(placeholders[1]), grey, "a placeholder in the tint while dimmed")
        assertChannels(try text(fields[0]), red, "the field's written text while dimmed")
        assertChannels(try text(fields[1]), grey, "the field's text in the tint while dimmed")
        assertChannels(try text(fields[2]), Array(grey.prefix(3)) + [51], "the field's text at a fifth of UIKit's grey while dimmed")
        dim(p, false)
        assertChannels(try text(fields[1]), red, "the field's text in the tint once the dimming ends")
    }

    /// Text in the tint in a dark subtree takes its light ancestor's accent in its own appearance,
    /// and greys from it while dimmed
    func testADarkSubtreeTakesItsAncestorsAccentInItsOwnAppearance() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "style": ["accent_color": [green, red]]],
            ["op": "create", "id": 2, "kind": "view", "style": ["color_scheme": "dark"]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Locked"], "style": ["text_color": tint]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 100.0, "h": 22.0],
            ["op": "roots", "ids": [1]],
        ])
        let status = try XCTUnwrap(p.views[3])
        XCTAssertTrue(status.drawsDark, "the subtree is dark")
        assertChannels(try runChannels(status), red, "the accent's dark side")
        dim(p, true)
        assertChannels(try runChannels(status), uikitGrey(red, uikitAlpha: true), "UIKit's grey of the accent's dark side")
        dim(p, false)
    }

    /// The page's canvas shows a root's fill in the tint as the root paints it: grey while dimmed,
    /// and in the tint again after
    func testTheCanvasGreysWithARootsFillInTheTint() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "style": ["background_color": tint, "written": ["background_color"]]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "roots", "ids": [1]],
        ])
        p.viewport.tintColor = color(red)
        let root = try XCTUnwrap(p.views[1])
        let canvas = { try self.components(XCTUnwrap(p.viewport.backgroundColor).resolvedColor(with: p.viewport.traitCollection).cgColor) }
        assertChannels(try canvas(), red, "the canvas in the tint")
        dim(p, true)
        let grey = uikitGrey(red, uikitAlpha: true)
        assertChannels(try fill(root), grey, "the root's fill while dimmed")
        assertChannels(try canvas(), grey, "the canvas while dimmed")
        dim(p, false)
        assertChannels(try canvas(), red, "the canvas once the dimming ends")
    }

    /// A symbol drawn in its configuration's colours, a palette in the tint, has no tint of its own:
    /// the tint UIKit resolves the palette by is the node's, which it dims
    func testAPaletteInTheTintTakesTheNodesTint() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "image", "props": ["imageSource": "symbol:sf/cloud.sun.fill", "symbolName": "cloud.sun.fill"],
             "style": ["font_size": 24.0, "symbol_rendering": "palette", "symbol_palette": [tint]]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0],
            ["op": "roots", "ids": [1]],
        ])
        p.viewport.tintColor = color(green)
        let symbol = try symbolView(p.views[1])
        assertChannels(try components(symbol.tintColor.cgColor), green, "the symbol's view in the node's tint, not the label's")
        dim(p, true)
        XCTAssertEqual(symbol.tintAdjustmentMode, .dimmed, "UIKit dims it")
        dim(p, false)
    }

    /// A route's backdrop in the tint is UIKit's tint where the controller's view draws it, grey
    /// while dimmed; under the route's own `accent-color`, which UIKit does not carry to that view,
    /// that accent
    func testARoutesBackdropInTheTintIsTheTintItDrawsIn() throws {
        let p = presenter([
            ["op": "create", "id": 9, "kind": "view"],
            ["op": "create", "id": 1, "kind": "view", "props": ["navigationKey": "home"], "style": ["background_color": tint, "written": ["background_color"]]],
            ["op": "create", "id": 2, "kind": "view", "props": ["navigationKey": "away"],
             "style": ["accent_color": green, "background_color": tint, "written": ["accent_color", "background_color"]]],
            ["op": "children", "id": 9, "ids": [1, 2]],
            ["op": "frame", "id": 9, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "roots", "ids": [9]],
        ])
        p.viewport.tintColor = color(red)
        let home = RouteController(try XCTUnwrap(p.views[1])), away = RouteController(try XCTUnwrap(p.views[2]))
        for route in [home, away] { route.loadViewIfNeeded(); p.viewport.addSubview(route.view) }
        let backdrop = { (route: RouteController) in
            try self.components(XCTUnwrap(route.view.backgroundColor).resolvedColor(with: route.view.traitCollection).cgColor)
        }
        assertChannels(try backdrop(home), red, "the tint it draws in")
        assertChannels(try backdrop(away), green, "the route's own accent")
        dim(p, true)
        assertChannels(try backdrop(home), uikitGrey(red, uikitAlpha: true), "UIKit's grey while dimmed")
        dim(p, false)
    }

    /// The tint as a value for what UIKit keeps (`carriedTint`) is the node's own tint, undimmed while
    /// UIKit dims it, and out of a window the roots' accent, which the window will take
    func testTheCarriedTintIsTheNodesUndimmed() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "style": ["accent_color": green]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Parked"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "roots", "ids": [1]],
        ])
        let node = try XCTUnwrap(p.views[2])
        assertChannels(try components(node.carriedTint.cgColor), green, "its tint")
        dim(p, true)
        assertChannels(try components(node.carriedTint.cgColor), green, "its tint undimmed while dimmed")
        dim(p, false)
        p.viewport.removeFromSuperview()
        p.apply(wireBatch([["op": "create", "id": 3, "kind": "text", "props": ["text": "Away"]], ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0],
                           ["op": "roots", "ids": [1, 3]]]))
        assertChannels(try components(XCTUnwrap(p.views[3]).carriedTint.cgColor), green, "out of a window, the roots' accent")
    }

    /// A native button's accent, written or the tint's, stays in its stored tint while dimmed, so
    /// the button shows it again when the dimming ends: UIKit greys the tint itself
    func testANativeButtonKeepsItsAccentThroughADimmedRefresh() throws {
        for accent in [red as Any, ["sys": "@tint", "c": blue] as Any] {
            var face = ButtonFace()
            face.title = "Go"; face.style = "bordered"; face.ios = "bordered"; face.iosBefore26 = "bordered"
            let p = presenter([
                ["op": "create", "id": 1, "kind": "control", "props": ["type": "button", "accessibilityRole": "button"], "handlers": ["press"],
                 "style": ["appearance": "auto", "text_color": [0, 0, 0, 255], "accent_color": accent]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 120.0, "h": 34.0],
                ["op": "roots", "ids": [1]],
            ], faces: [1: face])
            p.viewport.tintColor = color(red)
            p.controls.sync()
            let button = try XCTUnwrap(p.controls.controls[1] as? NativeButtonIOS)
            dim(p, true)
            p.apply(wireBatch([["op": "props", "id": 1, "set": ["accessibilityLabel": "Go now"]]]))
            p.controls.sync()
            assertChannels(try components(try XCTUnwrap(button.written?.accent).cgColor), red, "the accent (\(accent)) the button keeps while dimmed")
            dim(p, false)
            assertChannels(try components(button.tintColor.cgColor), red, "the accent (\(accent)) the button shows once the dimming ends")
        }
    }


    /// A neutral colour stays exactly as written on a dimmed face: an HDR white fill keeps its range
    func testANeutralHDRFillKeepsItsRangeWhileDimmed() throws {
        let fourTimesWhite: [String: Any] = ["cs": [["s": "srgb-linear", "v": [4.0, 4.0, 4.0, 1.0]]], "c": [255, 255, 255, 255]]
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"], "style": ["background_color": fourTimesWhite, "written": ["background_color"]]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "roots", "ids": [1]],
        ])
        let button = try XCTUnwrap(p.views[1])
        button.layer.displayIfNeeded()
        XCTAssertTrue(ColorRange.isHDR(button.layer.backgroundColor), "an HDR fill before the dimming")
        dim(p, true)
        button.layer.displayIfNeeded()
        let fill = try XCTUnwrap(button.layer.backgroundColor)
        XCTAssertTrue(ColorRange.isHDR(fill), "the HDR fill while dimmed: \(fill)")
        XCTAssertEqual(fill.components, [4, 4, 4, 1], "in its own space, as written")
        dim(p, false)
    }


    /// A filtered box's picture draws again when a link in the tint beneath it greys, outside any
    /// control, and when it takes its colour back: here a paragraph that draws itself, with no raster
    /// whose landing would draw the picture again
    func testAFilteredBoxAboveALinkDrawsAgainWithTheTint() throws {
        let sub: [Double] = [-10, -10, 20, 20]
        let program = sub + [1] + [0, -1, -3] + sub + [0, 2, 2]
        guard SvgFilterGPU.runs(program.map(Float.init)) else { throw missing("metal", "no GPU filter path") }
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "style": ["filter": ["p": program]]],
            ["op": "create", "id": 2, "kind": "text", "style": ["background_clip": "text"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 22.0],
            ["op": "roots", "ids": [1]],
        ])
        var tinted = InlineStyle()
        tinted.colorRef = .object(["sys": .string("@tint"), "c": [0, 122, 255, 255]])
        p.applyParagraph(2, [InlineText(id: 3, parent: 2, props: ["text": "terms", "href": "https://example.com/terms"], style: tinted, handlers: [], paints: true)])
        let paragraph = try XCTUnwrap(p.views[2])
        XCTAssertFalse(paragraph.canRasterText, "the paragraph draws itself")
        XCTAssertNotNil(p.views[1]?.boxFilter?.picture, "the box is pictured")
        until("the first picture lands") { !p.boxFilters.drawing }
        dim(p, true)
        XCTAssertTrue(p.boxFilters.drawing, "the picture draws again as the link greys")
        until("that picture lands") { !p.boxFilters.drawing }
        dim(p, false)
        XCTAssertTrue(p.boxFilters.drawing, "and again as the link takes its colour back")
        until("the last picture lands") { !p.boxFilters.drawing }
    }


    /// A pressable route's backdrop, which its controller's view keeps, takes its fill as written
    /// while dimmed, so the view shows it once the dimming ends
    func testAPressableRoutesBackdropIsAsWrittenWhileDimmed() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"], "props": ["navigationKey": "home"],
             "style": ["background_color": blue, "written": ["background_color"]]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "roots", "ids": [1]],
        ])
        let node = try XCTUnwrap(p.views[1]), route = RouteController(node)
        node.tintAdjustmentMode = .dimmed
        route.loadViewIfNeeded()
        node.tintAdjustmentMode = .automatic
        assertChannels(try components(XCTUnwrap(route.view.layer.backgroundColor)), blue, "the backdrop once the dimming ends")
    }


    /// A run in the tint beneath a link it stands in, and one a press acts on, grey, while the
    /// paragraph's plain text in a colour of its own keeps it
    func testANestedLinkAndAPressedRunDimAsLinks() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "text"],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 22.0],
            ["op": "roots", "ids": [1]],
        ])
        var tinted = InlineStyle(), plain = InlineStyle()
        tinted.colorRef = .object(["sys": .string("@tint"), "c": [0, 122, 255, 255]])
        plain.run.color = red.map(Double.init)
        p.applyParagraph(1, [
            InlineText(id: 2, parent: 1, props: ["href": "https://example.com/terms"], style: InlineStyle(), handlers: [], paints: false),
            InlineText(id: 3, parent: 2, props: ["text": "terms"], style: tinted, handlers: [], paints: true),
            InlineText(id: 4, parent: 1, props: ["text": " and "], style: plain, handlers: [], paints: true),
            InlineText(id: 5, parent: 1, props: ["text": "more"], style: tinted, handlers: ["press"], paints: true),
        ])
        let paragraph = try XCTUnwrap(p.views[1])
        p.viewport.tintColor = color(blue)
        dim(p, true)
        assertChannels(try runChannels(paragraph, 0), uikitGrey(blue, uikitAlpha: true), "the run beneath a link")
        assertChannels(try runChannels(paragraph, 1), red, "the plain text")
        assertChannels(try runChannels(paragraph, 2), uikitGrey(blue, uikitAlpha: true), "the run a press acts on")
        dim(p, false)
        assertChannels(try runChannels(paragraph, 0), blue, "the run beneath a link once the dimming ends")
    }


    /// The page's canvas, which the viewport keeps, takes a pressable root's fill as written: a
    /// batch while dimmed leaves it coloured
    func testThePageCanvasKeepsAPressableRootsFillWhileDimmed() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"], "style": ["background_color": blue, "written": ["background_color"]]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "roots", "ids": [1]],
        ])
        let canvas = { try self.components(XCTUnwrap(p.viewport.backgroundColor).resolvedColor(with: p.viewport.traitCollection).cgColor) }
        assertChannels(try canvas(), blue, "the canvas before the dimming")
        dim(p, true)
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["accessibilityLabel": "Page"]]]))
        assertChannels(try canvas(), blue, "the canvas after a batch while dimmed")
        dim(p, false)
        assertChannels(try canvas(), blue, "the canvas once the dimming ends")
    }


    private func presenter(_ ops: [[String: Any]], faces: [UInt32: ButtonFace] = [:]) -> Presenter {
        let p = Presenter()
        p.buttonFace = { faces[$0] ?? ButtonFace() }
        mount(p)
        p.apply(wireBatch(ops))
        return p
    }

    /// Mounts `p`'s viewport in a new light window of 400 points
    private func mount(_ p: Presenter) {
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.overrideUserInterfaceStyle = .light
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
    }

    /// A path icon in its colour (IconPathTests)
    private func icon(_ id: Int, _ style: [String: Any]) -> [[String: Any]] {
        var props: [String: Any] = ["imageSource": "symbol:icon||0 0 24 24|M2 2 L22 2 L22 22 L2 22 Z|icons/x.svg", "symbolName": "",
                                    "symbolPath": "M 2 2 L 22 2 L 22 22 L 2 22 Z", "symbolViewBox": "0 0 24 24"]
        if style["text_color"] != nil { props["symbolColorWritten"] = "1" }
        return [["op": "create", "id": id, "kind": "image", "props": props, "handlers": [], "style": ["font_size": 24.0].merging(style) { $1 }],
                ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0]]
    }

    /// Runs the main run loop until `done`, for at most `seconds`: pictures land on it (docs/testing.md)
    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    /// UIKit dims tints behind a sheet, or stops, and the window lays out what changed
    private func dim(_ p: Presenter, _ on: Bool) {
        p.viewport.tintAdjustmentMode = on ? .dimmed : .automatic
        window.layoutIfNeeded()
    }

    /// The grey UIKit gives a tint of `c` while it dims tints, at `c`'s own alpha or at UIKit's
    private func uikitGrey(_ c: [Int], uikitAlpha: Bool = false) -> [Int] {
        let probe = UIView()
        probe.tintColor = color(c)
        probe.tintAdjustmentMode = .dimmed
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        probe.tintColor.getRed(&r, green: &g, blue: &b, alpha: &a)
        return [r, g, b].map { Int(($0 * 255).rounded()) } + [uikitAlpha ? Int((a * 255).rounded()) : c[3]]
    }

    private func color(_ c: [Int]) -> UIColor {
        UIColor(red: CGFloat(c[0]) / 255, green: CGFloat(c[1]) / 255, blue: CGFloat(c[2]) / 255, alpha: CGFloat(c[3]) / 255)
    }

    /// The fill a node's layer shows once Core Animation displays it (`NodeLayer.display`)
    private func fill(_ node: NodeView) throws -> [Double] {
        node.layer.displayIfNeeded()
        return try components(XCTUnwrap(node.layer.backgroundColor, "the layer has a fill"))
    }

    /// The colour a node's outer shadow is cast in
    private func shadow(_ node: NodeView) throws -> [Double] {
        node.layer.displayIfNeeded()
        let caster = try XCTUnwrap(node.shadowCaster, "the node casts a shadow")
        let cast = ([caster] + (caster.sublayers ?? [])).compactMap { $0.shadowOpacity > 0 ? $0.shadowColor : nil }
        return try components(XCTUnwrap(cast.first, "a layer casts it"))
    }

    private func symbolView(_ node: NodeView?) throws -> UIImageView {
        try XCTUnwrap(node?.symbolView, "the node draws a symbol")
    }

    /// The channels paragraph `node` paints run `index` in, as the run carries them
    private func runChannels(_ node: NodeView, _ index: Int = 0) throws -> [Double] {
        let runs = node.paragraphSpec().runs
        return try XCTUnwrap(index < runs.count ? runs[index].color : nil, "run \(index) of \(runs.count) has a colour")
    }

    private func components(_ c: CGColor) throws -> [Double] {
        let rgb = try XCTUnwrap(c.converted(to: CGColorSpace(name: CGColorSpace.sRGB)!, intent: .defaultIntent, options: nil)?.components)
        return rgb.map { Double($0) * 255 }
    }

    /// The first four channels (sRGB, 0...255) are `expected`'s, alpha included
    private func assertChannels(_ c: [Double], _ expected: [Int], _ what: String, file: StaticString = #filePath, line: UInt = #line) {
        guard c.count >= 4 else { return XCTFail("\(what): \(c) has no four channels", file: file, line: line) }
        for i in 0..<4 { XCTAssertEqual(c[i], Double(expected[i]), accuracy: 2, "\(what): \(Array(c.prefix(4))), not \(expected)", file: file, line: line) }
    }
}
#endif
