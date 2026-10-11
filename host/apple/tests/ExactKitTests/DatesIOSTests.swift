#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// HTML's empty date field on iOS (bench t7-wizard, 2026-10-08): a
/// `UIDatePicker` always holds a date, so while the bound value is "" it
/// shows the format as a placeholder over its faded content, as the Mac's
/// `DateField` and Chrome do, and still takes the tap that opens it.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class DatesIOSTests: XCTestCase {
    func testAnEmptyDateShowsThePlaceholderNotToday() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        defer { window.isHidden = true }
        let picker = try XCTUnwrap(p.controls.makeDate("date") as? BoundDatePicker)
        picker.locale = Locale(identifier: "en_US")
        picker.frame = CGRect(x: 20, y: 20, width: 160, height: 34)
        window.addSubview(picker)
        window.makeKeyAndVisible()
        picker.layoutIfNeeded()
        let fitted = picker.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize)
        picker.empty = true
        picker.layoutIfNeeded()
        let shown = try XCTUnwrap(picker.placeholderShown, "an empty picker shows a placeholder")
        XCTAssertEqual(shown.text, "mm/dd/yyyy", "the locale's field order, as Chrome writes an empty one")
        XCTAssertGreaterThan(shown.frame.width, 0, "the pill has a size")
        XCTAssertGreaterThan(shown.frame.height, 0)
        XCTAssertEqual(picker.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize), fitted,
                       "the placeholder takes no part in the picker's measured size")
        XCTAssertEqual(picker.accessibilityValue, "", "VoiceOver hears no date")
        for view in picker.subviews {
            XCTAssertGreaterThanOrEqual(view.alpha, 0.01, "faded, but above UIKit's hit-test floor, so a tap opens it")
            XCTAssertLessThan(view.alpha, 0.1, "today's date is not readable")
        }
        XCTAssertEqual(DateValue.shown("date", picker), "", "the value it reports is HTML's empty one")
        picker.empty = false
        picker.layoutIfNeeded()
        XCTAssertNil(picker.placeholderShown, "a value takes the placeholder away")
        XCTAssertTrue(picker.subviews.allSatisfy { $0.alpha == 1 }, "and shows the picker's own content")
        XCTAssertEqual(DateValue.placeholder("time", Locale(identifier: "en_US")), "--:-- --")
    }
}
#endif
