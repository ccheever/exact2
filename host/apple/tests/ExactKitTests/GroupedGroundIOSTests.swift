#if os(iOS)
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists
import UIKit

/// A grouped list's background as its author wrote it is the list's ground on iOS, as it replaces the sheet's on the
/// other hosts (LLP 1084 D7): the list's own layer paints it once, UIKit's grounds clear, so `transparent` shows what
/// lies behind the list around its cards, which keep theirs.
final class GroupedGroundIOSTests: GroupedListFixture {
    private let light = UITraitCollection(userInterfaceStyle: .light), dark = UITraitCollection(userInterfaceStyle: .dark)
    private func rgba(_ color: UIColor?, _ traits: UITraitCollection) -> [CGFloat] {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        (color ?? .clear).resolvedColor(with: traits).getRed(&r, green: &g, blue: &b, alpha: &a)
        return [r, g, b, a]
    }
    /// What UIKit paints under the cards: the collection view's background and each section's.
    private func grounds(_ l: GroupedListView, _ traits: UITraitCollection? = nil) -> [[CGFloat]] {
        l.collection.layoutIfNeeded()
        let sections = l.collection.subviews.filter { NSStringFromClass(type(of: $0)).contains("BackgroundColorDecorationView") }
        XCTAssertFalse(sections.isEmpty, "UIKit draws each section's background in a decoration view")
        return ([l.collection] + sections).map { rgba($0.backgroundColor, traits ?? light) }
    }
    private func write(_ p: Presenter, _ colour: Any?) {
        var style: [String: Any] = ["overflow_y": "scroll"]
        if let colour { style["background_color"] = colour; style["written"] = ["background_color"] }
        p.apply(wireBatch([["op": "style", "id": 1, "style": style]]))
    }

    func testAWrittenBackgroundIsPaintedOnceByTheListAndItsCardsKeepTheirs() throws {
        let p = presenter { self.model() }
        let l = try list(p)
        let system = rgba(.systemGroupedBackground, light)
        XCTAssertTrue(grounds(l).allSatisfy { $0 == system }, "nothing written: UIKit's grouped ground")
        XCTAssertTrue(grounds(l, dark).allSatisfy { $0 == rgba(.systemGroupedBackground, dark) }, "in dark too")
        // Half-transparent red: UIKit's grounds are clear, and the list's own layer paints it, once
        write(p, [255, 0, 0, 128])
        XCTAssertTrue(grounds(l).allSatisfy { $0[3] == 0 }, "UIKit paints none of it: \(grounds(l))")
        l.owner.layer.displayIfNeeded() // the layer paints its box when it displays (`NodeLayer`)
        let own = rgba(l.owner.layer.backgroundColor.map(UIColor.init(cgColor:)), light)
        for (got, want) in zip(own, [1, 0, 0, 128.0 / 255]) { XCTAssertEqual(got, want, accuracy: 0.01, "the list's layer, at the alpha written: \(own)") }
        let card = try cell(p, 10)
        XCTAssertEqual(rgba(card.backgroundConfiguration?.resolvedBackgroundColor(for: card.tintColor), light)[3], 1, "a card keeps its opaque background")
        // Written no longer: UIKit's again
        write(p, nil)
        XCTAssertTrue(grounds(l).allSatisfy { $0 == system }, "\(grounds(l))")
    }

    /// Written from the list's first layout on, the collection view is clear too, which UIKit leaves its default
    /// `systemBackground` for a layout with a ground of its own.
    func testAGroundWrittenFromTheStartClearsTheWholeList() throws {
        let p = presenter(style: ["background_color": [0, 0, 0, 0], "written": ["background_color"]]) { self.model() }
        let all = grounds(try list(p))
        XCTAssertTrue(all.allSatisfy { $0[3] == 0 }, "\(all)")
    }

    /// A plain list's ground is the system background, not the grouped one, the collection view's as the sections'.
    func testAPlainListsGroundIsTheSystemBackground() throws {
        let p = presenter { self.model("plain") }
        let all = grounds(try list(p))
        XCTAssertTrue(all.allSatisfy { $0 == rgba(.systemBackground, light) }, "\(all)")
    }

    /// A theme's page ground (LLP 1095 D4) is a plain list's, its `Canvas` where it sets no `background`, as a page's is.
    func testAThemedPlainListsGroundIsThePagesGround() throws {
        ThemeRoles.apply(wireBatch([["op": "roles", "roles": ["Canvas": [253, 246, 227, 255]]]]))
        defer { ThemeRoles.apply(wireBatch([["op": "roles", "roles": [String: Any]()]])) }
        let p = presenter { self.model("plain") }
        let all = grounds(try list(p))
        XCTAssertTrue(all.allSatisfy { abs($0[0] - 253.0 / 255) < 0.01 && abs($0[2] - 227.0 / 255) < 0.01 }, "\(all)")
    }

    /// A theme's grouped ground (LLP 1095 D4) is the collection view's from the first layout on, as the sections'.
    func testAThemesGroundIsTheWholeListsFromTheFirstLayout() throws {
        ThemeRoles.apply(wireBatch([["op": "roles", "roles": ["grouped-background": [[10, 20, 30, 255], [40, 50, 60, 255]]]]]))
        defer { ThemeRoles.apply(wireBatch([["op": "roles", "roles": [String: Any]()]])) }
        let p = presenter { self.model() }
        let all = grounds(try list(p))
        XCTAssertTrue(all.allSatisfy { abs($0[0] - 10.0 / 255) < 0.01 && abs($0[2] - 30.0 / 255) < 0.01 }, "\(all)")
    }
}
#endif
