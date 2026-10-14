#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// LLP 1095 D4's reach on UIKit: the roles the app's theme replaces, sent
/// to the presenter at boot (`{"op": "roles"}`), colour the surfaces UIKit
/// draws itself. A simulator runs it: bun host/apple/build.mjs --test --ios
final class ThemeRolesIOSTests: GroupedListFixture {
    /// LLP 1095 D4's reach: a theme that replaces the grouped roles colours
    /// UIKit's own list (its ground, the cards, the rows' text) where it
    /// would read the role; without one the platform's colours stay.
    func testAThemeThatReplacesTheGroupedRolesColoursTheNativeList() throws {
        defer { ThemeRoles.apply(wireBatch([["op": "roles", "roles": [String: Any]()]])) }
        let platform = presenter { self.model() }
        let before = try cell(platform, 10)
        XCTAssertNotEqual(before.backgroundConfiguration?.backgroundColor?.resolvedColor(with: UITraitCollection(userInterfaceStyle: .light)), UIColor(red: 10 / 255, green: 20 / 255, blue: 30 / 255, alpha: 1))
        ThemeRoles.apply(wireBatch([["op": "roles", "roles": [
            "grouped-background": [[1, 2, 3, 255], [4, 5, 6, 255]],
            "secondary-grouped-background": [10, 20, 30, 255],
            "label": [200, 0, 0, 255],
        ]]]))
        let themed = presenter { self.model() }
        let card = try cell(themed, 10)
        let light = UITraitCollection(userInterfaceStyle: .light), dark = UITraitCollection(userInterfaceStyle: .dark)
        XCTAssertEqual(card.backgroundConfiguration?.backgroundColor?.resolvedColor(with: light), UIColor(red: 10 / 255, green: 20 / 255, blue: 30 / 255, alpha: 1))
        let text = try XCTUnwrap(card.contentConfiguration as? UIListContentConfiguration)
        XCTAssertEqual(text.textProperties.color.resolvedColor(with: light), UIColor(red: 200 / 255, green: 0, blue: 0, alpha: 1))
        let ground = try XCTUnwrap(ThemeRoles.color("grouped-background"))
        XCTAssertEqual(ground.resolvedColor(with: dark), UIColor(red: 4 / 255, green: 5 / 255, blue: 6 / 255, alpha: 1))
        XCTAssertNil(ThemeRoles.color("separator"), "a role the theme leaves stays the platform's")
    }
}
#endif
