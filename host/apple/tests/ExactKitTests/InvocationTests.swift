import XCTest
@testable import ExactKit

/// What a button invokes, as HTML's button activation resolves it
/// (`Invocation`), on both Apple hosts.
final class InvocationTests: XCTestCase {
    private let p = Presenter()

    private func node(_ id: UInt32, _ kind: String, _ props: [String: String]) -> NodeView {
        let v = NodeView(id: id, kind: kind, presenter: p)
        v.props = props
        return v
    }

    func testCommandforThatNamesAnElementGovernsElsePopovertargetActs() {
        let a = node(10, "view", ["id": "a", "popover": "auto"])
        let b = node(11, "view", ["id": "b", "popover": "auto"])
        let plain = node(12, "view", ["id": "plain"])
        let dialog = node(13, "view", ["id": "d", "semanticTag": "dialog"])
        let elements = ["a": a, "b": b, "plain": plain, "d": dialog]
        func resolve(_ props: [String: String], kind: String = "button") -> (NodeView, Invocation.Action)? {
            Invocation(of: node(1, kind, props)) { name in
                name == "run" ? .element : elements[name].map { .view($0) }
            }.map { ($0.target, $0.action) }
        }
        func check(_ props: [String: String], _ expected: (NodeView, Invocation.Action)?, _ why: String, kind: String = "button") {
            let got = resolve(props, kind: kind)
            XCTAssertTrue(got?.0 === expected?.0, "\(why): target")
            XCTAssertEqual(got?.1, expected?.1, "\(why): action")
        }
        check(["commandfor": "b", "command": "show-popover", "popovertarget": "a"], (b, .show), "a resolved commandfor wins")
        check(["commandfor": "missing", "command": "show-popover", "popovertarget": "a"], (a, .toggle), "a missing one falls back")
        check(["commandfor": "", "command": "toggle-popover", "popovertarget": "a", "popovertargetaction": "show"], (a, .show),
              "an empty one falls back, and its leftover command is ignored")
        check(["commandfor": "plain", "command": "show-popover", "popovertarget": "a"], nil, "one naming a non-popover: no fallback")
        check(["commandfor": "run", "command": "show-popover", "popovertarget": "a"], nil, "one naming a paragraph's run: no fallback")
        check(["commandfor": "b", "command": "bogus", "popovertarget": "a"], nil, "an unknown command: no fallback")
        check(["commandfor": "b", "popovertarget": "a"], nil, "a missing command: no fallback")
        check(["commandfor": "b", "command": "HIDE-POPOVER"], (b, .hide), "keywords are ASCII case-insensitive")
        check(["commandfor": "b", "command": " hide-popover "], nil, "and not trimmed")
        check(["commandfor": "d", "command": "show-modal"], (dialog, .showModal), "show-modal on a dialog")
        check(["commandfor": "d", "command": "close"], (dialog, .close), "close on a dialog")
        check(["commandfor": "b", "command": "show-modal"], nil, "show-modal on a popover that is no dialog")
        check(["popovertarget": "d"], nil, "popovertarget naming a dialog without popover")
        check(["popovertarget": "a", "popovertargetaction": "HIDE"], (a, .hide), "popovertargetaction is case-insensitive")
        check(["popovertarget": "a", "popovertargetaction": " hide "], (a, .toggle), "an invalid action toggles")
        check(["popovertarget": "a", "popovertargetaction": ""], (a, .toggle), "an empty action toggles")
        check(["popovertarget": "a"], nil, "a view is no button", kind: "view")
    }
}
