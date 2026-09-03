// The hostile sample host, iOS (LLP 1031 D10): a native UIKit app with a
// navigation controller hosting two sessions of the one plan the archive
// carries. The fixture the smoke drives; the external consumer is the proof.
import ExactKit
import UIKit

final class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?
    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        let w = UIWindow(frame: UIScreen.main.bounds)
        let a = ExactApp.shared.makeSession(label: "a")
        let vc = UIViewController()
        let view = ExactView(session: a)
        view.frame = vc.view.bounds
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        vc.view.addSubview(view)
        w.rootViewController = UINavigationController(rootViewController: vc)
        w.makeKeyAndVisible()
        window = w
        return true
    }
}

UIApplicationMain(CommandLine.argc, CommandLine.unsafeArgv, nil, NSStringFromClass(AppDelegate.self))
