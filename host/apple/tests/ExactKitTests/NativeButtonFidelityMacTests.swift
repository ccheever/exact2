// @ref LLP 1069.011.001 §4, D1–D17: real AppKit geometry and independent pixel references.
#if os(macOS)
import AppKit
import CExact
import CoreText
import XCTest
@testable import ExactKit

final class NativeButtonFidelityMacTests: XCTestCase {
    private var window: NSWindow!
    private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../..").standardizedFileURL
    override func tearDown() { window?.close(); window = nil; super.tearDown() }
    private func makeWindow() -> NSWindow {
        _ = NSApplication.shared
        let win = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 850), styleMask: [.titled], backing: .buffered, defer: false)
        win.isReleasedWhenClosed = false
        win.appearance = NSAppearance(named: .aqua)
        window = win
        return win
    }
    private func compile(_ source: URL) throws -> Data {
        let dir = root.appendingPathComponent("target/1104-b3/tests/" + UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let plan = dir.appendingPathComponent("app.plan")
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", source.path, "-o", plan.path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: plan)
    }
    private func fixture() throws -> ExactSession {
        let win = makeWindow()
        let session = ExactApp.shared.makeSession(label: "button-fidelity-mac")
        win.contentView = session.presenter.viewport
        XCTAssertNil(session.boot(plan: try compile(root.appendingPathComponent("scripts/fixtures/native-buttons.contract")), size: win.contentLayoutRect.size).error)
        return session
    }
    private func button(_ session: ExactSession, _ name: String) throws -> NativeButtonMac {
        let node = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == name }, name)
        return try XCTUnwrap(session.presenter.controls.controls[node.id] as? NativeButtonMac, name)
    }
    private func data(_ rows: [String: Any] = [:], title: String = "Title", style: String = "bordered") throws -> Data {
        try JSONSerialization.data(withJSONObject: ["title": title, "symbol": "lock.fill", "style": style, "macos": "push", "rows": rows], options: [.sortedKeys])
    }
    private func configured(_ face: ButtonFace) -> NSButton {
        let b = NSButton(title: "", target: nil, action: nil)
        ButtonConfigurationMac.apply(face, to: b, appearance: NSAppearance(named: .aqua)!, accent: nil)
        return b
    }
    func testAbsentAndClearedRowsUsePlatformFontAndDefaults() {
        var f = ButtonFace(); f.title = "Title"; f.symbol = "lock.fill"
        let b = configured(f), reference = NSButton(title: "Title", target: nil, action: nil)
        reference.bezelStyle = .push
        XCTAssertEqual(b.controlSize, reference.controlSize)
        XCTAssertEqual(b.font, NSFont.systemFont(ofSize: NSFont.systemFontSize(for: reference.controlSize)))
        XCTAssertEqual(b.bezelColor, reference.bezelColor); XCTAssertEqual(b.contentTintColor, reference.contentTintColor)
        XCTAssertEqual(b.alignment, .center)
        XCTAssertNotNil(b.symbolConfiguration)
        f.rows.title = ["font_size": .number(23), "font_weight": .number(700), "text_color": .array([.number(255), .number(0), .number(0), .number(255)])]
        f.rows.button["control_size"] = .string("large")
        f.rows.symbol["tint_color"] = .array([.number(0), .number(255), .number(0), .number(255)])
        ButtonConfigurationMac.apply(f, to: b, appearance: NSAppearance(named: .aqua)!, accent: nil)
        XCTAssertEqual(b.controlSize, .large)
        XCTAssertEqual(b.font?.pointSize, 23); XCTAssertEqual(TextEngine.controlWeight(b.font!), 700)
        f.rows = ButtonFaceRows()
        ButtonConfigurationMac.apply(f, to: b, appearance: NSAppearance(named: .aqua)!, accent: nil)
        XCTAssertEqual(b.controlSize, reference.controlSize, "clearing restores the pristine platform size")
        XCTAssertEqual(b.font, NSFont.systemFont(ofSize: NSFont.systemFontSize(for: reference.controlSize)))
        XCTAssertEqual(b.attributedTitle.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor,
            reference.attributedTitle.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor)
        XCTAssertNil(b.contentTintColor); XCTAssertTrue(b.image?.isTemplate == true)
    }
    func testMinContentUsesTheLongestRunEvenWithAppKitsSingleLineStandIn() {
        let cache = ButtonMeasureCache(), appearance = NSAppearance(named: .aqua)!
        var f = ButtonFace(); f.title = "A long title that wraps"; f.macos = "push"
        f.symbol = "lock.fill"
        let minimum = cache.measure(f, widthKind: 1, width: 0, appearance: appearance, scale: 2)
        let maximum = cache.measure(f, widthKind: 2, width: 0, appearance: appearance, scale: 2)
        var run = f; run.title = "wraps"
        let expected = configured(run).fittingSize
        XCTAssertEqual(CGFloat(minimum.width), expected.width, accuracy: 0.5)
        XCTAssertLessThan(minimum.width, maximum.width)
        f.rows.title["white_space"] = .string("nowrap")
        XCTAssertEqual(cache.measure(f, widthKind: 1, width: 0, appearance: appearance, scale: 2).width, maximum.width)
    }
    func testAbsentControlSizeDoesNotOverwriteThePlatformControl() {
        var f = ButtonFace(); f.title = "Title"
        let b = NSButton(title: "Title", target: nil, action: nil); b.controlSize = .small
        ButtonConfigurationMac.apply(f, to: b, appearance: NSAppearance(named: .aqua)!, accent: nil)
        XCTAssertEqual(b.controlSize, .small, "absent means leave the platform field alone")
        f.rows.button["control_size"] = .string("medium")
        ButtonConfigurationMac.apply(f, to: b, appearance: NSAppearance(named: .aqua)!, accent: nil)
        XCTAssertEqual(b.controlSize, .regular, "explicit medium still maps")
        f.rows.button = [:]
        ButtonConfigurationMac.apply(f, to: b, appearance: NSAppearance(named: .aqua)!, accent: nil)
        XCTAssertEqual(b.controlSize, .small, "restore the pristine control's own value")
    }
    func testNativeMenuItemKeepsItsSubtitle() throws {
        _ = NSApplication.shared
        let p = Presenter()
        p.buttonFace = { _ in
            var f = ButtonFace(); f.title = "Last Parked"; f.subtitle = "Updated just now"; return f
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["popover": "auto", "id": "menu"]],
            ["op": "create", "id": 2, "kind": "control", "props": ["type": "button"], "handlers": ["press"]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]]]))
        let menu = p.menus.menu(of: try XCTUnwrap(p.views[1]))
        let item = try XCTUnwrap(menu.items.first)
        if #available(macOS 14.4, *) {
            XCTAssertEqual(item.title, "Last Parked")
            XCTAssertEqual(item.subtitle, "Updated just now")
        } else {
            XCTAssertEqual(item.title, "Last Parked — Updated just now")
        }
    }
    func testAllPlacementsFontAxesAlignmentSizesAndSymbolRows() throws {
        var f = ButtonFace(); f.title = "Title"; f.symbol = "lock.fill"
        for (name, position) in [("leading", NSControl.ImagePosition.imageLeading), ("trailing", .imageTrailing), ("top", .imageAbove), ("bottom", .imageBelow)] {
            f.placement = name; XCTAssertEqual(configured(f).imagePosition, position)
        }
        for (name, size) in [("mini", NSControl.ControlSize.mini), ("small", .small), ("medium", .regular), ("large", .large)] {
            f.rows.button["control_size"] = .string(name)
            let b = configured(f)
            XCTAssertEqual(b.controlSize, size); XCTAssertEqual(b.font?.pointSize, NSFont.systemFontSize(for: size))
            f.rows.title["font_weight"] = .number(600)
            let weighted = configured(f)
            XCTAssertEqual(weighted.font?.pointSize, NSFont.systemFontSize(for: size)); XCTAssertEqual(TextEngine.controlWeight(weighted.font!), 600)
            f.rows.title = [:]
        }
        for (name, alignment) in [("start", NSTextAlignment.left), ("end", .right), ("left", .left), ("right", .right), ("center", .center)] {
            f.rows.title["text_align"] = .string(name); XCTAssertEqual(configured(f).alignment, alignment)
        }
        XCTAssertEqual(ButtonConfigurationMac.alignment("start", rtl: true), .right)
        XCTAssertEqual(ButtonConfigurationMac.alignment("end", rtl: true), .left)
        f.rows.title["font_size"] = .number(23)
        f.rows.symbol = ["font_size": .number(31), "font_weight": .number(700), "tint_color": .array([.number(0), .number(255), .number(0), .number(255)])]
        let b = configured(f)
        XCTAssertEqual(b.font?.pointSize, 23)
        XCTAssertEqual(b.symbolConfiguration, NSImage.SymbolConfiguration(pointSize: 31, weight: .bold))
        XCTAssertFalse(try XCTUnwrap(b.image).isTemplate)
        let session = try fixture(); defer { session.destroy() }
        XCTAssertEqual(try button(session, "check-update").alignment, .left)
        XCTAssertEqual(try button(session, "control-tile").imagePosition, .imageAbove)
        XCTAssertNotNil(try button(session, "resend-code").contentTintColor)
        XCTAssertNotNil(try button(session, "sign-in").bezelColor)
        // LLP 1115 wave 1: an icon-only button's label is its help tag; a titled one has none unasked.
        XCTAssertEqual(try button(session, "clear").toolTip, "Clear")
        XCTAssertNil(try button(session, "sign-in").toolTip)
    }
    /// LLP 1115 wave 1: `plain` is SwiftUI's, in the label colour;
    /// `borderless` keeps the accent; an authored colour wins over both.
    func testPlainDrawsInLabelColourAndBorderlessInTheAccent() {
        var f = ButtonFace(); f.title = "Title"; f.macos = "borderless"
        f.style = "plain"
        XCTAssertEqual(configured(f).contentTintColor, .labelColor)
        f.style = "borderless"
        XCTAssertEqual(configured(f).contentTintColor, .controlAccentColor)
        f.style = "plain"
        f.rows.title = ["text_color": .array([.number(255), .number(0), .number(0), .number(255)])]
        XCTAssertNotEqual(configured(f).contentTintColor, .labelColor)
    }
    func testEveryUnsupportedRowHasAStandInAndRadiusWins() throws {
        var f = ButtonFace(); f.title = "Title"; f.subtitle = "Subtitle"; f.rows.imageGap = 17
        f.rows.title = ["white_space": .string("normal"), "line_clamp": .number(2)]
        for side in ["top", "right", "bottom", "left"] { f.rows.button["padding_" + side] = .number(10) }
        for corner in ["dynamic", "small", "medium", "large"] {
            f.rows.button["control_corner_style"] = .string(corner)
            let b = configured(f), rows = ButtonConfigurationMac.observation(f, button: b)
            for row in ["gap", "subtitle", "white-space", "line-clamp", "-exact-corner-style", "padding-top", "padding-right", "padding-bottom", "padding-left"] {
                XCTAssertNotNil((rows[row] as? [String: Any])?["standIn"], row)
            }
            XCTAssertEqual(b.title, "Title")
        }
        f.rows.button["control_corner_style"] = .string("capsule")
        let b = configured(f)
        if #available(macOS 26, *), LinkedDesign.liquidGlass { XCTAssertEqual(b.borderShape, .capsule) }
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] { f.rows.button["border_radius_" + corner] = .number(3) }
        ButtonConfigurationMac.corners(f, to: b, size: NSSize(width: 120, height: 40))
        let rows = ButtonConfigurationMac.observation(f, button: b)
        XCTAssertNotNil((rows["border-radius"] as? [String: Any])?["standIn"])
        if #available(macOS 26, *), LinkedDesign.liquidGlass { XCTAssertEqual(b.borderShape, .automatic) }
        for corner in ["top_left", "top_right", "bottom_right", "bottom_left"] { f.rows.button["border_radius_" + corner] = .number(20) }
        b.frame = NSRect(x: 0, y: 0, width: 120, height: 40)
        ButtonConfigurationMac.corners(f, to: b, size: b.bounds.size)
        if #available(macOS 26, *), LinkedDesign.liquidGlass {
            XCTAssertEqual(b.borderShape, .capsule)
            XCTAssertNil((ButtonConfigurationMac.observation(f, button: b)["border-radius"] as? [String: Any])?["standIn"])
        }
        let session = try fixture(); defer { session.destroy() }
        let padded = try button(session, "radius-padding")
        XCTAssertEqual(padded.alignmentRect(forFrame: padded.frame), padded.owner?.bounds, "padding is an ignored inset stand-in, never exterior spacing")
        XCTAssertNotNil((session.presenter.controls.observation(padded.owner!)?["rows"] as? [String: Any])?["padding-left"])
        let tinted = try button(session, "tinted")
        XCTAssertNotNil((session.presenter.controls.observation(tinted.owner!)?["standIns"] as? [String: String])?["buttonStyle"])
    }
    func testFittingSizeMatchesFrameAfterWindowLayoutAndPushButtonsDoNotWrap() throws {
        let host = makeWindow().contentView!, cache = ButtonMeasureCache()
        let appearance = NSAppearance(named: .aqua)!
        for size in ["mini", "small", "medium", "large"] {
            for placement in ["leading", "trailing", "top", "bottom"] {
                var f = ButtonFace(); f.title = "Send this very long message to everyone in the group"; f.symbol = "lock.fill"; f.placement = placement
                f.rows.button["control_size"] = .string(size)
                var heights: [Float] = []
                for width in [CGFloat(90), 300] {
                    let b = configured(f)
                    b.translatesAutoresizingMaskIntoConstraints = false; host.addSubview(b)
                    b.widthAnchor.constraint(equalToConstant: width).isActive = true
                    host.layoutSubtreeIfNeeded()
                    let answer = cache.measure(f, widthKind: 0, width: width, appearance: appearance, scale: 1)
                    XCTAssertEqual(answer.width, Float(b.frame.width), size + placement)
                    XCTAssertEqual(answer.height, Float(ceil(b.frame.height)), size + placement)
                    heights.append(answer.height); b.removeFromSuperview()
                }
                XCTAssertEqual(heights[0], heights[1], "AppKit push buttons do not wrap")
            }
        }
    }
    func testCacheKeysIncludeFaceRowsStyleSizeWidthAppearanceAndScale() throws {
        let cache = ButtonMeasureCache(), aqua = NSAppearance(named: .aqua)!
        cache.configure(aqua, scale: 1)
        let plain = try data()
        XCTAssertEqual(cache.answer(face: plain, widthKind: 0, width: 100).provisional, 1)
        XCTAssertEqual(cache.answer(face: plain, widthKind: 0, width: 100).provisional, 0)
        XCTAssertEqual(cache.misses, 1)
        for variant in [try data(title: "Other"), try data(["title": ["font_weight": 600]]), try data(style: "gray"), try data(["button": ["control_size": "large"]])] {
            XCTAssertEqual(cache.answer(face: variant, widthKind: 0, width: 100).provisional, 1)
        }
        for (kind, width) in [(UInt8(0), Float(300)), (1, 0), (2, 0)] { XCTAssertEqual(cache.answer(face: plain, widthKind: kind, width: width).provisional, 1) }
        for (appearance, scale) in [(NSAppearance(named: .darkAqua)!, CGFloat(1)), (aqua, CGFloat(2))] {
            XCTAssertTrue(cache.configure(appearance, scale: scale)); XCTAssertEqual(cache.answer(face: plain, widthKind: 0, width: 100).provisional, 1)
            XCTAssertEqual(cache.answer(face: plain, widthKind: 0, width: 100).provisional, 0)
        }
    }
    func testColdLaunchAndNewFaceAfterLaunchPublishOnlyExactGeometry() throws {
        let win = makeWindow(), session = ExactApp.shared.makeSession(label: "new-button-measure")
        defer { session.destroy() }
        win.contentView = session.presenter.viewport
        let source = root.appendingPathComponent("target/1104-b3/miss.contract")
        try """
        component Miss
          state more = false
          action show
            more = true
          view
            column font-size=44 color="red"
              button appearance="auto" testId="show" press=show
                text "Show"
              when more
                button appearance="auto" testId="new" -exact-control-size="large" font-size=25
                  text "A new measured face"

        """.write(to: source, atomically: true, encoding: .utf8)
        XCTAssertNil(session.boot(plan: try compile(source), size: win.contentLayoutRect.size).error)
        XCTAssertGreaterThan(session.buttonMeasurements.misses, 0)
        XCTAssertEqual(try button(session, "show").font?.pointSize, NSFont.systemFontSize(for: .regular), "ancestor typography does not reach the native face")
        let misses = session.buttonMeasurements.misses
        try button(session, "show").performClick(nil)
        XCTAssertGreaterThan(session.buttonMeasurements.misses, misses)
        XCTAssertEqual(try button(session, "new").font?.pointSize, 25)
        XCTAssertEqual(session.fieldChrome.presentedProvisional, 0)
        let before = session.presenter.views.mapValues { $0.frame }
        session.presenter.controls.sync()
        XCTAssertEqual(session.presenter.views.mapValues { $0.frame }, before)
        XCTAssertTrue(session.buttonMeasurements.configure(NSAppearance(named: .darkAqua)!, scale: 2))
    }
    func testDisabledAuthorColoursAndPointerEvents() throws {
        let session = try fixture(); defer { session.destroy() }
        let b = try button(session, "disabled-authored")
        XCTAssertFalse(b.isEnabled); XCTAssertNotNil(b.contentTintColor)
        XCTAssertNotNil(b.attributedTitle.attribute(.foregroundColor, at: 0, effectiveRange: nil))
        XCTAssertFalse(try XCTUnwrap(b.image).isTemplate)
        let standIns = try XCTUnwrap(session.presenter.controls.observation(b.owner!)?["standIns"] as? [String: String])
        XCTAssertNotNil(standIns["color"]); XCTAssertNotNil(standIns["symbol.-exact-tint-color"])
        let none = try button(session, "pointer-none"), auto = try button(session, "pointer-override")
        XCTAssertNil(none.hitTest(none.frame.origin))
        XCTAssertTrue(auto.hitTest(auto.frame.origin) === auto)
        XCTAssertEqual(session.presenter.controls.observation(none.owner!)?["pointerEvents"] as? String, "none")
    }
    func testNativeInvokersActivateOnceThenPresentAndAnchorToButtonFrame() throws {
        let session = try fixture(); defer { session.destroy() }
        let dialog = try button(session, "open-dialog")
        var count = 0
        session.presenter.onPress = { _ in count += 1 }
        dialog.performClick(nil)
        XCTAssertEqual(count, 1); XCTAssertNotNil(session.presenter.dialogs.active)
        if let active = session.presenter.dialogs.active { session.presenter.dialogs.close(active) }
        let b = try button(session, "open-popover"), owner = try XCTUnwrap(b.owner)
        b.performClick(nil)
        XCTAssertEqual(count, 2)
        XCTAssertEqual(session.presenter.menus.presented.count, 1)
        XCTAssertEqual(session.presenter.menus.anchor(owner, in: session.presenter.viewport), b.convert(b.bounds, to: session.presenter.viewport))
        let pop = try XCTUnwrap(session.presenter.menus.presented.first)
        session.presenter.menus.close(pop)
        owner.applyProps(set: ["commandfor": "button-popover", "command": "show-popover"], clear: ["popovertarget"])
        b.performClick(nil); XCTAssertEqual(count, 3); XCTAssertTrue(session.presenter.menus.isOpen(pop))
        owner.applyProps(set: ["command": "toggle-popover"], clear: [])
        b.performClick(nil); XCTAssertEqual(count, 4); XCTAssertFalse(session.presenter.menus.isOpen(pop))
    }

    /// Fixture intent is written independently of ButtonConfigurationMac.
    private func reference(_ id: String) throws -> NSButton {
        let glass = id.hasPrefix("glass-")
        let name = glass ? String(id.dropFirst(6)) : id
        let b = NSButton(title: "", target: nil, action: nil)
        let borderless: Set<String> = ["plain", "borderless", "get-app", "try-demo", "subscriptions", "sign-out", "clear", "a-plain"]
        let accent: Set<String> = ["filled", "bordered-prominent", "control-tile", "sign-in", "radius-padding", "pointer-none", "open-dialog", "wide", "narrow"]
        let glassStyles: Set<String> = ["glass", "prominent-glass", "clear-glass", "prominent-clear-glass", "symbol-only", "a-glass", "a-clear", "a-sf", "g1", "g2", "g3", "pointer-override"]
        let titles: [String: String] = ["defrost-front": "Front Defrost", "defrost-rear": "Rear Defrost", "control-tile": "Lock",
            "subtitle": "Last Parked", "check-update": "Check for update", "sign-in": "Sign In", "resend-code": "Resend Code",
            "method-email": "Email", "method-passkey": "Passkey", "get-app": "Get the Lexus app for your vehicle",
            "try-demo": "Try Demo", "subscriptions": "Manage Subscriptions", "sign-out": "Sign Out", "radius-padding": "Radius 18",
            "disabled-authored": "Disabled colours", "nowrap": "A long title truncated at the end", "clamped": "A long title that stops after at most two lines",
            "pointer-override": "Takes touch", "pointer-none": "Passes touch through", "open-dialog": "Open confirmation", "open-popover": "Open popover",
            "confirm-action": "Confirm", "confirm-cancel": "Cancel", "symbol-title": "Send", "title-symbol": "Forward", "wide": "Fixed 240 wide",
            "a-plain": "plain", "a-gray": "gray", "a-bordered": "bordered", "a-glass": "glass", "a-clear": "clear", "a-tinted": "tinted",
            "narrow": "Send this very long message", "stretched": "Stretched (default bordered)", "disabled": "Disabled", "g1": "Lock", "g2": "Unlock", "g3": "Fade"]
        let symbols: [String: String] = ["defrost-front": "windshield.front.and.wiper", "defrost-rear": "windshield.rear.and.wiper", "control-tile": "lock.fill",
            "subtitle": "location.fill", "check-update": "arrow.clockwise", "method-email": "envelope.fill", "method-passkey": "key.fill", "clear": "xmark.circle.fill",
            "disabled-authored": "lock.fill", "symbol-only": "plus", "symbol-title": "arrow.up", "title-symbol": "arrowshape.turn.up.right", "a-plain": "arrow.up",
            "a-gray": "arrow.up", "a-bordered": "arrow.up", "a-glass": "arrow.up", "a-clear": "arrow.up", "a-tinted": "arrow.up", "a-sf": "paperplane.fill"]
        b.bezelStyle = .push
        if #available(macOS 26, *), LinkedDesign.liquidGlass, glass || glassStyles.contains(name) { b.bezelStyle = .glass }
        b.isBordered = glass || !borderless.contains(name)
        if name == "sign-in" { b.controlSize = .large }
        let small = ["get-app", "try-demo", "subscriptions", "sign-out", "clear"].contains(name)
        let fontSize = small ? 13 : NSFont.systemFontSize(for: b.controlSize)
        if small || name == "sign-in" {
            // CSS 600 is the SF variable axis, not AppKit's approximate named weight.
            let base = NSFont.systemFont(ofSize: fontSize, weight: .semibold)
            let descriptor = CTFontDescriptorCreateWithAttributes([kCTFontVariationAttribute: [0x77676874: 600]] as CFDictionary)
            b.font = CTFontCreateCopyWithAttributes(base as CTFont, fontSize, nil, descriptor) as NSFont
        } else { b.font = NSFont.systemFont(ofSize: fontSize) }
        let green = NSColor(red: 52.0 / 255, green: 199.0 / 255, blue: 89.0 / 255, alpha: 1)
        let tint = name.hasPrefix("a-") || name == "wide" ? green : NSColor.controlAccentColor
        if !glass && accent.contains(name) || ["prominent-glass", "prominent-clear-glass"].contains(name) { b.bezelColor = tint }
        // `plain` is SwiftUI's: the label colour, not the accent (LLP 1115 wave 1).
        let plain: Set<String> = ["plain", "get-app", "try-demo", "subscriptions", "sign-out", "clear", "a-plain"]
        b.contentTintColor = b.isBordered ? nil : plain.contains(name) ? .labelColor : tint
        b.title = titles[name] ?? (symbols[name] == nil ? name : "")
        if name == "resend-code" || name == "disabled-authored" {
            b.contentTintColor = .controlAccentColor
            let p = NSMutableParagraphStyle(); p.alignment = .center
            b.attributedTitle = NSAttributedString(string: b.title, attributes: [.font: b.font!, .foregroundColor: NSColor.controlAccentColor, .paragraphStyle: p])
        }
        b.alignment = name == "check-update" ? .left : .center
        b.image = symbols[name].flatMap { NSImage(systemSymbolName: $0, accessibilityDescription: nil) }
        b.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: name == "control-tile" ? 28 : fontSize, weight: name == "control-tile" || small || name == "sign-in" ? .semibold : .regular)
        if ["control-tile", "disabled-authored"].contains(name), let image = b.image {
            let tint = NSColor(red: 0, green: 128.0 / 255, blue: 0, alpha: 1)
            b.image = image.withSymbolConfiguration(b.symbolConfiguration!.applying(NSImage.SymbolConfiguration(paletteColors: [tint])))
            b.image?.isTemplate = false
        }
        b.imagePosition = b.image == nil ? .noImage : b.title.isEmpty ? .imageOnly : name == "control-tile" ? .imageAbove : name == "title-symbol" ? .imageTrailing : .imageLeading
        if #available(macOS 26, *), LinkedDesign.liquidGlass, name == "sign-in" || name == "radius-padding" { b.borderShape = .capsule }
        b.lineBreakMode = .byTruncatingTail
        b.isEnabled = name != "disabled-authored" && name != "disabled"
        return b
    }
    private func pixels(_ b: NSButton, host: NSView, label: String) throws -> [UInt8] {
        host.addSubview(b)
        host.layoutSubtreeIfNeeded()
        let image = try XCTUnwrap(b.bitmapImageRepForCachingDisplay(in: b.bounds))
        b.cacheDisplay(in: b.bounds, to: image)
        let evidence = root.appendingPathComponent("target/1104-b3/pixels")
        try FileManager.default.createDirectory(at: evidence, withIntermediateDirectories: true)
        try image.representation(using: .png, properties: [:])?.write(to: evidence.appendingPathComponent(label + ".png"))
        let cg = try XCTUnwrap(image.cgImage)
        var bytes = [UInt8](repeating: 0, count: cg.width * cg.height * 4)
        let context = try XCTUnwrap(CGContext(data: &bytes, width: cg.width, height: cg.height, bitsPerComponent: 8, bytesPerRow: cg.width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.draw(cg, in: CGRect(x: 0, y: 0, width: cg.width, height: cg.height))
        let colours = Set(stride(from: 0, to: bytes.count, by: 4).map { UInt32(bytes[$0]) << 24 | UInt32(bytes[$0 + 1]) << 16 | UInt32(bytes[$0 + 2]) << 8 | UInt32(bytes[$0 + 3]) })
        XCTAssertGreaterThan(colours.count, 2, "capture must contain visible ink, not a blank diff")
        b.removeFromSuperview()
        return bytes
    }
    func testAllFixtureRowsPixelDiffAgainstHandConfiguredAppKit() throws {
        let session = try fixture(); defer { session.destroy() }
        let host = try XCTUnwrap(window.contentView)
        let buttons = session.presenter.controls.controls.values.compactMap { $0 as? NativeButtonMac }.sorted { ($0.written?.testId ?? "") < ($1.written?.testId ?? "") }
        XCTAssertGreaterThan(buttons.count, 50)
        for native in buttons {
            let id = try XCTUnwrap(native.written?.testId), hand = try reference(id)
            let mount = native.superview, frame = native.frame
            native.frame = NSRect(x: 20, y: 100, width: frame.width, height: frame.height)
            hand.frame = native.frame
            if #available(macOS 26, *), LinkedDesign.liquidGlass, id == "control-tile" {
                let size = hand.alignmentRect(forFrame: hand.frame).size
                hand.borderShape = 18 >= min(size.width, size.height) / 2 ? .capsule : .automatic
            }
            let actual = try pixels(native, host: host, label: id + "-native"), expected = try pixels(hand, host: host, label: id + "-hand")
            native.frame = frame; mount?.addSubview(native)
            XCTAssertEqual(actual.count, expected.count, id)
            guard actual.count == expected.count else { continue }
            var different = 0
            for i in stride(from: 0, to: actual.count, by: 4) {
                if (0..<4).contains(where: { abs(Int(actual[i + $0]) - Int(expected[i + $0])) > 8 }) { different += 1 }
            }
            let fraction = Double(different) / Double(actual.count / 4)
            print("button-pixel-diff \(id): \(different)/\(actual.count / 4) pixels >8 (\(fraction * 100)%)")
            XCTAssertLessThanOrEqual(fraction, 0.02, id)
        }
    }
}
#endif
