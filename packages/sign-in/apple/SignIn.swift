// @exact/sign-in on iOS and macOS (LLP 1133.014 D3, D5, D7): the capabilities
// this build can offer and the providers' buttons. The identity member's
// requests (Apple's in passkeys' controller through the session's request
// table, Google's through the host's auth session, D10 items 2 and 3) are
// stage 2's and not built: a request reaching this implementation fails
// `NotSupportedError`. Under the agent the substitute answers it.
//
// - `getIdentityCapabilities`: Apple where the build carries
//   `com.apple.developer.applesignin` (on macOS only a build whose profile
//   authorizes it, D7; read with `SecTaskCopyValueForEntitlement`), active,
//   passive and beside a passkey; Google active, through a browser session.
// - `exact-sign-in-button`: Apple's `ASAuthorizationAppleIDButton`, its
//   type from `context` and its style from the appearance (black in light,
//   white in dark, the HIG's); Google's standard button, its "G" drawn here.
import AuthenticationServices
import ExactPackage
import Foundation
#if os(macOS)
import AppKit
#else
import UIKit
#endif

public final class SignInModule: NSObject, ExactPackage {
    let context: ExactPackageContext

    public init(context: ExactPackageContext) { self.context = context }

    private var apple: Bool {
        #if os(macOS)
        guard let task = SecTaskCreateFromSelf(nil) else { return false }
        return SecTaskCopyValueForEntitlement(task, "com.apple.developer.applesignin" as CFString, nil) != nil
        #else
        return true
        #endif
    }

    private func capabilities() -> [String: Any] {
        let a = apple
        return ["available": true, "providers": [
            "apple": ["active": a, "passive": a, "withPublicKey": a],
            "google": ["active": true, "passive": false, "withPublicKey": false],
        ]]
    }

    public func later(_ name: String, _ args: [String: Any], preferred: Any?, reply: @escaping ([String: Any]) -> Void) {
        switch name {
        case "getIdentityCapabilities": reply(capabilities())
        case "preventSilentAccess": reply([:]) // neither Apple provider signs in without UI (D1)
        default: reply(["error": "NotSupportedError", "message": "\(name) is not built on Apple yet (LLP 1133.014 §11 stage 2)"])
        }
    }

    public func call(_ name: String, _ args: [String: Any]) throws -> [String: Any] {
        name == "getIdentityCapabilitiesSync" ? capabilities() : ["error": "NotSupportedError"]
    }

    public func view(_ tag: String, id: String, props: [String: String], events: ExactPackageEvents) throws -> ExactPackageView {
        guard tag == "exact-sign-in-button" else { throw ExactPackageRefusal("no view \(tag)") }
        let button = SignInButton(events: events)
        try button.setProps(props)
        events.load()
        return button
    }
}

/// The provider's button: Apple's system button, or Google's standard one.
final class SignInButton: NSObject, ExactPackageView {
    let view: ExactPackagePlatformView
    private let events: ExactPackageEvents
    private var shown: (provider: String, context: String)?

    init(events: ExactPackageEvents) {
        self.events = events
        #if os(macOS)
        view = NSView()
        #else
        view = UIView()
        #endif
    }

    func setProps(_ props: [String: String]) throws {
        let provider = props["provider"] ?? "apple", context = props["context"] ?? "signin"
        guard provider == "apple" || provider == "google" else { throw ExactPackageRefusal("provider \(provider)") }
        if shown?.provider == provider, shown?.context == context { return }
        shown = (provider, context)
        for sub in view.subviews { sub.removeFromSuperview() }
        let made = provider == "apple" ? appleButton(context, radius: props["cornerRadius"].flatMap(Double.init)) : googleButton(context)
        made.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(made)
        NSLayoutConstraint.activate([
            made.leadingAnchor.constraint(equalTo: view.leadingAnchor), made.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            made.topAnchor.constraint(equalTo: view.topAnchor), made.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
    }

    @objc func pressed() { events.press() }

    private func appleButton(_ context: String, radius: Double?) -> ExactPackagePlatformView {
        let type: ASAuthorizationAppleIDButton.ButtonType = context == "signup" ? .signUp : context == "continue" ? .continue : .signIn
        #if os(macOS)
        let dark = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        let button = ASAuthorizationAppleIDButton(authorizationButtonType: type, authorizationButtonStyle: dark ? .white : .black)
        button.target = self
        button.action = #selector(pressed)
        #else
        let dark = view.traitCollection.userInterfaceStyle == .dark
        let button = ASAuthorizationAppleIDButton(authorizationButtonType: type, authorizationButtonStyle: dark ? .white : .black)
        button.addTarget(self, action: #selector(pressed), for: .touchUpInside)
        #endif
        if let radius { button.cornerRadius = radius }
        return button
    }

    private func googleButton(_ context: String) -> ExactPackagePlatformView {
        let words = context == "signup" ? "Sign up with Google" : context == "continue" ? "Continue with Google" : "Sign in with Google"
        #if os(macOS)
        let button = NSButton(title: words, image: Self.mark(), target: self, action: #selector(pressed))
        button.imagePosition = .imageLeading
        button.bezelStyle = .rounded
        button.setAccessibilityLabel(words)
        #else
        var config = UIButton.Configuration.bordered()
        config.title = words
        config.image = Self.mark()
        config.imagePadding = 10
        config.baseForegroundColor = UIColor(red: 0.12, green: 0.12, blue: 0.12, alpha: 1)
        config.baseBackgroundColor = .white
        config.background.strokeColor = UIColor(red: 0.455, green: 0.467, blue: 0.459, alpha: 1)
        config.background.strokeWidth = 1
        config.cornerStyle = .small
        let button = UIButton(configuration: config)
        button.addTarget(self, action: #selector(pressed), for: .touchUpInside)
        button.accessibilityLabel = words
        #endif
        return button
    }

    /// Google's "G": four arcs in its colours and the bar, 18 points square.
    private static func mark() -> PlatformImage {
        let size = CGSize(width: 18, height: 18)
        let draw: (CGContext) -> Void = { g in
            let centre = CGPoint(x: 9, y: 9), r: CGFloat = 6.75, width: CGFloat = 3.4
            g.setLineWidth(width)
            let arcs: [(CGFloat, CGFloat, CGColor)] = [
                (-40, 45, CGColor(red: 0.259, green: 0.522, blue: 0.957, alpha: 1)),
                (45, 135, CGColor(red: 0.204, green: 0.659, blue: 0.325, alpha: 1)),
                (135, 200, CGColor(red: 0.984, green: 0.737, blue: 0.020, alpha: 1)),
                (200, 320, CGColor(red: 0.918, green: 0.263, blue: 0.208, alpha: 1)),
            ]
            for (from, to, colour) in arcs {
                g.setStrokeColor(colour)
                g.beginPath()
                g.addArc(center: centre, radius: r, startAngle: from * .pi / 180, endAngle: to * .pi / 180, clockwise: false)
                g.strokePath()
            }
            g.setFillColor(arcs[0].2)
            g.fill(CGRect(x: 9, y: 9 - width / 2, width: r + width / 2, height: width))
        }
        #if os(macOS)
        return NSImage(size: size, flipped: true) { _ in
            if let g = NSGraphicsContext.current?.cgContext { draw(g) }
            return true
        }
        #else
        return UIGraphicsImageRenderer(size: size).image { draw($0.cgContext) }
        #endif
    }
}

#if os(macOS)
typealias PlatformImage = NSImage
#else
typealias PlatformImage = UIImage
#endif
