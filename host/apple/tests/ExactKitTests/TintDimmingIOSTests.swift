#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A control's face greys as one while UIKit dims tints behind a sheet or an alert, and comes
/// back when the dimming ends; a status outside a control keeps its colour, and a link in the
/// tint dims as UIKit's do (TintDimmingIOS.swift). Each grey is UIKit's own for that colour, read
/// from a view it dims. UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class TintDimmingIOSTests: XCTestCase {
    private var window: UIWindow!
    private let red = [255, 59, 48, 255], green = [52, 199, 89, 255], blue = [0, 122, 255, 255]

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
            dim(p, false)
            assertChannels(try components(button.tintColor.cgColor), red, "the accent (\(accent)) the button shows once the dimming ends")
        }
    }

    /// A custom button's fill, its red label, a label in display-p3 and its coloured shadow take
    /// UIKit's grey for each, and their own colours back when the dimming ends
    func testACustomControlsFaceGreysAsOne() throws {
        let p3: [String: Any] = ["c": [255, 0, 0, 255], "cs": [["s": "display-p3", "v": [1.0, 0.0, 0.0, 1.0]]]]
        let glow: [[String: Any]] = [["o": [0.0, 2.0], "b": 6.0, "c": green]]
        let p = presenter([
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press"],
             "style": ["background_color": blue, "box_shadow": glow, "written": ["background_color", "box_shadow"]]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Delete"], "style": ["text_color": red]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Wide"], "style": ["text_color": p3]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "frame", "id": 1, "x": 10.0, "y": 10.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 22.0],
            ["op": "frame", "id": 3, "x": 100.0, "y": 0.0, "w": 100.0, "h": 22.0],
            ["op": "roots", "ids": [1]],
        ])
        let button = try XCTUnwrap(p.views[1]), label = try XCTUnwrap(p.views[2]), wide = try XCTUnwrap(p.views[3])
        assertChannels(try fill(button), blue, "the fill before the dimming")
        assertChannels(try shadow(button), green, "the shadow before the dimming")
        dim(p, true)
        assertChannels(try fill(button), uikitGrey(blue), "the button's fill")
        assertChannels(try shadow(button), uikitGrey(green), "the button's shadow")
        assertChannels(try runChannels(label), uikitGrey(red), "the button's red label")
        let dimmedWide = try runChannels(wide)
        assertChannels(try components(TextEngine.color(dimmedWide).cgColor), uikitGrey([255, 0, 0, 255]), "a label in display-p3, drawn from its sRGB channels: \(dimmedWide)")
        dim(p, false)
        assertChannels(try fill(button), blue, "the fill once the dimming ends")
        assertChannels(try shadow(button), green, "the shadow once the dimming ends")
        assertChannels(try runChannels(label), red, "the label once the dimming ends")
        let restored = try runChannels(wide)
        XCTAssertEqual(restored.count, 9, "the display-p3 label is in its own space again: \(restored)")
        XCTAssertEqual(Array(restored.dropFirst(4)), [1, 1, 0, 0, 1], "display-p3 (`textChannels`' space code 1) and its channels")
    }

    /// A custom slider, which a pointer and keys drive and nothing presses, is a control: its face
    /// greys
    func testACustomSliderGreysAsAControl() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["pointerdown", "pointermove", "pointerup", "key"],
             "props": ["accessibilityRole": "slider"], "style": ["background_color": blue, "written": ["background_color"]]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "roots", "ids": [1]],
        ])
        let slider = try XCTUnwrap(p.views[1])
        dim(p, true)
        assertChannels(try fill(slider), uikitGrey(blue), "the slider's fill")
        dim(p, false)
        assertChannels(try fill(slider), blue, "the slider's fill once the dimming ends")
    }

    /// The colour a node hands UIKit as its own (`ownUIColor`: a tab bar's tint, an alert's, a
    /// title's) is as written while dimmed: UIKit dims it itself, and keeps it once the dimming ends
    func testTheColourHandedToUIKitIsAsWrittenWhileDimmed() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Today"], "style": ["text_color": red, "written": ["text_color"]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 22.0],
            ["op": "roots", "ids": [1]],
        ])
        let label = try XCTUnwrap(p.views[2])
        dim(p, true)
        assertChannels(try runChannels(label), uikitGrey(red), "the label Exact paints greys")
        assertChannels(try components(XCTUnwrap(label.ownUIColor("text_color")).cgColor), red, "the colour handed to UIKit")
        dim(p, false)
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

    /// Boxes drawn flat in a control's layer grey with it, one alone and a run of alike ones that
    /// one shape layer draws, and take their colour back
    func testFlatBoxesInAControlGreyWithIt() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"]],
            ["op": "create", "id": 2, "kind": "view", "style": bar(green)],
            ["op": "create", "id": 3, "kind": "view", "style": bar(blue)],
            ["op": "create", "id": 4, "kind": "view", "style": bar(blue)],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 20.0, "h": 20.0],
            ["op": "frame", "id": 3, "x": 40.0, "y": 0.0, "w": 20.0, "h": 20.0],
            ["op": "frame", "id": 4, "x": 60.0, "y": 0.0, "w": 20.0, "h": 20.0],
            ["op": "roots", "ids": [1]],
        ])
        let row = try XCTUnwrap(p.views[1]), leaf = try XCTUnwrap(p.flats.leaves[2], "the box is drawn flat")
        XCTAssertNotNil(p.flats.leaves[3], "and so are the two alike")
        assertChannels(try runFill(row), blue, "the run before the dimming")
        dim(p, true)
        assertChannels(try components(XCTUnwrap(leaf.layer.backgroundColor)), uikitGrey(green), "the flat box")
        assertChannels(try runFill(row), uikitGrey(blue), "the run of flat boxes")
        dim(p, false)
        assertChannels(try components(XCTUnwrap(leaf.layer.backgroundColor)), green, "the flat box once the dimming ends")
        assertChannels(try runFill(row), blue, "the run once the dimming ends")
    }

    /// A colour a face writes and the same colour from the tint grey alike, at their own alpha
    func testALiteralAndATintColourOnOneFaceGreyAlike() throws {
        let faint = [0, 122, 255, 51]
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"]],
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Written"], "style": ["text_color": faint]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Tinted"], "style": ["text_color": ["sys": "@tint/0.2", "c": faint]]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 22.0],
            ["op": "frame", "id": 3, "x": 100.0, "y": 0.0, "w": 100.0, "h": 22.0],
            ["op": "roots", "ids": [1]],
        ])
        p.viewport.tintColor = color(blue)
        let written = try XCTUnwrap(p.views[2]), tinted = try XCTUnwrap(p.views[3])
        assertChannels(try runChannels(tinted), faint, "the tint at a fifth before the dimming")
        dim(p, true)
        assertChannels(try runChannels(written), uikitGrey(faint), "the written colour")
        assertChannels(try runChannels(tinted), uikitGrey(faint), "the same colour from the tint")
        dim(p, false)
        assertChannels(try runChannels(tinted), faint, "the tint at a fifth once the dimming ends")
    }

    /// Boxes drawn flat in a control that is already dimmed paint greyed, one alone and a run, and
    /// take their colour when the dimming ends
    func testFlatBoxesAddedToADimmedControlPaintGreyed() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "roots", "ids": [1]],
        ])
        dim(p, true)
        p.apply(wireBatch([
            ["op": "create", "id": 2, "kind": "view", "style": bar(green)],
            ["op": "create", "id": 3, "kind": "view", "style": bar(blue)],
            ["op": "create", "id": 4, "kind": "view", "style": bar(blue)],
            // Placed where their frames already put them, so placing them moves nothing
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 20.0, "h": 20.0],
            ["op": "frame", "id": 3, "x": 40.0, "y": 0.0, "w": 20.0, "h": 20.0],
            ["op": "frame", "id": 4, "x": 60.0, "y": 0.0, "w": 20.0, "h": 20.0],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
        ]))
        let row = try XCTUnwrap(p.views[1]), leaf = try XCTUnwrap(p.flats.leaves[2], "the box is drawn flat")
        assertChannels(try components(XCTUnwrap(leaf.layer.backgroundColor)), uikitGrey(green), "a flat box added while dimmed")
        assertChannels(try runFill(row), uikitGrey(blue), "a run of flat boxes added while dimmed")
        // Moved, still dimmed, to a box that is no control, where it stands just as it did
        p.apply(wireBatch([
            ["op": "create", "id": 5, "kind": "view"],
            ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "children", "id": 1, "ids": [3, 4]],
            ["op": "children", "id": 5, "ids": [2]],
            ["op": "roots", "ids": [1, 5]],
        ]))
        XCTAssertTrue(p.flats.leaves[2] === leaf, "the box is the same flat leaf")
        assertChannels(try components(XCTUnwrap(leaf.layer.backgroundColor)), green, "a flat box moved out of the dimmed control")
        dim(p, false)
        assertChannels(try components(XCTUnwrap(leaf.layer.backgroundColor)), green, "the flat box once the dimming ends")
        assertChannels(try runFill(row), blue, "the run once the dimming ends")
    }

    /// Text and a symbol in the tint outside a control keep the tint as it rests: shown first while
    /// dimmed, after their accent changes while dimmed, and with no accent, their embedder's tint
    func testAStatusOutsideAControlKeepsItsTintWhileDimmed() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "style": ["accent_color": green]],
            ["op": "create", "id": 5, "kind": "text", "props": ["text": "Parked"], "style": ["text_color": ["sys": "@tint", "c": blue]]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 50.0, "w": 100.0, "h": 22.0],
            ["op": "roots", "ids": [1, 5]],
        ])
        p.viewport.tintColor = color(red)
        let parked = try XCTUnwrap(p.views[5])
        assertChannels(try runChannels(parked), red, "text in the embedder's tint before the dimming")
        dim(p, true)
        p.apply(wireBatch([
            ["op": "create", "id": 2, "kind": "text", "props": ["text": "Locked"], "style": ["text_color": ["sys": "@tint", "c": blue]]],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 22.0],
        ] + icon(3, ["text_color": ["sys": "@tint", "c": blue]]) + [["op": "children", "id": 1, "ids": [2, 3]]]))
        let status = try XCTUnwrap(p.views[2]), symbol = try symbolView(p.views[3])
        assertChannels(try runChannels(status), green, "a status first shown while dimmed")
        assertChannels(try components(symbol.tintColor.cgColor), green, "a symbol first shown while dimmed")
        XCTAssertEqual(symbol.tintAdjustmentMode, .normal, "UIKit leaves the symbol's ink as its node says")
        assertChannels(try runChannels(parked), red, "text with no accent keeps its embedder's tint")
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["accent_color": blue]]]))
        assertChannels(try runChannels(status), blue, "the status after its accent changed while dimmed")
        assertChannels(try components(symbol.tintColor.cgColor), blue, "the symbol after its accent changed while dimmed")
        dim(p, false)
    }

    /// Text in the tint in a dark subtree takes its light ancestor's accent in its own appearance
    /// while dimmed, as the accent's dynamic tint resolves there before
    func testADarkSubtreeKeepsItsAncestorsAccentInItsOwnAppearance() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "view", "style": ["accent_color": [green, red]]],
            ["op": "create", "id": 2, "kind": "view", "style": ["color_scheme": "dark"]],
            ["op": "create", "id": 3, "kind": "text", "props": ["text": "Locked"], "style": ["text_color": ["sys": "@tint", "c": blue]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 100.0, "h": 22.0],
            ["op": "roots", "ids": [1]],
        ])
        let status = try XCTUnwrap(p.views[3])
        XCTAssertTrue(status.drawsDark, "the subtree is dark")
        assertChannels(try runChannels(status), red, "the accent's dark side before the dimming")
        dim(p, true)
        assertChannels(try runChannels(status), red, "the accent's dark side while dimmed")
        dim(p, false)
    }

    /// A symbol in a custom control greys with it, and its colour comes back
    func testASymbolInAControlGreysWithIt() throws {
        let p = presenter([["op": "create", "id": 1, "kind": "view", "handlers": ["press"]]]
                          + icon(2, ["text_color": red]) + [
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "roots", "ids": [1]],
        ])
        let symbol = try symbolView(p.views[2])
        assertChannels(try components(symbol.tintColor.cgColor), red, "the symbol before the dimming")
        dim(p, true)
        assertChannels(try components(symbol.tintColor.cgColor), uikitGrey(red), "the symbol in a dimmed control")
        dim(p, false)
        assertChannels(try components(symbol.tintColor.cgColor), red, "the symbol once the dimming ends")
    }

    /// A link in the tint in a paragraph outside a control greys, as a `UITextView`'s link does,
    /// while the paragraph's other text keeps its colour; a link in a fixed colour keeps it
    func testALinkInTheTintDims() throws {
        let p = presenter([
            ["op": "create", "id": 1, "kind": "text"],
            ["op": "create", "id": 4, "kind": "text"],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 22.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 30.0, "w": 300.0, "h": 22.0],
            ["op": "roots", "ids": [1, 4]],
        ])
        var tinted = InlineStyle(), plain = InlineStyle(), fixed = InlineStyle()
        tinted.colorRef = .object(["sys": .string("@tint"), "c": [0, 122, 255, 255]])
        plain.run.color = red.map(Double.init)
        fixed.run.color = green.map(Double.init)
        p.applyParagraph(1, [
            InlineText(id: 2, parent: 1, props: ["text": "Read the "], style: plain, handlers: [], paints: true),
            InlineText(id: 3, parent: 1, props: ["text": "terms", "href": "https://example.com/terms"], style: tinted, handlers: [], paints: true),
        ])
        p.applyParagraph(4, [InlineText(id: 5, parent: 4, props: ["text": "privacy", "href": "https://example.com/privacy"], style: fixed, handlers: [], paints: true)])
        let paragraph = try XCTUnwrap(p.views[1]), other = try XCTUnwrap(p.views[4])
        p.viewport.tintColor = color(blue)
        assertChannels(try runChannels(paragraph, 1), blue, "the link before the dimming")
        assertChannels(try runChannels(other), green, "the fixed-colour link before the dimming")
        dim(p, true)
        assertChannels(try runChannels(paragraph, 0), red, "the paragraph's own text")
        assertChannels(try runChannels(paragraph, 1), uikitGrey(blue, uikitAlpha: true), "the link, as UIKit dims it")
        assertChannels(try runChannels(other), green, "the fixed-colour link")
        dim(p, false)
        assertChannels(try runChannels(paragraph, 1), blue, "the link once the dimming ends")
    }

    /// A link in a neutral tint greys as UIKit dims that tint, its alpha included, as a text view's
    /// link does
    func testALinkInANeutralTintDimsAsUIKitDimsIt() throws {
        let grey = [142, 142, 147, 255]
        let p = presenter([
            ["op": "create", "id": 1, "kind": "text"],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 22.0],
            ["op": "roots", "ids": [1]],
        ])
        var tinted = InlineStyle()
        tinted.colorRef = .object(["sys": .string("@tint"), "c": [142, 142, 147, 255]])
        p.applyParagraph(1, [InlineText(id: 2, parent: 1, props: ["text": "terms", "href": "https://example.com/terms"], style: tinted, handlers: [], paints: true)])
        let paragraph = try XCTUnwrap(p.views[1])
        p.viewport.tintColor = color(grey)
        dim(p, true)
        assertChannels(try runChannels(paragraph), uikitGrey(grey, uikitAlpha: true), "the link, as UIKit dims a grey tint")
        dim(p, false)
        assertChannels(try runChannels(paragraph), grey, "the link once the dimming ends")
    }

    /// A run in the tint beneath a link it stands in, and one a press acts on, grey as links do,
    /// while the paragraph's plain text keeps its colour
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

    /// A native field in a pressable row is UIKit's to dim: its written text colour stays while the
    /// row's own fill greys
    func testANativeFieldInAControlKeepsItsTextColour() throws {
        // A session's presenter: a field takes its font and colour from its text engine
        let session = ExactApp.shared.makeSession(label: "dim-field-\(UUID().uuidString)")
        defer { session.destroy() }
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.overrideUserInterfaceStyle = .light
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"], "style": ["background_color": blue, "written": ["background_color"]]],
            ["op": "create", "id": 2, "kind": "input", "handlers": ["change"], "props": ["value": "Garage"], "style": ["text_color": red, "written": ["text_color"]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 200.0, "h": 44.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 200.0, "h": 34.0],
            ["op": "roots", "ids": [1]],
        ]))
        let row = try XCTUnwrap(p.views[1]), node = try XCTUnwrap(p.views[2])
        let field = try XCTUnwrap(node.field, "the input is a native field")
        let text = { try self.components(XCTUnwrap(field.textColor).resolvedColor(with: field.traitCollection).cgColor) }
        assertChannels(try text(), red, "the field's text before the dimming")
        dim(p, true)
        assertChannels(try fill(row), uikitGrey(blue), "the row's fill")
        assertChannels(try text(), red, "the field's text while dimmed")
        dim(p, false)
    }

    private func presenter(_ ops: [[String: Any]], faces: [UInt32: ButtonFace] = [:]) -> Presenter {
        let p = Presenter()
        p.buttonFace = { faces[$0] ?? ButtonFace() }
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.overrideUserInterfaceStyle = .light
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch(ops))
        return p
    }

    /// A box the host sends flat: only its fill and size, which its author wrote
    private func bar(_ fill: [Int]) -> [String: Any] {
        ["width": 20.0, "height": 20.0, "background_color": fill, "text_color": [0, 0, 0, 255], "written": ["background_color"]]
    }

    /// A path icon in its colour (IconPathTests)
    private func icon(_ id: Int, _ style: [String: Any]) -> [[String: Any]] {
        var props: [String: Any] = ["imageSource": "symbol:icon||0 0 24 24|M2 2 L22 2 L22 22 L2 22 Z|icons/x.svg", "symbolName": "",
                                    "symbolPath": "M 2 2 L 22 2 L 22 22 L 2 22 Z", "symbolViewBox": "0 0 24 24"]
        if style["text_color"] != nil { props["symbolColorWritten"] = "1" }
        return [["op": "create", "id": id, "kind": "image", "props": props, "handlers": [], "style": ["font_size": 24.0].merging(style) { $1 }],
                ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0]]
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

    /// The fill of the shape layer that draws a run of alike flat boxes in `node`
    private func runFill(_ node: NodeView) throws -> [Double] {
        func shapes(_ layer: CALayer) -> [CAShapeLayer] { ((layer as? CAShapeLayer).map { [$0] } ?? []) + (layer.sublayers ?? []).flatMap(shapes) }
        let run = try XCTUnwrap(shapes(node.layer).first { $0.fillColor != nil }, "a shape layer draws the run")
        return try components(XCTUnwrap(run.fillColor))
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
