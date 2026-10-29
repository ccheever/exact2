// @ref LLP 1133.025 D6 — a launch with no window on macOS: an accessory app
// at every launch, and a login launch whose registration asked `hidden`.
// The host makes the window and does not order it front; the session boots
// by the window's path, and since a window never ordered front never draws,
// the host calls the first-draw continuation itself once the first commit
// is applied and its status items placed. That is the launch's first pixel.
#if os(macOS)
import AppKit

extension ExactSession {
    /// The first frame is published and its status items placed, with no
    /// window shown: what follows first pixel runs now (the app's module, its
    /// data code, held module events), as a drawn first frame would start it.
    public func presentWithoutWindow(_ window: NSWindow) {
        PageFacts.launchedUnshown(window)
        guard state != .destroyed, booted, firstDrawMs == nil else { return }
        presenter.statusItems.sync()
        firstDrawn(generation: generation, token: updateToken)
    }

    /// Whether this session's `-exact-menu="status"` buttons are placed (D4).
    public var placesStatusItems: Bool { presenter.statusItems.placed }

    /// Told when this session's placed status items become none, or some.
    public var onStatusItemsPlaced: ((Bool) -> Void)? {
        get { presenter.statusItems.onPlacedChange }
        set { presenter.statusItems.onPlacedChange = newValue }
    }

    /// What the agent's `state` says of the status items AppKit shows.
    public var observedStatusItems: [[String: Any]] { presenter.statusItems.observed }
}

/// How this launch came, read before the session boots.
public enum LaunchKind {
    /// The open-application Apple event said the login item launched the
    /// app (`keyAELaunchedAsLogInItem`); under the agent, `--login`
    /// (`EXACT_LOGIN_LAUNCH`). Set by `install()`'s handler.
    nonisolated(unsafe) public private(set) static var login = ExactEnv.environment["EXACT_LOGIN_LAUNCH"] != nil

    /// The registration asked to open hidden: the login item package's own
    /// record (`apple/LoginItem.swift`), which neither platform keeps;
    /// under the agent, `--login hidden`.
    public static var hiddenLogin: Bool {
        if let given = ExactEnv.environment["EXACT_LOGIN_LAUNCH"] { return given == "hidden" }
        return login && UserDefaults.standard.bool(forKey: "exact.loginItem.hidden")
    }

    /// The app is a menu-bar app (`LSUIElement`, `host.macos.activationPolicy`).
    public static var accessory: Bool {
        Bundle.main.object(forInfoDictionaryKey: "LSUIElement") as? Bool == true
    }

    /// Whether this launch opens no window.
    public static var withoutWindow: Bool { accessory || hiddenLogin }

    /// Installs the open-application handler, in
    /// `applicationWillFinishLaunching`, before AppKit dispatches that event.
    /// The login item package reads the launch as the unpersisted default
    /// `exact.launch.login`.
    public static func install() {
        if login { UserDefaults.standard.register(defaults: ["exact.launch.login": true]) }
        NSAppleEventManager.shared().setEventHandler(Handler.shared, andSelector: #selector(Handler.open(_:reply:)),
                                                     forEventClass: AEEventClass(kCoreEventClass), andEventID: AEEventID(kAEOpenApplication))
    }

    private final class Handler: NSObject {
        static let shared = Handler()
        @objc func open(_ event: NSAppleEventDescriptor, reply: NSAppleEventDescriptor) {
            let property = event.paramDescriptor(forKeyword: AEKeyword(keyAEPropData))
            guard property?.enumCodeValue == OSType(keyAELaunchedAsLogInItem) else { return }
            LaunchKind.login = true
            UserDefaults.standard.register(defaults: ["exact.launch.login": true])
        }
    }
}
#endif
