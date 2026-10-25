#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// The number pads' bar (LLP 1116.003 D7): the decimal pad has no minus key
/// and no Return key, so a field on it gets, in the keyboard's accessory bar,
/// ± where its `min` is below zero, which flips the sign and fires `input` as
/// a keystroke does (round 5's unit converter), and Done, always (LLP
/// 1116.008 D8; expense-tracker's amount had no way to put the pad away),
/// which does what Return does and puts the keyboard away. A keyboard that
/// has both keys gets no bar.
///
/// LLP 1116.006 D6 over `scripts/fixtures/pad-bar.contract`: the bar is the
/// platform's. Round 8's converter showed an opaque white band across the
/// width above the keys, clipping the TO header: the bar paints no fill of
/// its own (UIKit's glass keys over the content), and what showed through it
/// was the canvas below the viewport the keys had shortened. The focused
/// field and its section stay above the keys' top, the bar's included.
final class PadBarIOSTests: XCTestCase {
    private var session: ExactSession?
    private var sessionWindow: UIWindow?

    override func tearDown() {
        session?.destroy()
        session = nil
        sessionWindow?.isHidden = true
        sessionWindow = nil
        super.tearDown()
    }
    private func waitFor(_ what: String, file: StaticString = #filePath, line: UInt = #line, _ ready: () -> Bool) {
        func done() -> Bool {
            guard let p = session?.presenter, !p.navigation.inTransition, !p.modals.inTransition else { return false }
            return ready()
        }
        let deadline = Date().addingTimeInterval(10)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertTrue(done(), what, file: file, line: line)
    }

