#if os(iOS) || os(macOS)
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists
#if os(iOS)
import UIKit
#else
import AppKit
#endif

/// Issue #428: a sheet inserts an offscreen custom row. The driver's reveal
/// must realize the row before its ordinary tap can reach the nested button.
final class GroupedRevealTests: XCTestCase {
    func testRevealReachesACustomButtonInsertedByASheet() throws {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_GROUPED_REVEAL_PLAN"])
        #if os(macOS)
        _ = NSApplication.shared
        clicksArriveAsInTheKeyWindow()
        #endif
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "grouped-reveal-sheet")
        defer { session.destroy() }
        let view = ExactView(session: session)
        #if os(iOS)
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else {
            throw missing("scene", "the sheet needs a hosted window scene")
        }
        let window = UIWindow(windowScene: scene)
        window.frame = window.screen.bounds
        let root = UIViewController()
        window.rootViewController = root
        window.makeKeyAndVisible()
        root.view.addSubview(view)
        view.frame = root.view.bounds
        defer { window.isHidden = true }
        #else
        let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 402, height: 874),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = view
        window.orderFrontRegardless()
        defer { window.close() }
        #endif
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: view.bounds.size).error)
        let p = session.presenter, agent = Agent(session: session)
        func settle() {
            window.layoutIfNeeded()
            let reply = agent.clock(["settle": true])
            XCTAssertEqual(reply["settled"] as? Bool, true, "\(reply)")
        }
        func node(_ name: String) throws -> NodeView {
            try XCTUnwrap(p.views.values.first { $0.props["testId"] == name }, name)
        }
        func tap(_ name: String) throws {
            let id = Int(try node(name).id)
            let reveal = agent.reveal(["id": id])
            XCTAssertNil(reveal["error"], "\(reveal)")
            let reply = agent.tap(["id": id])
            XCTAssertNil(reply["error"], "\(reply)")
            settle()
        }
        func removed() throws -> Bool? {
            let state = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"state"}"#).utf8)) as? [String: Any])
            return (state["slots"] as? [String: Any])?["removed"] as? Bool
        }
        settle()
        try tap("add")
        try tap("save")
        XCTAssertNil(agent.clock(["data": true])["error"])
        settle()
        XCTAssertEqual(try removed(), false)
        #if os(iOS)
        let target = try node("remove")
        let collection = try XCTUnwrap(p.groupedLists?.scroller(for: target.id) as? UICollectionView)
        XCTAssertFalse(target.isDescendant(of: collection), "the new custom row has no realized cell yet")
        let before = collection.contentOffset.y
        #endif
        try tap("remove")
        XCTAssertEqual(try removed(), true, "the driver pressed the button after revealing its row")
        #if os(iOS)
        XCTAssertGreaterThan(collection.contentOffset.y, before)
        XCTAssertTrue(target.isDescendant(of: collection), "the button is carried into its realized cell")
        let middle = target.convert(CGPoint(x: target.bounds.midX, y: target.bounds.midY), to: collection)
        XCTAssertTrue(agent.visiblePort(collection).contains(middle))
        #endif
    }
}
#endif
