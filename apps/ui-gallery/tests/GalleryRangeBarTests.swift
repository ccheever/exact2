import XCTest
import UIKit

final class GalleryRangeBarTests: XCTestCase {
    @MainActor
    func testMaterialCoversTheCollapsedHeaderAndSafeAreaGutters() throws {
        let controller = UIViewController()
        let navigation = UINavigationController(rootViewController: controller)
        navigation.loadViewIfNeeded()
        navigation.view.frame = CGRect(x: 0, y: 0, width: 480, height: 800)
        navigation.view.layoutIfNeeded()
        controller.loadViewIfNeeded()
        let background = RangeBarBackground(frame: .zero)
        controller.view.addSubview(background)
        let material = try XCTUnwrap(background.subviews.first as? UIVisualEffectView)

        for top: CGFloat in [82, 24, 82, 24] {
            let column = controller.view.convert(navigation.view.bounds, from: navigation.view)
            background.frame = CGRect(x: column.minX + 16, y: column.minY + top, width: column.width - 32, height: 56)
            background.setNeedsLayout()
            background.layoutIfNeeded()
            let coverage = material.convert(material.bounds, to: navigation.view)
            XCTAssertEqual(coverage.minY, navigation.view.bounds.minY, accuracy: 0.5,
                           "The collapsed header must not expose a blank strip above the material")
            XCTAssertEqual(coverage.minX, navigation.view.bounds.minX, accuracy: 0.5)
            XCTAssertEqual(coverage.maxX, navigation.view.bounds.maxX, accuracy: 0.5)
            let bar = background.convert(background.bounds, to: navigation.view)
            XCTAssertEqual(coverage.maxY, bar.maxY + 16, accuracy: 0.5,
                           "Extending the material upward preserves the fading lower edge")
            let gradient = try XCTUnwrap(material.mask?.layer.sublayers?.first as? CAGradientLayer)
            let fadeStart = CGFloat(try XCTUnwrap(gradient.locations?[1]).doubleValue) * material.bounds.height
            XCTAssertEqual(coverage.minY + fadeStart, bar.maxY - 8, accuracy: 0.5)
        }
    }
}
