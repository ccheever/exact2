import UIKit
import WebKit

/// A full-screen WKWebView over the three.js twin, as `android/` is the system WebView's. The page and
/// three.js are in the bundle (`web/`), served under `twin://app/` so module scripts load as from a site;
/// the launch's first argument is the page's query (`devicectl device process launch --console … dev.exact.twin
/// 'scene=cubes&n=32000&fit=screen'`). The page's report goes to stderr and `Documents/bench.json` as `BENCH {…}`,
/// and the app exits.
@main
final class TwinApp: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        application.isIdleTimerDisabled = true
        return true
    }

    func application(_ application: UIApplication, configurationForConnecting session: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
        let config = UISceneConfiguration(name: nil, sessionRole: session.role)
        config.delegateClass = TwinScene.self
        return config
    }
}

final class TwinScene: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        let window = UIWindow(windowScene: scene as! UIWindowScene)
        window.rootViewController = TwinController()
        window.makeKeyAndVisible()
        self.window = window
    }
}

final class TwinController: UIViewController, WKScriptMessageHandler, WKURLSchemeHandler {
    override var prefersStatusBarHidden: Bool { true }
    override var prefersHomeIndicatorAutoHidden: Bool { true }

    override func loadView() {
        let config = WKWebViewConfiguration()
        config.setURLSchemeHandler(self, forURLScheme: "twin")
        config.userContentController.add(self, name: "bench")
        let web = WKWebView(frame: .zero, configuration: config)
        // Under the status bar and the home indicator, as Exact's full-screen canvas draws: the same pixels.
        web.scrollView.contentInsetAdjustmentBehavior = .never
        web.scrollView.isScrollEnabled = false
        web.isInspectable = true
        web.isOpaque = false
        web.backgroundColor = .black
        view = web
        let args = CommandLine.arguments.dropFirst().first { !$0.hasPrefix("-") }
        web.load(URLRequest(url: URL(string: "twin://app/bench.html?" + (args ?? "scene=cubes&n=10000&fit=screen"))!))
    }

    func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
        let line = "BENCH \(message.body)\n"
        FileHandle.standardError.write(Data(line.utf8))
        let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        try? Data(line.utf8).write(to: documents.appendingPathComponent("bench.json"))
        exit(0)
    }

    func webView(_ webView: WKWebView, start task: WKURLSchemeTask) {
        let path = task.request.url!.path
        let file = Bundle.main.resourceURL!.appendingPathComponent("web").appendingPathComponent(path)
        guard let data = try? Data(contentsOf: file) else {
            task.didFailWithError(URLError(.fileDoesNotExist))
            return
        }
        let type = path.hasSuffix(".html") ? "text/html" : "text/javascript"
        task.didReceive(HTTPURLResponse(url: task.request.url!, statusCode: 200, httpVersion: "HTTP/1.1",
                                        headerFields: ["Content-Type": type, "Cache-Control": "no-store"])!)
        task.didReceive(data)
        task.didFinish()
    }

    func webView(_ webView: WKWebView, stop task: WKURLSchemeTask) {}
}
