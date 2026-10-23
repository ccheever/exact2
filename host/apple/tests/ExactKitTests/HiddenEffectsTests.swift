import XCTest
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
@testable import ExactKit

final class HiddenEffectsTests: XCTestCase {
    func testHidingAMaterialKeepsItsVisibleDescendantsAndContainer() throws {
        #if canImport(UIKit)
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else { throw missing("scene", "material descendants keep their focused editor") }
        let window = UIWindow(windowScene: scene)
        window.frame = scene.screen.bounds
        let controller = UIViewController()
        window.rootViewController = controller
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        #else
        _ = NSApplication.shared
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 300, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        #endif
        for materialKind in ["ultra-thin", "glass"] {
            let p = Presenter()
            defer { p.viewport.removeFromSuperview() }
            p.viewport.frame = CGRect(x: 0, y: 0, width: 300, height: 200)
            #if canImport(UIKit)
            controller.view.addSubview(p.viewport)
            #else
            window.contentView = p.viewport
            #endif
            p.apply(wireBatch([
                ["op": "create", "id": 1, "kind": "view", "props": ["backgroundMaterial": materialKind, "glassGroup": "8"]],
                ["op": "create", "id": 2, "kind": "input", "props": ["value": "Visible"]],
                ["op": "children", "id": 1, "ids": [2]], ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 150],
                ["op": "frame", "id": 2, "x": 10, "y": 10, "w": 150, "h": 40]
            ]))
            let node = try XCTUnwrap(p.views[1]), child = try XCTUnwrap(p.views[2]), field = try XCTUnwrap(child.field)
            let material = try XCTUnwrap(node.materialView), holder = child.superview
            #if canImport(UIKit)
            XCTAssertTrue(field.becomeFirstResponder())
            #else
            XCTAssertTrue(child.isDescendant(of: material), "visible content belongs to the native effect")
            XCTAssertTrue(window.makeFirstResponder(field))
            #endif
            p.apply(wireBatch([["op": "style", "id": 1, "style": ["visibility": "hidden"]]]))
            XCTAssertTrue(node.materialView === material)
            XCTAssertTrue(child.superview === holder)
            XCTAssertEqual(node.glassGroupConflict, "material")
            #if canImport(UIKit)
            XCTAssertNil(material.effect)
            XCTAssertTrue(field.isFirstResponder)
            #else
            XCTAssertTrue(material.isHidden)
            XCTAssertFalse(field.isHiddenOrHasHiddenAncestor)
            XCTAssertNotNil(field.currentEditor())
            #endif
            p.apply(wireBatch([["op": "style", "id": 1, "style": [:]]]))
            #if canImport(UIKit)
            XCTAssertNotNil(material.effect)
            #else
            XCTAssertFalse(material.isHidden)
            XCTAssertTrue(child.isDescendant(of: material), "restored content regains vibrancy and corner clipping")
            XCTAssertNotNil(field.currentEditor())
            #endif
        }
    }

    func testHiddenBackgroundKeyframesStaySuppressedAcrossClockSeeks() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let p = Presenter()
        let spec: [String: Any] = ["id": "paint", "k": "backgroundColor", "s": 0.0, "dl": 0.0, "d": 1.0, "n": 100.0,
                                  "t": [0.0, 1.0], "v": [[255, 0, 0, 255], [0, 0, 255, 255]], "c": [], "fill": 3.0]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["id": "paint"], "style": ["visibility": "hidden"]],
            ["op": "roots", "ids": [1]], ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 100, "h": 100],
            ["op": "animations", "id": 1, "specs": [spec]]
        ]))
        let node = try XCTUnwrap(p.views[1])
        #if canImport(UIKit)
        let layer = node.layer
        #else
        let layer = try XCTUnwrap(node.layer)
        #endif
        XCTAssertNil(layer.animation(forKey: "paint"))
        p.svg.seek(clock: 500)
        XCTAssertNil(layer.animation(forKey: "paint"))
        p.apply(wireBatch([["op": "style", "id": 1, "style": [:]]]))
        XCTAssertNotNil(layer.animation(forKey: "paint"))
    }
}
