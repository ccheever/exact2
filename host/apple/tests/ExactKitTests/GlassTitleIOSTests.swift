#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A glass button's title in a role (`-exact-label`) is UIKit's own colour (`UIColor.label`), as a
/// hand-built button's is: UIKit brings it out of a sheet's dim as the sheet leaves, where a colour
/// resolved from the role kept its dimmed look until the sheet had gone. The compiled fixture
/// `glass-title.contract` beside a hand-built `UIButton.Configuration.glass()`. UIKit in a scene's window, so the hosted bundle runs it:
/// bun host/apple/build.mjs --test --ios
final class GlassTitleIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        window?.isHidden = true
        window = nil
        session = nil
        super.tearDown()
    }

    /// The title's colour is UIKit's own label colour, the hand-built button's
    func testAGlassButtonsTitleInARoleIsUIKitsOwnColour() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "a glass button draws in a scene's window") }
        guard #available(iOS 26.0, *) else { throw XCTSkip("glass is iOS 26") }
        let (exact, twin, _) = try stage()
        XCTAssertEqual(exact.configuration?.baseForegroundColor, UIColor.label, "the configuration's foreground is UIKit's label colour")
        XCTAssertEqual(exact.titleLabel?.textColor, twin.titleLabel?.textColor, "the title draws in the hand-built button's colour")
    }

    /// As a sheet over it leaves, UIKit animates the title out of the dim exactly as it animates the
    /// hand-built button's: the same animations on the two buttons' layers at every turn
    func testAGlassButtonsTitleComesOutOfADimAsAHandBuiltOnesDoes() throws {
        guard UIApplication.shared.connectedScenes.first is UIWindowScene else { throw missing("scene", "a sheet presents only in a scene's window") }
        guard #available(iOS 26.0, *) else { throw XCTSkip("glass is iOS 26") }
        let (exact, twin, controller) = try stage()
        let resting = animations(twin)
        let sheet = UIViewController()
        sheet.view.backgroundColor = .systemBackground
        controller.present(sheet, animated: false)
        until("UIKit dims the screen under the sheet") { controller.view.tintAdjustmentMode == .dimmed }
        var dismissed = false, animated = false
        controller.dismiss(animated: true) { dismissed = true }
        until("the sheet leaves") {
            XCTAssertEqual(animations(exact), animations(twin), "the title comes out of the dim as the hand-built one's")
            if animations(twin) != resting { animated = true }
            return dismissed
        }
        XCTAssertTrue(animated, "UIKit animated the hand-built button out of the dim")
    }

    /// The fixture's glass button, its hand-built twin 200 points to its right, and their controller
    @available(iOS 26.0, *)
    private func stage() throws -> (UIButton, UIButton, UIViewController) {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_GLASS_TITLE_PLAN"], "the hosted run compiles the fixture")
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(x: 0, y: 0, width: 400, height: 200)
        window.overrideUserInterfaceStyle = .light
        let controller = UIViewController()
        controller.view.backgroundColor = .white
        window.rootViewController = controller
        window.makeKeyAndVisible()
        self.window = window
        let session = ExactApp.shared.makeSession(label: "glass-title")
        self.session = session
        session.presenter.viewport.frame = controller.view.bounds
        controller.view.addSubview(session.presenter.viewport)
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: controller.view.bounds.size).error)
        window.layoutIfNeeded()
        let node = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "glass" })
        let exact = try XCTUnwrap(session.presenter.controls.controls[node.id] as? UIButton, "the fixture's glass button")
        var configuration = UIButton.Configuration.glass()
        configuration.image = UIImage(systemName: "map.fill")?.withTintColor(.systemRed, renderingMode: .alwaysOriginal)
        configuration.title = "Parked"
        configuration.baseForegroundColor = .label
        let twin = UIButton(configuration: configuration)
        twin.frame = exact.convert(exact.bounds, to: controller.view).offsetBy(dx: 200, dy: 0)
        controller.view.addSubview(twin)
        window.layoutIfNeeded()
        return (exact, twin, controller)
    }

    /// The animations on a view's layers, each by its layer's path in the tree and its key
    private func animations(_ view: UIView) -> Set<String> {
        func walk(_ layer: CALayer, _ path: String) -> [String] {
            (layer.animationKeys() ?? []).map { "\(path):\($0)" } + (layer.sublayers ?? []).enumerated().flatMap { walk($1, "\(path)/\($0)") }
        }
        return Set(walk(view.layer, ""))
    }

    /// Runs the main run loop until `done`, for at most `seconds` (docs/testing.md)
    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.005)) }
        XCTAssertTrue(done(), what)
    }
}
#endif
