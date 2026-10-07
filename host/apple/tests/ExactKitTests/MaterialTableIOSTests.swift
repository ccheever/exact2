#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class MaterialTableIOSTests: XCTestCase {
    func testEveryMaterialIsAUIKitOne() throws {
        try MaterialTable.check(1) { Materials.blurStyle($0) != nil }
    }

    /// Named materials reach their styles; `backdrop-filter` alone draws
    /// `.light` (Charlie, 2026-09-27; LLP 1053.000 §3).
    func testNamedMaterialsAndABareBlurDrawTheirStyles() {
        XCTAssertEqual(Backdrop.material, .light)
        for (name, style) in [("thin", UIBlurEffect.Style.systemThinMaterial), ("chrome-dark", .systemChromeMaterialDark), ("prominent", .prominent)] {
            XCTAssertEqual(Materials.blurStyle(Materials.platform(name)!.apple), style, name)
        }
        XCTAssertEqual(Materials.platform("sidebar")?.standIn, true, "an AppKit material draws a stand-in")
    }

    func testSaturationKeepsTheDeclaredMaterialApproximation() {
        let p = Presenter()
        let node = NodeView(id: 1, kind: "view", presenter: p)
        node.applyStyle(["backdrop_filter": [["saturate": 0]]])
        XCTAssertEqual(node.materialRequest, "backdrop")
        XCTAssertTrue(node.materialView is BackdropEffectView)
        XCTAssertEqual((node.materialView as? BackdropEffectView)?.operations, [.saturate(0)])
        node.applyStyle([:])
        XCTAssertNil(node.materialRequest)
        XCTAssertNil(node.materialView)
    }
}
#endif
