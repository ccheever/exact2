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
        // A list cell is a button whatever it opens, as UIKit's is.
        XCTAssertTrue(milk.accessibilityTraits.contains(.button))
        XCTAssertFalse(milk.accessibilityTraits.contains(.link))
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

    func testTheDetailButtonSurvivesAccessibilityAndRowUpdates() throws {
        var detail = Row(view: 12, title: "Information", accessory: "detail", target: 13)
        let p = presenter {
            var m = self.model()
            m.sections[0].rows[2] = detail
            return m
        }
        defer { window.isHidden = true }
        let row = try cell(p, 12)
        let list = try list(p)
        let button = try XCTUnwrap(list.accessory(12) as? UIButton)
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }

        p.apply(wireBatch([["op": "props", "id": 12, "set": [
            "accessibilityLabel": "Read information", "accessibilitySelected": "true"
        ]]]))
        XCTAssertTrue(try cell(p, 12) === row)
        XCTAssertTrue(list.accessory(12) === button)
        XCTAssertEqual(row.accessibilityLabel, "Read information")
        XCTAssertTrue(row.accessibilityTraits.contains(.selected))
        _ = button.accessibilityLabel
        _ = button.accessibilityTraits
        button.isHighlighted = true
        row.updateConfiguration(using: row.configurationState)
        row.setNeedsLayout()
        row.layoutIfNeeded()
        XCTAssertTrue(list.accessory(12) === button)
        XCTAssertNotNil(button.window, "the contact's button remains in the window through the refresh")
        XCTAssertTrue(button.isHighlighted)
        button.isHighlighted = false

        detail.title = "Updated information"
        p.apply(wireBatch([["op": "props", "id": 12, "set": ["accessibilitySelected": "false"],
                           "clear": ["accessibilityLabel"]]]))
        XCTAssertTrue(try cell(p, 12) === row)
        XCTAssertTrue(list.accessory(12) === button)
        XCTAssertEqual(row.accessibilityLabel, "Updated information")
        XCTAssertFalse(row.accessibilityTraits.contains(.selected))
        button.sendActions(for: .touchUpInside)
        XCTAssertEqual(presses, [13])
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

    /// UIKit's accessibility reads a list content view's hidden labels: a
    /// cell whose row drops its value must hold no label with the old one.
    func testACellWhoseRowDropsItsValueKeepsNoLabelOfIt() throws {
        var row = Row(view: 11, title: "Font", secondary: "Georgia", accessory: "disclosure", pressable: true)
        let p = presenter {
            var m = self.model()
            m.sections[0].rows[1] = row
            return m
        }
        defer { window.isHidden = true }
        XCTAssertTrue(labelText(in: try cell(p, 11)).contains("Georgia"))
        row.title = "Privacy Policy"
        row.secondary = nil
        p.apply(wireBatch([["op": "props", "id": 11, "set": ["testId": "privacy"]]]))
        let after = try cell(p, 11)
        after.layoutIfNeeded()
        XCTAssertEqual(labelText(in: after), ["Privacy Policy"])
    }

    /// Every label's text in a view, hidden ones too.
    private func labelText(in view: UIView) -> [String] {
        view.subviews.flatMap { ($0 as? UILabel).map { [$0.text ?? ""] } ?? labelText(in: $0) }.filter { !$0.isEmpty }
    }
}
#endif
