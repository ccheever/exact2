import QuartzCore
import UIKit

/// The platform callbacks each scenario observes. `install` returns the objects that must stay alive.
@MainActor
enum Instruments {
  static func install(_ scenario: Scenario, view: UIView, scrollView: UIScrollView) -> [AnyObject] {
    switch scenario {
    case .updateLink: [passiveUpdateLink(view: view)]
    case .runLoop: runLoopObservers()
    case .scrollState: [scrollSampler(scrollView)]
    case .displayLink: [DisplayLinkProbe()]
    }
  }

  /// A `UIUpdateLink` that observes UI updates without asking for any (`requiresContinuousUpdates` false). It logs
  /// `afterUpdateScheduled`, the phase the RFC uses, and three more phases that place input dispatch in the update.
  static func passiveUpdateLink(view: UIView) -> UIUpdateLink {
    let link = UIUpdateLink(view: view)
    let phases: [(String, UIUpdateActionPhase)] = [
      ("afterUpdateScheduled", .afterUpdateScheduled),
      ("beforeEventDispatch", .beforeEventDispatch),
      ("afterEventDispatch", .afterEventDispatch),
      ("afterUpdateComplete", .afterUpdateComplete),
    ]
    for (name, phase) in phases {
      link.addAction(to: phase) { _, info in
        Recorder.shared.log("link.\(name)", update: info.modelTime)
      }
    }
    // Set this only after adding an action: the iOS 27.1 setter calls `os_crash` ("UIUpdateLink without associated
    // phase actions can't request continuous updates.") on a link with none, even to set false, the default
    link.requiresContinuousUpdates = false
    link.isEnabled = true
    Recorder.shared.log(
      "link.configured",
      note: "requiresContinuousUpdates=\(link.requiresContinuousUpdates) wantsLowLatencyEventDispatch=\(link.wantsLowLatencyEventDispatch)"
    )
    return link
  }

  /// A `beforeWaiting` observer in the default mode only and one in the common modes, and 100 ms repeating timers in
  /// each, on the main run loop
  static func runLoopObservers() -> [AnyObject] {
    var objects: [AnyObject] = []
    for (name, mode) in [("default", CFRunLoopMode.defaultMode), ("common", CFRunLoopMode.commonModes)] {
      let observer = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue, true, 0) { _, _ in
        MainActor.assumeIsolated { Recorder.shared.log("beforeWaiting.\(name)") }
      }
      CFRunLoopAddObserver(CFRunLoopGetMain(), observer, mode)
      objects.append(observer!)
    }
    for (name, mode) in [("default", RunLoop.Mode.default), ("common", RunLoop.Mode.common)] {
      let timer = Timer(timeInterval: 0.1, repeats: true) { _ in
        MainActor.assumeIsolated { Recorder.shared.log("timer.\(name)") }
      }
      RunLoop.main.add(timer, forMode: mode)
      objects.append(timer)
    }
    return objects
  }

  /// Samples the scroll view's state every 50 ms in the common modes, so tracking does not hold it back
  static func scrollSampler(_ scrollView: UIScrollView) -> Timer {
    let timer = Timer(timeInterval: 0.05, repeats: true) { [weak scrollView] _ in
      MainActor.assumeIsolated {
        guard let scrollView else {
          return
        }
        Recorder.shared.log("sample", scroll: scrollView, value: Double(UIApplication.shared.applicationState.rawValue))
      }
    }
    RunLoop.main.add(timer, forMode: .common)
    return timer
  }
}

/// A `CADisplayLink` left at the default `preferredFrameRateRange`. Each callback logs `targetTimestamp - timestamp`,
/// the frame duration the link is running at.
@MainActor
final class DisplayLinkProbe: NSObject {
  private var link: CADisplayLink?

  override init() {
    super.init()
    let link = CADisplayLink(target: self, selector: #selector(tick))
    Recorder.shared.log(
      "displayLink.configured",
      note: "preferredFrameRateRange=\(link.preferredFrameRateRange.minimum)-\(link.preferredFrameRateRange.maximum) preferred \(link.preferredFrameRateRange.preferred ?? 0)"
    )
    link.add(to: .main, forMode: .common)
    self.link = link
  }

  @objc private func tick(_ link: CADisplayLink) {
    Recorder.shared.log("displayLink", value: link.targetTimestamp - link.timestamp)
  }
}
