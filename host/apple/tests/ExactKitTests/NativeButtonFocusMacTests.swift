// @ref LLP 1104 D6 — one native owner, independent of Keyboard navigation.
#if os(macOS)
import AppKit
import ObjectiveC
import XCTest
@testable import ExactKit

final class NativeButtonFocusMacTests: XCTestCase {
    private var window: NSWindow!
    override func tearDown() { window?.close(); window = nil }

    private func fixture(_ firstProps: [String: String] = [:], firstHandlers: [String] = ["focus", "blur", "key", "keyup"]) throws -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        var face = ButtonFace(); face.title = "Focus"
        p.buttonFace = { _ in face }
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.autorecalculatesKeyViewLoop = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press", "key", "keyup"]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 300],
            ["op": "create", "id": 2, "kind": "control", "props": ["type": "button", "id": "first"].merging(firstProps) { $1 }, "handlers": firstHandlers, "style": ["appearance": "auto"]],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 120, "h": 30],
            ["op": "create", "id": 3, "kind": "button", "props": ["id": "bare"], "handlers": []],
            ["op": "frame", "id": 3, "x": 0, "y": 50, "w": 120, "h": 30],
            ["op": "create", "id": 4, "kind": "control", "props": ["type": "button", "id": "last"], "handlers": ["press"], "style": ["appearance": "auto"]],
            ["op": "frame", "id": 4, "x": 0, "y": 100, "w": 120, "h": 30],
            ["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]]))
        p.flushKeyViewLoop()
        return p
    }
    private func key(_ chars: String, _ code: UInt16, flags: NSEvent.ModifierFlags = [], up: Bool = false) throws -> NSEvent {
        try XCTUnwrap(NSEvent.keyEvent(with: up ? .keyUp : .keyDown, location: .zero, modifierFlags: flags, timestamp: 0,
                                     windowNumber: window.windowNumber, context: nil, characters: chars,
                                     charactersIgnoringModifiers: chars, isARepeat: false, keyCode: code))
    }
    private func deliver(_ p: Presenter, _ event: NSEvent) {
        if !p.routeKey(event, focused: true, in: window) { window.sendEvent(event) }
    }

    /// Override only the process's AppKit query; never change the person's preferences.
    /// AppKit's key-view eligibility and traversal are exercised with both answers.
    func testTabAndShiftTabUseOnlyTheNativeOwnerWithKeyboardNavigationOnAndOff() throws {
        let method = try XCTUnwrap(class_getInstanceMethod(NSApplication.self, #selector(getter: NSApplication.isFullKeyboardAccessEnabled)))
        let original = method_getImplementation(method)
        defer { method_setImplementation(method, original) }
        for enabled in [false, true] {
            let query: @convention(block) (AnyObject) -> Bool = { _ in enabled }
            let replacement = imp_implementationWithBlock(query)
            method_setImplementation(method, replacement)
            defer { method_setImplementation(method, original); imp_removeBlock(replacement) }
            XCTAssertEqual(NSApp.isFullKeyboardAccessEnabled, enabled)
            let p = try fixture()
            let first = try XCTUnwrap(p.controls.controls[2] as? NativeButtonMac)
            let bare = try XCTUnwrap(p.views[3])
            let last = try XCTUnwrap(p.controls.controls[4] as? NativeButtonMac)
            XCTAssertTrue(first.acceptsFirstResponder)
            XCTAssertTrue(first.canBecomeKeyView)
            XCTAssertFalse(try XCTUnwrap(p.views[2]).acceptsFirstResponder)
            XCTAssertFalse(try XCTUnwrap(p.views[2]).canBecomeKeyView)
            XCTAssertEqual(first.focusRingType, .default)
            XCTAssertTrue(try XCTUnwrap(p.views[2]).focusRingMaskBounds.isEmpty)
            XCTAssertTrue(first.nextKeyView === bare)
            XCTAssertTrue(bare.nextKeyView === last)
            XCTAssertTrue(last.nextKeyView === p.views[1], "the pressable ancestor has its own independent stop")
            XCTAssertTrue(window.makeFirstResponder(first))
            deliver(p, try key("\t", 48))
            XCTAssertTrue(window.firstResponder === bare)
            XCTAssertEqual(bare.focusRingMaskBounds, bare.bounds, "no own press handler needed")
            deliver(p, try key("\t", 48))
            XCTAssertTrue(window.firstResponder === last)
            deliver(p, try key("\t", 48, flags: .shift))
            XCTAssertTrue(window.firstResponder === bare)
            deliver(p, try key("\t", 48, flags: .shift))
            XCTAssertTrue(window.firstResponder === first)
            window.close(); window = nil
        }
    }

    func testFocusBlurKeyAndCollectionReadsRelayToTheNodeOnce() throws {
        let p = try fixture()
        let first = try XCTUnwrap(p.controls.controls[2] as? NativeButtonMac)
        var focuses: [UInt32] = [], blurs: [UInt32] = [], keys: [UInt32] = []
        p.onFocus = { focuses.append($0) }; p.onBlur = { blurs.append($0) }
        p.onKey = { id, _ in keys.append(id) }
        p.focusElement(["first"])
        XCTAssertTrue(window.firstResponder === first)
        XCTAssertEqual(focuses, [2])
        XCTAssertTrue(p.focusedNode === p.views[2])
        XCTAssertEqual(p.collections.focusedView(), 2)
        XCTAssertTrue(p.keyTarget(first) === p.views[2])
        p.focusElement(["first"])
        XCTAssertEqual(focuses, [2], "no second becomeFirstResponder")
        deliver(p, try key("x", 7))
        deliver(p, try key("x", 7, up: true))
        XCTAssertEqual(keys, [2, 1, 2, 1])
        p.blurElement(["last"])
        XCTAssertTrue(window.firstResponder === first)
        p.blurElement(["first"])
        XCTAssertEqual(blurs, [2])
        XCTAssertNil(p.focusedNode)
    }

    func testSpaceAndReturnPressExactlyOnceAndCancellationPreventsTheAction() throws {
        let p = try fixture()
        var pressed: [UInt32] = [], heard: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.onKey = { id, _ in heard.append(id) }
        for (chars, code) in [(" ", UInt16(49)), ("\r", UInt16(36))] {
            for target in ["first", "bare", "last"] {
                p.focusElement([target])
                let count = pressed.count
                deliver(p, try key(chars, code))
                deliver(p, try key(chars, code, up: true))
                XCTAssertEqual(pressed.count, count + 1)
                XCTAssertEqual(pressed.last, target == "last" ? 4 : 1, "a handlerless button activates its ancestor")
                XCTAssertEqual(p.focusedNode?.props["id"], target, "activation preserves the button's owner")
            }
        }
        p.focusElement(["first"])
        p.onKey = { id, _ in heard.append(id); p.defaultPrevented = true }
        let count = pressed.count
        deliver(p, try key(" ", 49)); deliver(p, try key("\r", 36))
        XCTAssertEqual(pressed.count, count)
        XCTAssertEqual(heard.suffix(4), [2, 1, 2, 1])
    }

    func testAutofocusAndRestartRestoreTheNativeOwner() throws {
        let p = try fixture(["autofocus": "true"])
        let node = try XCTUnwrap(p.views[2]), button = try XCTUnwrap(p.controls.controls[2])
        XCTAssertTrue(window.firstResponder === button)
        let tree = "{\"roots\":[1],\"nodes\":[{\"id\":1,\"type\":\"view\",\"children\":[2,3,4]},{\"id\":2,\"parent\":1,\"type\":\"control\",\"children\":[]}]}"
        let kept = try XCTUnwrap(p.focusPlace(tree: tree))
        p.blurElement([])
        p.restoreFocus(kept, tree: tree)
        XCTAssertTrue(window.firstResponder === button)
        XCTAssertTrue(p.focusedNode === node)
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [3, 4]], ["op": "destroy", "id": 2]]))
        XCTAssertNil(p.focusedNode, "a removed native owner is no logical focus")
        XCTAssertNil(p.collections.focusedView())
    }

    func testRetainFocusKeepsThePreviousOwnerWhileAnAncestorHandlesThePress() throws {
        let p = try fixture(["retainFocus": "true"])
        var pressed: [UInt32] = []; p.onPress = { pressed.append($0) }
        p.focusElement(["bare"])
        try XCTUnwrap(p.controls.controls[2] as? NSButton).performClick(nil)
        XCTAssertTrue(window.firstResponder === p.views[3])
        XCTAssertEqual(pressed, [1])
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["retainFocus": "false"]],
                          ["op": "props", "id": 1, "set": ["retainFocus": "true"]]]))
        try XCTUnwrap(p.controls.controls[2] as? NSButton).performClick(nil)
        XCTAssertTrue(window.firstResponder === p.views[3])
        XCTAssertEqual(pressed, [1, 1])
    }

    func testNegativeTabIndexStillAllowsScriptFocusAndRestrictionsRefuseIt() throws {
        let p = try fixture(["tabIndex": "-1"])
        let button = try XCTUnwrap(p.controls.controls[2] as? NativeButtonMac)
        XCTAssertTrue(button.acceptsFirstResponder)
        XCTAssertFalse(button.canBecomeKeyView)
        p.focusElement(["first"])
        XCTAssertTrue(window.firstResponder === button)
        p.blurElement([])
        for prop in ["disabled", "inert"] {
            p.apply(wireBatch([["op": "props", "id": 2, "set": [prop: "true"]]]))
            XCTAssertFalse(button.acceptsFirstResponder)
            p.focusElement(["first"])
            XCTAssertFalse(window.firstResponder === button)
            p.apply(wireBatch([["op": "props", "id": 2, "set": [prop: "false"]]]))
        }
        try XCTUnwrap(p.views[2]).isHidden = true
        XCTAssertFalse(button.acceptsFirstResponder)
        p.focusElement(["first"])
        XCTAssertFalse(window.firstResponder === button)
    }

    private func valueFixture() throws -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.selectOptions = { _ in SelectMenu(options: [.init(value: "a", label: "Alpha", disabled: false),
                                                     .init(value: "b", label: "Beta", disabled: false)], chosen: 0) }
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.autorecalculatesKeyViewLoop = false
        window.contentView = p.viewport
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 500, "h": 400],
            ["op": "create", "id": 2, "kind": "button", "props": ["id": "before"]],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 160, "h": 30]]
        for (index, kind) in ["date", "time", "datetime-local", "select"].enumerated() {
            let id = index + 3
            let value = kind == "date" ? "2026-10-08" : kind == "time" ? "09:30" : kind == "select" ? "a" : "2026-10-08T09:30"
            ops.append(["op": "create", "id": id, "kind": "control", "props": ["id": kind, "type": kind, "value": value],
                        "handlers": ["focus", "blur", "key", "keyup"]])
            ops.append(["op": "frame", "id": id, "x": 0, "y": (index + 1) * 50, "w": 240, "h": 30])
        }
        ops += [["op": "create", "id": 7, "kind": "button", "props": ["id": "after"]],
                ["op": "frame", "id": 7, "x": 0, "y": 250, "w": 160, "h": 30],
                ["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6, 7]], ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops)); p.flushKeyViewLoop()
        return p
    }

    func testDatesAndSelectTabThroughTheirNativeOwnersWithKeyboardNavigationOnAndOff() throws {
        let method = try XCTUnwrap(class_getInstanceMethod(NSApplication.self, #selector(getter: NSApplication.isFullKeyboardAccessEnabled)))
        let original = method_getImplementation(method)
        defer { method_setImplementation(method, original) }
        for enabled in [false, true] {
            let query: @convention(block) (AnyObject) -> Bool = { _ in enabled }
            let replacement = imp_implementationWithBlock(query)
            method_setImplementation(method, replacement)
            defer { method_setImplementation(method, original); imp_removeBlock(replacement) }
            let p = try valueFixture()
            var focuses: [UInt32] = [], blurs: [UInt32] = []
            p.onFocus = { focuses.append($0) }; p.onBlur = { blurs.append($0) }
            p.focusElement(["before"])
            for id in [UInt32(3), 4, 5, 6, 7] {
                deliver(p, try key("\t", 48))
                let owner = p.keyView(of: try XCTUnwrap(p.views[id]))
                XCTAssertTrue(window.firstResponder === owner, "Tab reaches native owner \(id), Keyboard navigation=\(enabled)")
                XCTAssertTrue(p.focusedNode === p.views[id])
                XCTAssertEqual(p.collections.focusedView(), id)
                if id < 7 {
                    XCTAssertFalse(try XCTUnwrap(p.views[id]).acceptsFirstResponder, "wrapper is not another focus owner")
                    XCTAssertTrue(owner.canBecomeKeyView)
                }
            }
            for id in [UInt32(6), 5, 4, 3, 2] {
                deliver(p, try key("\t", 48, flags: .shift))
                XCTAssertTrue(window.firstResponder === p.keyView(of: try XCTUnwrap(p.views[id])))
            }
            XCTAssertEqual(focuses, [3, 4, 5, 6, 6, 5, 4, 3])
            XCTAssertEqual(blurs, [3, 4, 5, 6, 6, 5, 4, 3])
            window.close(); window = nil
        }
    }

    func testNativeValueFocusHonorsNegativeTabIndexAndRestrictions() throws {
        let p = try valueFixture()
        for id in [UInt32(3), 4, 5, 6] {
            let node = try XCTUnwrap(p.views[id]), control = try XCTUnwrap(p.controls.controls[id])
            p.apply(wireBatch([["op": "props", "id": id, "set": ["tabIndex": "-1"]]]))
            XCTAssertFalse(Presenter.tabbable(node)); XCTAssertFalse(control.canBecomeKeyView)
            p.focusElement([node.props["id"]!])
            XCTAssertTrue(window.firstResponder === control, "negative tabindex allows explicit focus")
            p.blurElement([node.props["id"]!]); XCTAssertNil(p.focusedNode)
            for prop in ["disabled", "inert"] {
                p.apply(wireBatch([["op": "props", "id": id, "set": [prop: "true"]]]))
                XCTAssertFalse(control.acceptsFirstResponder)
                p.focusElement([node.props["id"]!]); XCTAssertFalse(window.firstResponder === control)
                p.apply(wireBatch([["op": "props", "id": id, "set": [prop: "false"]]]))
            }
            node.isHidden = true
            XCTAssertFalse(control.acceptsFirstResponder); XCTAssertFalse(Presenter.tabbable(node))
            node.isHidden = false
            p.apply(wireBatch([["op": "style", "id": id, "style": ["visibility": "hidden"]]]))
            XCTAssertTrue(node.cssVisibilityHidden, "the style op applies visibility to native control \(id)")
            XCTAssertFalse(control.acceptsFirstResponder); XCTAssertFalse(Presenter.tabbable(node))
            p.focusElement([node.props["id"]!]); XCTAssertFalse(window.firstResponder === control)
            p.apply(wireBatch([["op": "style", "id": id, "style": ["visibility": "visible"]]]))
            XCTAssertFalse(node.cssVisibilityHidden)
            XCTAssertTrue(control.acceptsFirstResponder, "visibility restoration restores explicit focus")
        }
    }

    /// Focused native date/select keys participate in Contract's cancellable
    /// handlers before AppKit edits; cancellation leaves their current values.
    func testNativeValueKeysUseTheFocusedNodeAndRespectCancellation() throws {
        let p = try valueFixture()
        var heard: [UInt32] = []
        p.onKey = { id, _ in heard.append(id); p.defaultPrevented = true }
        for id in [UInt32(3), 4, 5, 6] {
            let node = try XCTUnwrap(p.views[id])
            p.focusElement([node.props["id"]!])
            let control = try XCTUnwrap(p.controls.controls[id])
            let before = p.controls.valueObservation(control)
            deliver(p, try key("\u{F700}", 126))
            deliver(p, try key("\u{F700}", 126, up: true))
            XCTAssertEqual(heard.suffix(2), [id, id])
            XCTAssertEqual(p.controls.valueObservation(control)?["value"] as? String, before?["value"] as? String)
            XCTAssertTrue(window.firstResponder === control)
        }
    }

}
#endif
