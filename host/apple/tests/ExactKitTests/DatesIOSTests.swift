#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// HTML's empty date field on iOS (bench t7-wizard, 2026-10-08; LLP
/// 1115.000 D13.1): a `UIDatePicker` always holds a date, so while the bound
/// value is "" its content is faded under the platform's pill, which holds
/// the author's `placeholder` and, with none, no words: never today's date,
/// and never a browser's `mm/dd/yyyy`. It still takes the tap that opens it.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class DatesIOSTests: XCTestCase {
    func testAnEmptyDateShowsItsAuthorsPlaceholderOrNoWordsNeverToday() throws {
        let p = Presenter()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        defer { window.isHidden = true }
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        // Two empty dates as a batch gives them: one with a placeholder.
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["id": "page"]],
            ["op": "create", "id": 2, "kind": "control", "props": ["type": "date", "value": ""], "handlers": ["change"], "style": [:]],
            ["op": "create", "id": 3, "kind": "control", "props": ["type": "date", "value": "", "placeholder": "Add a date"], "handlers": ["change"], "style": [:]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 20.0, "y": 20.0, "w": 160.0, "h": 34.0],
            ["op": "frame", "id": 3, "x": 20.0, "y": 80.0, "w": 160.0, "h": 34.0],
        ]))
        let bare = try XCTUnwrap(p.controls.controls[2] as? BoundDatePicker), said = try XCTUnwrap(p.controls.controls[3] as? BoundDatePicker)
        bare.layoutIfNeeded(); said.layoutIfNeeded()
        XCTAssertEqual(try XCTUnwrap(bare.placeholderShown, "an empty picker shows its pill").text, "", "with no placeholder, no words: not a date format")
        // No words at all: across the pill's middle every pixel is the
        // pill's fill, with nothing of today's date under it.
        let pill = try XCTUnwrap(bare.placeholderShown).frame
        let format = UIGraphicsImageRendererFormat(); format.scale = 1; format.opaque = true
        let drawn = UIGraphicsImageRenderer(bounds: bare.bounds, format: format).image { context in
            UIColor.white.setFill(); context.fill(bare.bounds)
            bare.layer.render(in: context.cgContext)
        }
        let cg = try XCTUnwrap(drawn.cgImage), w = cg.width, h = cg.height
        var data = [UInt8](repeating: 0, count: w * h * 4)
        let bitmap = try XCTUnwrap(CGContext(data: &data, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpaceCreateDeviceRGB(),
                                             bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        bitmap.draw(cg, in: CGRect(x: 0, y: 0, width: w, height: h))
        var seen = Set<[UInt8]>()
        for y in [Int(pill.midY) - 3, Int(pill.midY), Int(pill.midY) + 3] where y >= 0 && y < h {
            for x in Int(pill.minX + 10)..<Int(pill.maxX - 10) where x >= 0 && x < w { seen.insert(Array(data[(y * w + x) * 4..<(y * w + x) * 4 + 3])) }
        }
        XCTAssertEqual(seen.count, 1, "one colour across the pill: \(seen)")
        XCTAssertNotEqual(seen.first, [255, 255, 255], "the pill's fill, not the page")
        XCTAssertEqual(try XCTUnwrap(said.placeholderShown).text, "Add a date", "the author's placeholder, in the platform's pill")
        // The words follow the prop, and go with it.
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["placeholder": "No date"], "clear": []]]))
        said.layoutIfNeeded()
        XCTAssertEqual(said.placeholderShown?.text, "No date")
        p.apply(wireBatch([["op": "props", "id": 3, "set": [:], "clear": ["placeholder"]]]))
        said.layoutIfNeeded()
        XCTAssertEqual(said.placeholderShown?.text, "")

        let picker = try XCTUnwrap(p.controls.makeDate("date") as? BoundDatePicker)
        picker.locale = Locale(identifier: "en_US")
        picker.frame = CGRect(x: 20, y: 140, width: 160, height: 34)
        window.addSubview(picker)
        picker.layoutIfNeeded()
        let fitted = picker.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize)
        picker.empty = true
        picker.layoutIfNeeded()
        let shown = try XCTUnwrap(picker.placeholderShown, "an empty picker shows its pill")
        XCTAssertEqual(shown.text, "", "no words of Exact's own")
        XCTAssertGreaterThan(shown.frame.width, 0, "the pill has a size")
        XCTAssertGreaterThan(shown.frame.height, 0)
        XCTAssertEqual(picker.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize), fitted,
                       "the placeholder takes no part in the picker's measured size")
        XCTAssertEqual(picker.accessibilityValue, "", "VoiceOver hears no date")
        XCTAssertFalse(picker.subviews.isEmpty)
        for view in picker.subviews {
            XCTAssertNotNil(view.layer.mask, "today's date is masked out whole")
            XCTAssertTrue(view.alpha == 1 && !view.isHidden, "and its content still takes the tap that opens it")
        }
        let middle = CGPoint(x: picker.bounds.midX, y: picker.bounds.midY)
        let hit = try XCTUnwrap(picker.hitTest(middle, with: nil), "a tap on the pill lands in the picker")
        XCTAssertTrue(hit !== picker && hit.isDescendant(of: picker), "on UIKit's own content, which opens the calendar")
        XCTAssertEqual(DateValue.shown("date", picker), "", "the value it reports is HTML's empty one")
        picker.empty = false
        picker.layoutIfNeeded()
        XCTAssertNil(picker.placeholderShown, "a value takes the pill away")
        XCTAssertTrue(picker.subviews.allSatisfy { $0.layer.mask == nil }, "and shows the picker's own content")
    }
}
#endif
