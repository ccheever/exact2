import Foundation
import XCTest
@testable import ExactKit

/// LLP 1071 §8.1: the kernel measures on the owner thread while main paints
/// the same paragraphs. The two engines of a session share nothing but the
/// published breaks, so this runs under the Thread Sanitizer clean, and what
/// each lays out agrees. Before the split, one engine's residency was shaped
/// from both threads at once and a scrolling list crashed within minutes.
/// Both platforms; the iOS simulator runs it as an `IOSTests` class.
final class OwnerTextIOSTests: XCTestCase {
    private func spec(_ i: Int, clamp: Int = 0) -> Spec {
        let run = Run(text: "Paragraph \(i): " + String(repeating: "several words wrap here ", count: 8 + i % 5),
                      size: 15 + CGFloat(i % 3), weight: i % 2 == 0 ? 400 : 600, family: 0, italic: false,
                      lineHeight: nil, letterSpacing: 0)
        return Spec(runs: [run], align: 0, lineClamp: clamp, color: [0, 0, 0, 255], strut: run)
    }

    func testTheOwnerMeasuresWhileMainPaintsTheSameText() {
        let painter = TextEngine.pair(resolve: { _ in nil })
        let measurer = painter.measuring
        XCTAssertFalse(painter === measurer)
        let specs = (0..<48).map { spec($0, clamp: $0 % 7 == 0 ? 2 : 0) }
        let widths: [CGFloat] = [180, 240, 320]
        let done = expectation(description: "the owner measured")
        var measured: [Int: Int] = [:]
        Owner.shared.post {
            for round in 0..<30 {
                for (i, s) in specs.enumerated() {
                    let p = measurer.paragraph(s, width: widths[(i + round) % widths.count])
                    if round == 0, (i + round) % widths.count == 0 { measured[i] = p.lines.count }
                }
                if round % 7 == 3 { measurer.dropCold() }
                if round % 11 == 5 { measurer.fitShaped(visibleParagraphs: 8) }
            }
            DispatchQueue.main.async { done.fulfill() }
        }
        var painted: [Int: Int] = [:]
        for round in 0..<30 {
            for (i, s) in specs.enumerated() {
                let width = widths[(i + round) % widths.count]
                let p = painter.paragraph(s, width: width)
                _ = painter.measuredBreaks(s, width: width)
                if round == 0, (i + round) % widths.count == 0 { painted[i] = p.lines.count }
            }
            if round % 5 == 2 { painter.dropCold() }
        }
        wait(for: [done], timeout: 60)
        Owner.shared.sync {}
        XCTAssertFalse(measured.isEmpty)
        XCTAssertEqual(measured, painted, "each engine lays out the same lines")
    }
}
