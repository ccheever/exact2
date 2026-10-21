import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// A `visibility: visible` inline span inside a hidden paragraph stays in the
/// accessibility tree, and only that span is spoken. The paragraph's source
/// text is unchanged. A fully hidden paragraph stays out of the tree.
final class HiddenInlineAccessibilityIOSTests: XCTestCase {
    #if os(macOS)
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil; super.tearDown() }
    #else
    private var window: UIWindow?
    override func tearDown() { window?.isHidden = true; window = nil; super.tearDown() }
    #endif

    /// `op` first, as the host writes a paragraph, so each run's `visibility` is the streaming decoder's.
    private func wire(_ json: String) -> Batch {
        let batch = Batch.decode(Data(json.utf8))
        precondition(batch.error == nil, batch.error ?? json)
        return batch
    }

    private func presenter() -> Presenter {
        let p = Presenter()
        #if os(macOS)
        _ = NSApplication.shared
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 200)
        let window = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        self.window = window
        #else
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 200))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        self.window = window
        #endif
        return p
    }

    private func label(_ node: NodeView) -> String? {
        #if os(macOS)
        node.accessibilityLabel()
        #else
        node.accessibilityLabel
        #endif
    }

    private func exposed(_ node: NodeView) -> Bool {
        #if os(macOS)
        node.isAccessibilityElement()
        #else
        node.isAccessibilityElement
        #endif
    }

    func testAVisibleInlineSpanOfAHiddenParagraphStaysExposed() throws {
        let p = presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "style": ["font_size": 18]],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Gone"], "style": ["font_size": 18, "visibility": "hidden"]],
            ["op": "roots", "ids": [1, 4]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 50.0, "w": 300.0, "h": 40.0],
        ]))
        p.apply(wire(#"{"ops":[{"op":"paragraph","id":1,"runs":[{"id":2,"parent":1,"paint":true,"props":{"text":"Secret"},"style":{"font_size":18,"visibility":"hidden"}},{"id":3,"parent":1,"paint":true,"props":{"text":"Shown"},"style":{"font_size":18,"visibility":"visible"}}]}]}"#))
        let node = try XCTUnwrap(p.views[1])
        p.apply(wireBatch([["op": "style", "id": 1, "style": ["font_size": 18, "visibility": "hidden"]]]))
        XCTAssertTrue(node.accessibilityVisible, "the visible run stays in the tree")
        XCTAssertTrue(exposed(node))
        XCTAssertEqual(label(node), "Shown")
        XCTAssertFalse(node.accessibleText.contains("Secret"))
        XCTAssertTrue(node.paragraphText.contains("Secret"), "selection still reads the source")
        let gone = try XCTUnwrap(p.views[4])
        XCTAssertFalse(gone.accessibilityVisible, "a fully hidden paragraph stays out")
        XCTAssertFalse(exposed(gone))
    }

    func testHiddenInteractiveRunsAreNotAccessibilityChildren() throws {
        let p = presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "style": ["font_size": 18, "visibility": "hidden"]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
        ]))
        p.apply(wire(#"{"ops":[{"op":"paragraph","id":1,"runs":[{"id":2,"parent":1,"paint":true,"props":{"text":"Secret","href":"/secret"},"style":{"font_size":18,"visibility":"hidden"}},{"id":3,"parent":1,"paint":true,"props":{"text":"Shown","href":"/shown"},"style":{"font_size":18,"visibility":"visible"}}]}]}"#))
        let node = try XCTUnwrap(p.views[1])
        let labels = (node.textAccessibilityChildren() ?? []).compactMap { child -> String? in
            #if os(macOS)
            (child as? NSAccessibilityElement)?.accessibilityLabel()
            #else
            (child as? NSObject)?.accessibilityLabel
            #endif
        }
        XCTAssertTrue(labels.contains("Shown"))
        XCTAssertFalse(labels.contains { $0.contains("Secret") })
    }

    func testInlineLinkARIAUnavailabilityAndAncestorState() throws {
        let p = presenter()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["accessibilityRole": "group", "accessibilityDisabled": "true"]],
            ["op": "create", "id": 2, "kind": "text", "style": ["font_size": 18]],
            ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
        ]))
        func paragraph(_ disabled: String?) {
            let state = disabled.map { ",\"accessibilityDisabled\":\"\($0)\"" } ?? ""
            p.apply(wire(#"{"ops":[{"op":"paragraph","id":2,"runs":[{"id":3,"parent":2,"paint":true,"props":{"text":"Updates","href":"/updates""# + state + #"},"style":{"font_size":18}}]}]}"#))
        }
        func unavailable() throws -> Bool {
            let node = try XCTUnwrap(p.views[2])
            let link = try XCTUnwrap((node.textAccessibilityChildren() ?? []).first { ($0 as? AgentOwned)?.agentViewId == 3 })
            #if os(macOS)
            return (link as? NSAccessibilityElement)?.accessibilityAttributeValue(.enabled) as? Bool == false
            #else
            return (link as? UIAccessibilityElement)?.accessibilityTraits.contains(.notEnabled) == true
            #endif
        }
        paragraph(nil)
        XCTAssertTrue(try unavailable(), "a disabled ancestor reaches the inline link")
        paragraph("false")
        XCTAssertFalse(try unavailable(), "the run's explicit false overrides its ancestor")
        paragraph("true")
        XCTAssertTrue(try unavailable(), "the run's own state reaches its exposed element")
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["accessibilityDisabled": "false"]]]))
        XCTAssertTrue(try unavailable(), "the run's own true survives clearing the ancestor")
        paragraph(nil)
        XCTAssertFalse(try unavailable(), "clearing the run and ancestor restores the link")
    }
}
