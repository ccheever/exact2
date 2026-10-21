#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// Dynamic Type on the iOS host (LLP 1115 D3; LLP 1116.011 D2.4, round 13's
/// weather at accessibility text sizes). A text role (`-exact-headline`) is
/// the platform's size for the reader's category, read from the schema's
/// ramp at the root font size the host tells the kernel; the host then
/// measures and draws the font the kernel names, so a row grows with its
/// text as UIKit's do. What does not follow is what the author wrote in px
/// (`width=48`): the weather's day label overlapped its icon because its box
/// stayed 48 points while its text became 33 (the author's, LLP 1115 D1).
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class DynamicTypeIOSTests: XCTestCase {
    /// Every category the user can pick, by name: xSmall to AX5.
    private static let categories: [UIContentSizeCategory] = [
        .extraSmall, .small, .medium, .large, .extraLarge, .extraExtraLarge, .extraExtraExtraLarge,
        .accessibilityMedium, .accessibilityLarge, .accessibilityExtraLarge,
        .accessibilityExtraExtraLarge, .accessibilityExtraExtraExtraLarge,
    ]
    private static let uikit: [String: UIFont.TextStyle] = [
        "largeTitle": .largeTitle, "title1": .title1, "title2": .title2, "title3": .title3, "headline": .headline,
        "body": .body, "callout": .callout, "subheadline": .subheadline, "footnote": .footnote,
        "caption1": .caption1, "caption2": .caption2,
    ]

    /// The kernel's ramp (`kernel/tables/schema.json`): the body sizes, and each style's UIKit name and size at each.
    private func ramp() throws -> (bodies: [Double], styles: [(name: String, sizes: [Double])]) {
        let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .appendingPathComponent("../../../../kernel/tables/schema.json").standardized
        let schema = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        let bodies = try XCTUnwrap(schema["textStyleBodies"] as? [Double])
        let rows = try XCTUnwrap(schema["textStyles"] as? [[Any]])
        return (bodies, try rows.map { row in
            (try XCTUnwrap(row[2] as? String), try XCTUnwrap(row[5] as? [Double]))
        })
    }

    private func traits(_ category: UIContentSizeCategory) -> UITraitCollection {
        UITraitCollection(preferredContentSizeCategory: category)
    }

    /// A text role's size is UIKit's at every category, the accessibility
    /// ones included: what `-exact-headline` is at AX2 is what a hand-built
    /// label with `UIFont.preferredFont(forTextStyle: .headline)` is.
    func testTheRampIsUIKitsAtEveryContentSizeCategory() throws {
        let (bodies, styles) = try ramp()
        XCTAssertEqual(styles.count, Self.uikit.count)
        for category in Self.categories {
            let t = traits(category)
            let body = UIFont.preferredFont(forTextStyle: .body, compatibleWith: t).pointSize
            let column = try XCTUnwrap(bodies.firstIndex(where: { abs($0 - Double(body)) < 0.5 }),
                                       "\(category.rawValue): UIKit's body \(body) is a row of the ramp's bodies \(bodies)")
            for (name, sizes) in styles {
                let style = try XCTUnwrap(Self.uikit[name], name)
                let platform = UIFont.preferredFont(forTextStyle: style, compatibleWith: t).pointSize
                XCTAssertEqual(Double(platform), sizes[column], accuracy: 0.5, "\(name) at \(category.rawValue) (body \(body))")
            }
        }
    }

    /// The host measures the font the kernel names at the size it names, so
    /// the width laid out for a role's text at an accessibility size is the
    /// width UIKit's own label draws it at (the font is the same: the system
    /// font at that size and weight), and it grows from the default size's by
    /// the ramp rather than staying where it was.
    func testATextRoleAtAnAccessibilitySizeIsMeasuredAtThatSize() throws {
        let (bodies, styles) = try ramp()
        let engine = TextEngine(resolve: { _ in nil })
        func measured(_ text: String, size: Double, weight: Int) -> CGFloat {
            let run = Run(text: text, size: CGFloat(size), weight: weight, family: 0, italic: false, lineHeight: nil, letterSpacing: 0)
            let spec = Spec(runs: [run], align: 0, lineClamp: 0, color: [0, 0, 0, 255], strut: run)
            return engine.paragraph(spec, width: 10_000).width
        }
        let headline = try XCTUnwrap(styles.first { $0.name == "headline" })
        var widths: [UIContentSizeCategory: CGFloat] = [:]
        for category in [UIContentSizeCategory.large, .accessibilityLarge, .accessibilityExtraExtraExtraLarge] {
            let t = traits(category)
            let body = UIFont.preferredFont(forTextStyle: .body, compatibleWith: t).pointSize
            let size = headline.sizes[try XCTUnwrap(bodies.firstIndex(where: { abs($0 - Double(body)) < 0.5 }))]
            // `-exact-headline` is semibold (the schema's weight).
            let ours = measured("Today", size: size, weight: 600)
            let label = UILabel()
            label.font = UIFont.preferredFont(forTextStyle: .headline, compatibleWith: t)
            label.text = "Today"
            let theirs = label.sizeThatFits(CGSize(width: 10_000, height: 10_000)).width
            XCTAssertEqual(Double(ours), Double(theirs), accuracy: 1.5, "\(category.rawValue): \(size)-point headline, ours against UIKit's label")
            widths[category] = ours
        }
        let large = try XCTUnwrap(widths[.large]), ax2 = try XCTUnwrap(widths[.accessibilityLarge]), ax5 = try XCTUnwrap(widths[.accessibilityExtraExtraExtraLarge])
        XCTAssertGreaterThan(ax2, large * 1.8, "33 points are nearly twice 17: the day label outgrows a 48-point box")
        XCTAssertGreaterThan(large, 40, "and at the default size it fits one, with little to spare")
        XCTAssertLessThan(large, 48)
        XCTAssertGreaterThan(ax2, 48 * 1.5)
        XCTAssertGreaterThan(ax5, ax2, "and the largest setting grows again")
    }
}
#endif
