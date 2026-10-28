import UIKit

/// What a launch measures. Each scenario installs only its own instruments, since some change what others see: a
/// `CADisplayLink` asks for frames on every vsync, and a timer wakes the run loop.
enum Scenario: String {
  /// A passive `UIUpdateLink`, with the order of its phases against touches and scrolling
  case updateLink
  /// A `beforeWaiting` observer and repeating timers in the default and common modes
  case runLoop
  /// `isTracking`, `isDragging` and `isDecelerating`, sampled every 50 ms, across gestures and backgrounding
  case scrollState
  /// A `CADisplayLink` with the default `preferredFrameRateRange`
  case displayLink

  /// The event kinds counted per phase in the summary
  var measuredKinds: [String] {
    let common = ["scroll", "touch.moved"]
    switch self {
    case .updateLink:
      return ["link.afterUpdateScheduled", "link.beforeEventDispatch", "link.afterEventDispatch", "link.afterUpdateComplete", "fire"] + common
    case .runLoop:
      return ["beforeWaiting.default", "beforeWaiting.common", "timer.default", "timer.common"] + common
    case .scrollState:
      return ["sample"] + common
    case .displayLink:
      return ["displayLink"] + common
    }
  }
}

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
  func application(_ application: UIApplication, configurationForConnecting session: UISceneSession, options: UIScene.ConnectionOptions) -> UISceneConfiguration {
    let configuration = UISceneConfiguration(name: nil, sessionRole: session.role)
    configuration.delegateClass = SceneDelegate.self
    return configuration
  }
}

final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
  var window: UIWindow?

  func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options connectionOptions: UIScene.ConnectionOptions) {
    guard let scene = scene as? UIWindowScene else {
      return
    }
    let defaults = UserDefaults.standard
    let scenario = defaults.string(forKey: "scenario").flatMap(Scenario.init(rawValue:)) ?? .updateLink
    let window = ProbeWindow(windowScene: scene)
    window.rootViewController = ProbeViewController(scenario: scenario, repetition: max(defaults.integer(forKey: "rep"), 1))
    window.makeKeyAndVisible()
    self.window = window
  }
}

/// Logs each touch as the window starts dispatching it, before gesture recognizers and views see it
final class ProbeWindow: UIWindow {
  override func sendEvent(_ event: UIEvent) {
    if event.type == .touches, let touch = event.allTouches?.first {
      Recorder.shared.log("touch.\(touch.phase.name)")
    }
    super.sendEvent(event)
  }
}

private extension UITouch.Phase {
  var name: String {
    switch self {
    case .began: "began"
    case .moved: "moved"
    case .stationary: "stationary"
    case .ended: "ended"
    case .cancelled: "cancelled"
    default: "other"
    }
  }
}
