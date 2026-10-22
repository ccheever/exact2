#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

final class GroupedAccessibilityIOSTests: GroupedListFixture {
    func testAccessibilityOnlyUpdatesRefreshTheVisibleCell() throws {
        let p = presenter { self.model() }
        let milk = try cell(p, 10)
        p.apply(wireBatch([["op": "props", "id": 10, "set": [
            "accessibilityLabel": "Buy milk", "accessibilityRole": "link", "accessibilitySelected": "true"
        ]]]))
        XCTAssertTrue(try cell(p, 10) === milk)
        XCTAssertEqual(milk.accessibilityLabel, "Buy milk")
        XCTAssertTrue(milk.accessibilityTraits.contains(.link))
        XCTAssertFalse(milk.accessibilityTraits.contains(.button))
        XCTAssertTrue(milk.accessibilityTraits.contains(.selected))
        p.apply(wireBatch([["op": "props", "id": 10, "set": [
            "accessibilityRole": "button", "accessibilitySelected": "false", "accessibilityCurrent": "page"
        ]]]))
        XCTAssertTrue(milk.accessibilityTraits.contains(.button))
        XCTAssertFalse(milk.accessibilityTraits.contains(.link))
        XCTAssertTrue(milk.accessibilityTraits.contains(.selected))
        p.apply(wireBatch([["op": "props", "id": 10, "set": [
            "accessibilityLabel": "Milk again", "accessibilityCurrent": "false"
        ]]]))
        XCTAssertEqual(milk.accessibilityLabel, "Milk again")
        XCTAssertFalse(milk.accessibilityTraits.contains(.selected))
    }

    func testAStandardRowsNameAndSelectionComeFromItsParts() throws {
        var row = Row(view: 10, symbol: "checkmark.circle", title: "Milk", secondary: "Two bottles", pressable: true)
        let p = presenter {
            var m = self.model()
            m.sections[0].rows[0] = row
            return m
        }
        let milk = try cell(p, 10)
        XCTAssertEqual(milk.accessibilityLabel, "Milk, Two bottles")
        XCTAssertTrue(milk.accessibilityTraits.contains(.button))
        XCTAssertFalse(milk.accessibilityTraits.contains(.selected))
        row.accessory = "checkmark"
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["testId": "row10"]]]))
        XCTAssertTrue(try cell(p, 10).accessibilityTraits.contains(.selected))
        row.accessory = "none"
        row.secondary = nil
        row.pressable = false
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["accessibilityLabel": "Buy milk"]]]))
        let renamed = try cell(p, 10)
        XCTAssertEqual(renamed.accessibilityLabel, "Buy milk")
        XCTAssertFalse(renamed.accessibilityTraits.contains(.button))
        XCTAssertFalse(renamed.accessibilityTraits.contains(.selected))
    }
}
#endif