    private func field(_ props: [String: String], handlers: Set<String> = ["input"]) -> (Presenter, NodeView, UIWindow) {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 300))
        let node = NodeView(id: 1, kind: "input", presenter: p)
        node.frame = CGRect(x: 0, y: 0, width: 300, height: 44)
        node.handlers = handlers
        window.addSubview(node); p.views[node.id] = node
        window.makeKeyAndVisible()
        node.applyProps(set: props, clear: [])
        return (p, node, window)
    }
    private func keys(_ node: NodeView) -> [String] {
        (node.field?.inputAccessoryView as? UIToolbar)?.items?.compactMap { $0.action.map { NSStringFromSelector($0) } } ?? []
    }

    func testANegativeMinGetsAPlusMinusKeyThatFlipsTheSignAsInput() throws {
        let (p, node, window) = field(["inputMode": "decimal", "min": "-40", "value": "12.5"])
        defer { window.isHidden = true }
        var heard: [String] = []
        p.onInput = { [unowned node] _, value in
            heard.append(value)
            node.applyProps(set: ["value": value], clear: [])
        }
        let field = try XCTUnwrap(node.field)
        XCTAssertEqual(field.keyboardType, .decimalPad)
        XCTAssertEqual(keys(node), ["toggleSign", "padDone"], "± and Done: a pad has no Return key")
        let sign = try XCTUnwrap((field.inputAccessoryView as? UIToolbar)?.items?.first)
        XCTAssertEqual(sign.title, "\u{00B1}")
        XCTAssertTrue(sign.target === node, "the key is the field's node's")
        node.toggleSign()
        XCTAssertEqual(heard.last, "-12.5", "the app hears the flipped value as input")
        XCTAssertEqual(field.text, "-12.5")
        node.toggleSign()
        XCTAssertEqual(heard.last, "12.5")
        field.text = ""
        node.toggleSign()
        XCTAssertEqual(heard.last, "-", "an empty field starts a negative number")
        // The bar follows the props: it is the field's only while its keyboard lacks a key.
        node.applyProps(set: ["min": "0"], clear: [])
        XCTAssertEqual(keys(node), ["padDone"], "min 0: no sign, but the pad's Done")
        node.applyProps(set: [:], clear: ["min"])
        XCTAssertEqual(keys(node), ["padDone"], "no min: Done alone")
        node.applyProps(set: ["inputMode": "text", "min": "-1"], clear: [])
        XCTAssertNil(field.inputAccessoryView, "a text keyboard has its minus and its Return")
        node.applyProps(set: ["type": "number", "min": "-273.15"], clear: ["inputMode"])
        XCTAssertEqual(field.keyboardType, .decimalPad)
        XCTAssertEqual(keys(node), ["toggleSign", "padDone"], "type=number with a negative min, as inputmode=decimal")
        // A phone pad has no Return key either, and no minus to ask for.
        node.applyProps(set: ["inputMode": "tel"], clear: ["type"])
        XCTAssertEqual(field.keyboardType, .phonePad)
        XCTAssertEqual(keys(node), ["padDone"])
        // A digits-only pad: Done, whatever the hint or the handlers say.
        node.applyProps(set: ["inputMode": "numeric", "enterKeyHint": "next"], clear: ["min"])
        XCTAssertEqual(field.keyboardType, .numberPad)
        XCTAssertEqual(keys(node), ["padDone"])
    }

    /// LLP 1116.019 D5: `type="number"` takes the number pad when it holds only whole numbers (`step` unsaid,
    /// HTML's 1, or whole, from a whole or unsaid `min`), and the decimal pad when it may hold a fraction.
    func testANumberFieldOfWholeNumbersGetsTheNumberPad() throws {
        let (_, node, window) = field(["type": "number"])
        defer { window.isHidden = true }
        let field = try XCTUnwrap(node.field)
        XCTAssertEqual(field.keyboardType, .numberPad, "unsaid step: HTML's 1")
        for (set, pad) in [(["step": "5", "min": "0"], UIKeyboardType.numberPad), (["step": "2.0", "min": "-10"], .numberPad),
                           (["step": "any"], .decimalPad), (["step": "0.5"], .decimalPad), (["step": "1", "min": "0.5"], .decimalPad),
                           (["step": "nonsense", "min": "1"], .numberPad)] {
            node.applyProps(set: set, clear: ["step", "min"].filter { set[$0] == nil })
            XCTAssertEqual(field.keyboardType, pad, "\(set)")
        }
        node.applyProps(set: ["step": "1", "min": "-40"], clear: [])
        XCTAssertEqual(keys(node), ["toggleSign", "padDone"], "a negative min on the number pad: ± and Done")
        node.applyProps(set: ["inputMode": "decimal"], clear: [])
        XCTAssertEqual(field.keyboardType, .decimalPad, "an authored inputmode wins")
    }

    /// The keys' top, their accessory bar's included: an iPhone 17 Pro's
    /// number pad and the bar, 350 points over the screen's bottom.
    private let keys: CGFloat = 524

    func testTheBarPaintsNoBandAndTheFieldsSectionStaysAboveIt() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_PAD_BAR_PLAN"], "build.mjs --test --ios compiles the pad-bar fixture")
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "pad-bar")
        self.session = session
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = window.screen.bounds
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        sessionWindow = window
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        let p = session.presenter
        waitFor("the initial route and its controls are laid out") {
            p.navigation.primaryNavigation?.view.window != nil && p.views.values.contains {
                $0.props["testId"] == "swap" && $0.window != nil && $0.convert($0.bounds, to: nil).maxY > self.keys
            }
        }
        func node(_ id: String) throws -> NodeView { try XCTUnwrap(p.views.values.first { $0.props["testId"] == id && $0.window != nil }, "no \(id)") }
        func box(_ v: UIView) -> CGRect { v.convert(v.bounds, to: nil) }
        let from = try node("from-value"), swap = try node("swap")
        let list = try XCTUnwrap(p.navigation.shownScroll(try node("content")))
        let canvas = try XCTUnwrap(view.backgroundColor)
        XCTAssertGreaterThan(box(swap).maxY, keys, "the fixture's Swap button starts where the keys will be")

        // The bar: ± and Done, the keyboard's own glass keys, and no fill
        // across it, opaque or not, over what is behind it.
        let bar = try XCTUnwrap(from.field?.inputAccessoryView as? PadBar, "± and Done in the accessory bar")
        bar.frame.size.width = window.bounds.width
        bar.layoutIfNeeded()
        func paints(_ v: UIView) -> Bool {
            if v.frame.width >= bar.bounds.width - 1, let c = v.backgroundColor, c.resolvedColor(with: v.traitCollection).cgColor.alpha > 0.99 { return true }
            return v.subviews.contains(where: paints)
        }
        XCTAssertFalse(paints(bar), "no opaque band across the bar")

        // Focused, as UIKit tells a field that becomes first responder, and
        // the keys come up, the bar counted in their frame.
        from.textFieldDidBeginEditing(try XCTUnwrap(from.field))
        setKeyboard(p, top: keys)
        let nav = try XCTUnwrap(p.navigation.primaryNavigation)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        waitFor("content draws under the accessory while its fields clear it") {
            abs(route.node.bounds.height - self.keys) < 0.5
                && abs(box(list).maxY - self.keys - bar.bounds.height) < 0.5
                && list.adjustedContentInset.bottom >= bar.bounds.height
                && [from, swap].allSatisfy { box($0).maxY <= self.keys + 0.5 && box($0).minY >= box(nav.navigationBar).maxY - 0.5 }
        }
        XCTAssertEqual(box(p.viewport), window.bounds, "the native container remains full-size under the accessory")
        XCTAssertEqual(box(list).maxY, keys + bar.bounds.height, accuracy: 0.5, "the edge scroller draws under the glass keys")
        XCTAssertGreaterThanOrEqual(list.adjustedContentInset.bottom, bar.bounds.height, "its content clears the accessory")
        XCTAssertNil(route.contentViewport, "the screen uses its authored scroller directly")
        var ancestor = list.superview
        while let view = ancestor {
            if view.clipsToBounds {
                XCTAssertGreaterThanOrEqual(box(view).maxY, keys + bar.bounds.height - 0.5, "the accessory's drawing band survives ancestor clipping")
            }
            ancestor = view.superview
        }
        for v in [from, swap] {
            XCTAssertLessThanOrEqual(box(v).maxY, keys + 0.5, "\(v.props["testId"] ?? "") above the bar: \(box(v))")
            XCTAssertGreaterThanOrEqual(box(v).minY, box(nav.navigationBar).maxY - 0.5, "\(v.props["testId"] ?? "") below the navigation bar: \(box(v))")
        }
        // The background continues below the scroller, where the keys begin.
        let behind = try XCTUnwrap(view.backgroundColor).resolvedColor(with: view.traitCollection)
        XCTAssertEqual(behind, try XCTUnwrap(list.backgroundColor).resolvedColor(with: view.traitCollection), "the list's background behind the bar")
        XCTAssertNotEqual(behind, canvas.resolvedColor(with: view.traitCollection), "not the canvas's")

        // The keys gone, the canvas again.
        from.field?.resignFirstResponder()
        setKeyboard(p, top: nil)
        waitFor("the drawing band and its background leave with the keyboard") {
            p.keyboardTop == nil && p.views.values.first { $0.props["testId"] == "content" }?.pageBand.bottom == 0
                && view.backgroundColor?.resolvedColor(with: view.traitCollection) == canvas.resolvedColor(with: view.traitCollection)
        }
        XCTAssertEqual(view.backgroundColor?.resolvedColor(with: view.traitCollection), canvas.resolvedColor(with: view.traitCollection))
        XCTAssertEqual(try node("content").pageBand.bottom, 0, "the accessory inset leaves with the keyboard")
    }

    func testAOneTimeCodeGetsNoBarAndABarTakesTheFieldsTint() throws {
        // Lexy's verification code: the code's suggestion sits over the pad
        // and the screen has its own Verify button.
        let (_, code, window) = field(["inputMode": "numeric", "autocomplete": "one-time-code", "enterKeyHint": "done"], handlers: ["input", "submit"])
        defer { window.isHidden = true }
        XCTAssertEqual(code.field?.keyboardType, .numberPad)
        XCTAssertEqual(code.field?.textContentType, .oneTimeCode)
        XCTAssertNil(code.field?.inputAccessoryView, "no Done bar over a one-time code's pad")
        // Another pad's bar shows in the keyboard's window, whose tint is
        // the system's: it takes the field's, here the window's red.
        let (_, amount, other) = field(["inputMode": "decimal"])
        defer { other.isHidden = true }
        other.tintColor = .systemRed
        XCTAssertTrue(amount.textFieldShouldBeginEditing(try XCTUnwrap(amount.field)))
        let bar = try XCTUnwrap(amount.field?.inputAccessoryView as? UIToolbar)
        XCTAssertEqual(bar.tintColor, .systemRed, "the bar takes the field's tint")
        // A tint that changes once the bar is made follows into it
        other.tintColor = .systemGreen
        XCTAssertEqual(bar.tintColor, .systemGreen, "the bar follows the tint")
    }

    func testDoneIsOnEveryPadAndDoesWhatReturnDoes() throws {
        // number-field's converter: a negative min and enterkeyhint="done", ± and Done.
        let (p, node, window) = field(["inputMode": "decimal", "min": "-273.15", "enterKeyHint": "done", "value": "-40"], handlers: ["input", "change"])
        defer { window.isHidden = true }
        XCTAssertEqual(keys(node), ["toggleSign", "padDone"])
        var committed: [String] = []
        p.onChange = { _, value in committed.append(value) }
        let field = try XCTUnwrap(node.field)
        XCTAssertTrue(field.becomeFirstResponder())
        node.toggleSign() // an edit, as a keystroke's
        node.padDone()
        XCTAssertEqual(committed, ["40"], "Done commits the edit, as Return does")
        XCTAssertFalse(field.isFirstResponder, "and puts the keyboard away")
        // Done alone, without a sign, and with neither hint nor submit
        // handler (expense-tracker's amount): the pad still has a way away.
        node.applyProps(set: [:], clear: ["min", "enterKeyHint"])
        XCTAssertEqual(keys(node), ["padDone"], "no hint and no submit: Done all the same")
        XCTAssertTrue(field.becomeFirstResponder())
        node.padDone()
        XCTAssertFalse(field.isFirstResponder, "and it puts the keyboard away")
        // With a submit handler, Done submits as Return does.
        node.handlers = ["input", "submit"]
        var submitted = 0
        p.onSubmit = { _ in submitted += 1 }
        XCTAssertTrue(node.textFieldShouldBeginEditing(field), "the bar is made as editing begins")
        XCTAssertEqual(keys(node), ["padDone"])
        node.padDone()
        XCTAssertEqual(submitted, 1, "and Done submits")
    }
}
#endif
