// @ref LLP 1121 stage 9 B (D10) — on iOS a page symbol nobody tinted whose
// `color` its author wrote, on it or on an ancestor (the kernel's
// `symbolColorWritten`), draws in that colour; under no written colour it
// keeps UIKit's inherited tint, the accent, and a written tint still wins.
#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class SymbolColorIOSTests: XCTestCase {
    private var window: UIWindow!
    override func tearDown() { window?.isHidden = true; window = nil }

    private let red: [Double] = [232, 28, 36, 255]
    private let tint: [String: Any] = ["sys": "@tint", "c": [[0, 136, 255, 255], [0, 145, 255, 255]]]
    private let teal: [Any] = [[48, 176, 199, 255], [64, 200, 224, 255]]
    private let pink: [Any] = [[173, 20, 87, 255], [244, 143, 177, 255]]

    /// A root whose accent is red, holding star symbols.
    private func presenter(_ images: [(Int, Bool, [String: Any])]) -> Presenter {
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 300, height: 300))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view", "props": [:], "handlers": [], "style": ["accent_color": red]],
                                    ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0]]
        for (id, written, style) in images {
            var props: [String: Any] = ["imageSource": "symbol:star", "symbolName": "star"]
            if written { props["symbolColorWritten"] = "1" }
            ops.append(["op": "create", "id": id, "kind": "image", "props": props, "handlers": [], "style": ["font_size": 17].merging(style) { $1 }])
            ops.append(["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0])
        }
        ops += [["op": "children", "id": 1, "ids": images.map { $0.0 }], ["op": "roots", "ids": [1]]]
        p.apply(wireBatch(ops))
        return p
    }
    private func ink(_ p: Presenter, _ id: UInt32) throws -> [Double] {
        let color = try XCTUnwrap(p.views[id]?.symbolView?.tintColor)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        _ = color.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)).getRed(&r, green: &g, blue: &b, alpha: &a)
        return [r, g, b, a].map { (Double($0) * 255).rounded() }
    }

    func testAWrittenColourIsTheSymbolsInk() throws {
        let p = presenter([(2, true, ["text_color": tint]), (3, true, ["text_color": pink]), (4, false, ["text_color": pink]),
                           (5, true, ["text_color": pink, "tint_color": teal])])
        XCTAssertNil(try XCTUnwrap(p.views[2]).symbolInk, "the accent is UIKit's live tint, inherited")
        XCTAssertEqual(try ink(p, 2), red, "`color=\"AccentColor\"`: the accent")
        XCTAssertEqual(try ink(p, 3), [173, 20, 87, 255], "a token's colour")
        XCTAssertEqual(try ink(p, 4), red, "a colour nobody wrote keeps the accent")
        XCTAssertEqual(try ink(p, 5), [48, 176, 199, 255], "a written tint wins")
        p.apply(wireBatch([["op": "props", "id": 4, "set": ["symbolColorWritten": "1"], "clear": []]]))
        XCTAssertEqual(try ink(p, 4), [173, 20, 87, 255], "written later")
        p.apply(wireBatch([["op": "props", "id": 4, "set": [:], "clear": ["symbolColorWritten"]]]))
        XCTAssertEqual(try ink(p, 4), red, "and unwritten again")
    }
}
#endif
