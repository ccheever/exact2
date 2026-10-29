// Chrome's position-area table (host/web/tests/position-area-region.test.mjs),
// which PositionAreaRegionMacTests and PositionAreaRegionIOSTests hold the
// painted popover to, and its mirror image right to left.
import CoreGraphics

enum PositionAreaRegionTable {
    /// [invoker x, invoker y, position-area, position-try-fallbacks, x, y, width, height], Chrome's.
    static let rows: [(CGFloat, CGFloat, String, String, CGRect)] = [
        (0, 100, "bottom span-right", "none", CGRect(x: 0, y: 120, width: 400, height: 36)),
        (150, 100, "bottom span-right", "none", CGRect(x: 150, y: 120, width: 250, height: 54)),
        (150, 100, "bottom span-right", "flip-inline", CGRect(x: 150, y: 120, width: 250, height: 54)),
        (300, 100, "bottom span-right", "none", CGRect(x: 300, y: 120, width: 100, height: 108)),
        (300, 100, "bottom span-right", "flip-inline", CGRect(x: 300, y: 120, width: 100, height: 108)),
        (380, 100, "bottom span-right", "none", CGRect(x: 370, y: 120, width: 50, height: 216)),
        (380, 100, "bottom span-right", "flip-inline", CGRect(x: 0, y: 120, width: 420, height: 36)),
        (150, 100, "bottom", "none", CGRect(x: 0, y: 120, width: 400, height: 36)),
        (150, 100, "center", "none", CGRect(x: 145, y: 2, width: 50, height: 216)),
        (10, 100, "center", "none", CGRect(x: 5, y: 2, width: 50, height: 216)),
        (150, 550, "bottom span-right", "flip-block", CGRect(x: 150, y: 496, width: 250, height: 54)),
        (150, 100, "right", "none", CGRect(x: 190, y: 83, width: 210, height: 54)),
        (150, 100, "left", "none", CGRect(x: 0, y: 74, width: 150, height: 72)),
        (150, 100, "top span-left", "none", CGRect(x: 0, y: 28, width: 190, height: 72)),
        (300, 100, "bottom span-left", "none", CGRect(x: 0, y: 120, width: 340, height: 36)),
        (20, 100, "bottom span-left", "flip-inline", CGRect(x: 0, y: 120, width: 60, height: 216)),
        // No option fits: shifted inside the window, as CSS Box Alignment's overflow alignment does.
        (150, 550, "bottom span-right", "none", CGRect(x: 150, y: 546, width: 250, height: 54)),
        // Six boxes: laid out once in the viewport the base would overflow its region and
        // the flip fit, and stick; measured in its region the base fits, and Chrome keeps it.
        (300, 100, "bottom span-right", "flip-inline", CGRect(x: 300, y: 120, width: 100, height: 54)),
        (300, 560, "bottom span-right", "flip-block", CGRect(x: 300, y: 506, width: 100, height: 54)),
        (60, 100, "bottom span-left", "flip-inline", CGRect(x: 0, y: 120, width: 100, height: 54)),
    ]
    // (The last three rows' popovers hold six boxes, the fixtures say.)
    /// A row in a right-to-left document, as Chrome mirrors it (checked there).
    static func mirrored(_ row: (CGFloat, CGFloat, String, String, CGRect)) -> (CGFloat, CGFloat, String, String, CGRect) {
        let area = row.2.split(separator: " ").map { $0.replacingOccurrences(of: "left", with: "L").replacingOccurrences(of: "right", with: "left").replacingOccurrences(of: "L", with: "right") }.joined(separator: " ")
        let b = row.4
        return (400 - row.0 - 40, row.1, area, row.3, CGRect(x: 400 - b.maxX, y: b.minY, width: b.width, height: b.height))
    }
}
