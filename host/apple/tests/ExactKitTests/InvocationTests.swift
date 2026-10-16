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

    /// The table the kernel's resolver reads too
    /// (`kernel/tests/it/fixtures/invocation.tsv`), on a real batch: the two
    /// resolvers cannot drift apart without one of them failing it. A case
    /// that opens a popover is a header's menu button (`HeaderRule`).
    func testAButtonInvokesWhatTheSharedTableSays() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let table = try String(contentsOf: root.appendingPathComponent("kernel/tests/it/fixtures/invocation.tsv"), encoding: .utf8)
        let rows = table.split(separator: "\n", omittingEmptySubsequences: true).filter { !$0.hasPrefix("#") }
            .map { $0.split(separator: "\t", omittingEmptySubsequences: false).map(String.init) }
        func props(_ field: String) -> [String: String] {
            Dictionary(uniqueKeysWithValues: field.split(separator: ";").map {
                let pair = $0.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
                return (String(pair[0]), String(pair[1]))
            })
        }
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view"]]
        var children: [Int] = []
        var paragraphs: [[String: Any]] = []
        for row in rows where row[0] == "element" || row[0] == "paragraph" {
            let id = Int(row[1])!
            if row[0] == "paragraph" {
                ops.append(["op": "create", "id": id, "kind": "text"])
                let runs = rows.filter { $0[0] == "run" && $0[2] == row[1] }.map { run -> [String: Any] in
                    ["id": Int(run[1])!, "parent": id, "paint": true, "props": props(run[3]).merging(["text": "run"]) { $1 }, "style": [:]]
                }
                paragraphs.append(["op": "paragraph", "id": id, "runs": runs])
            } else {
                ops.append(["op": "create", "id": id, "kind": "view", "props": props(row[2])])
            }
            children.append(id)
        }
        ops += [["op": "children", "id": 1, "ids": children], ["op": "roots", "ids": [1]]] + paragraphs
        p.apply(wireBatch(ops))
        for row in rows where row[0] == "rename" {
            p.apply(wireBatch([["op": "props", "id": Int(row[1])!, "set": ["id": row[2]]]]))
        }
        let cases = rows.filter { $0[0] == "case" }
        ops = []
        for (index, row) in cases.enumerated() {
            var set = props(row[2])
            let kind: String
            switch row[1] {
            case "button": kind = "button"
            case "nativeButton": kind = "control"; set["type"] = "button"
            default: kind = "view"
            }
            ops.append(["op": "create", "id": 100 + index, "kind": kind, "props": set])
            children.append(100 + index)
        }
        ops.append(["op": "children", "id": 1, "ids": children])
        p.apply(wireBatch(ops))
        for (index, row) in cases.enumerated() {
            let why = row[5]
            let got = p.invocation(of: try XCTUnwrap(p.views[UInt32(100 + index)]))
            if row[3] == "-" {
                XCTAssertNil(got, why)
            } else {
                let expect = row[3].split(separator: " ")
                XCTAssertEqual(got?.target.id, UInt32(expect[0]), "\(why): target")
                XCTAssertEqual(got.map { "\($0.action)" }, String(expect[1]), "\(why): action")
            }
            XCTAssertEqual(got?.opensPopover ?? false, row[4] == "menu", "\(why): a menu")
            let source = try XCTUnwrap(p.views[UInt32(100 + index)])
            XCTAssertEqual(HeaderRule.facts(source, back: nil) & 16 != 0, row[4] == "menu", "\(why): a header's menu button")
        }
    }

    /// A `commandfor` naming a run of a paragraph names an element, which no
    /// command acts on: nothing falls back to `popovertarget`.
    func testACommandforNamingARunSuppressesTheFallback() {
        let a = node(10, "view", ["id": "a", "popover": "auto"])
        let source = node(1, "button", ["commandfor": "run", "command": "show-popover", "popovertarget": "a"])
        XCTAssertNil(Invocation(of: source) { name in name == "run" ? .element : name == "a" ? .view(a) : nil })
    }

    /// A row closes a dialog in the mode it shows in: `close` a modal one,
    /// `hide-popover` one shown as a popover, and either way a plain popover
    /// only by hiding.
    func testWhatClosesATargetDependsOnHowItShows() {
        let pop = node(10, "view", ["id": "a", "popover": "auto"])
        let dialog = node(11, "view", ["id": "d", "popover": "auto", "semanticTag": "dialog"])
        let elements = ["a": pop, "d": dialog]
        func closes(_ props: [String: String], _ target: NodeView, modally: Bool) -> Bool {
            Invocation(of: node(1, "button", props)) { elements[$0].map { .view($0) } }?.closes(target, modally: modally) ?? false
        }
        XCTAssertTrue(closes(["commandfor": "d", "command": "close"], dialog, modally: true))
        XCTAssertFalse(closes(["commandfor": "d", "command": "hide-popover"], dialog, modally: true))
        XCTAssertTrue(closes(["popovertarget": "d", "popovertargetaction": "hide"], dialog, modally: false))
        XCTAssertFalse(closes(["commandfor": "d", "command": "close"], dialog, modally: false))
        XCTAssertTrue(closes(["popovertarget": "a", "popovertargetaction": "hide"], pop, modally: true))
        XCTAssertFalse(closes(["popovertarget": "a", "popovertargetaction": "hide"], dialog, modally: false), "another target")
    }

    /// A button is pressable for invoking only when it invokes something; one
    /// whose targets are empty leaves the press to its ancestor.
    func testAButtonIsPressableForInvokingOnlyWhenItInvokesSomething() throws {
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "props": ["popovertarget": "a"]],
            ["op": "create", "id": 3, "kind": "button", "props": ["popovertarget": "", "commandfor": ""]],
            ["op": "create", "id": 4, "kind": "button", "props": ["commandfor": "a", "command": "hide-popover"]],
            ["op": "create", "id": 5, "kind": "view", "props": ["id": "a", "popover": "auto"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5]], ["op": "roots", "ids": [1]],
        ]))
        XCTAssertTrue(try XCTUnwrap(p.views[2]).pressable, "it opens a popover")
        XCTAssertFalse(try XCTUnwrap(p.views[3]).pressable, "it names nothing")
        XCTAssertTrue(try XCTUnwrap(p.views[4]).pressable, "a command-only hide is an invoker too")
    }
}
