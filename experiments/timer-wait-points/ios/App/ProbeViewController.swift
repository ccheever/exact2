import UIKit

/// A tall table of plain rows and a small box that the script fades. The UI test drives it through `Signal`s: `script`
/// runs the still window and the two animations, `gesture.<name>` labels the gesture the test performs next, and `dump`
/// writes the log. The app answers `scripted`, `decelerating`, `settled`, `backgroundSampled` and `dumped`, and asks
/// `run.sh` for an app switch with `backgroundNow`.
final class ProbeViewController: UIViewController, UITableViewDataSource, UITableViewDelegate {
  let scenario: Scenario
  let repetition: Int
  /// What drives the app: "xctest" for the UI test, or "control" for `run.sh` posting signals with no test attached
  let driver = UserDefaults.standard.string(forKey: "driver") ?? "xctest"
  private let tableView = UITableView(frame: .zero, style: .plain)
  private let box = UIView()
  private var instruments: [AnyObject] = []
  private var settleTimer: Timer?
  private var started = false
  /// The gesture the test said it performs next
  private var gesture = ""

  init(scenario: Scenario, repetition: Int) {
    self.scenario = scenario
    self.repetition = repetition
    super.init(nibName: nil, bundle: nil)
  }

  @available(*, unavailable)
  required init?(coder: NSCoder) {
    fatalError("init(coder:) is not supported")
  }

  override func viewDidLoad() {
    super.viewDidLoad()
    tableView.frame = view.bounds
    tableView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    tableView.rowHeight = 44
    tableView.dataSource = self
    tableView.delegate = self
    tableView.register(UITableViewCell.self, forCellReuseIdentifier: "row")
    tableView.panGestureRecognizer.addTarget(self, action: #selector(pan))
    view.addSubview(tableView)

    box.frame = CGRect(x: 24, y: 120, width: 64, height: 64)
    box.backgroundColor = .systemOrange
    view.addSubview(box)

    Recorder.shared.view = view
    Signal.observe("script") { [weak self] in self?.runScript() }
    Signal.observe("dump") { [weak self] in self?.dump() }
    for gesture in ["heldDrag", "fling", "flingAway", "flingAwayAgain"] {
      Signal.observe("gesture.\(gesture)") { [weak self] in
        self?.gesture = gesture
        Recorder.shared.log("gesture", note: gesture)
      }
    }
    observeLifecycle()
  }

  override func viewDidAppear(_ animated: Bool) {
    super.viewDidAppear(animated)
    guard !started else {
      return
    }
    started = true
    // Start mid-table so that no gesture reaches an edge and bounces
    tableView.scrollToRow(at: IndexPath(row: 5_000, section: 0), at: .top, animated: false)
    instruments = Instruments.install(scenario, view: view, scrollView: tableView)
    Recorder.shared.log("ready", note: scenario.rawValue)
  }

  // MARK: Script

  /// Still for 2 s, then a 1 s `UIView.animate` fade, 1 s still, a 1 s `UIViewPropertyAnimator` fade and 1 s still
  private func runScript() {
    let recorder = Recorder.shared
    recorder.log("still.begin")
    after(2) { [self] in
      recorder.log("still.end")
      recorder.log("animate.begin")
      UIView.animate(withDuration: 1, animations: { self.box.alpha = 0.2 }) { _ in
        recorder.log("animate.end")
        self.after(1) {
          recorder.log("animator.begin")
          let animator = UIViewPropertyAnimator(duration: 1, curve: .easeInOut) { self.box.alpha = 1 }
          animator.addCompletion { _ in
            recorder.log("animator.end")
            self.after(1) {
              let finish = {
                recorder.log("scripted")
                Signal.post("scripted")
                // With no test to ask for it, write the log as soon as the script ends
                if self.driver != "xctest" {
                  self.dump()
                }
              }
              if self.scenario == .updateLink {
                self.runFiringSegments(then: finish)
              } else {
                finish()
              }
            }
          }
          animator.startAnimation()
        }
      }
    }
  }

  /// Three segments, 500 ms apart, of ten callbacks 100 ms apart that change nothing on screen: a `Timer` that logs
  /// with `UIUpdateInfo.current(for:)`, a `Timer` that logs without it, and a chain of `asyncAfter` blocks without it.
  /// The passive link's callbacks in each show what makes a UI update on a still screen.
  private func runFiringSegments(then done: @escaping @MainActor () -> Void) {
    let recorder = Recorder.shared
    func segment(_ name: String, next: @escaping @MainActor () -> Void) -> (Int) -> Void {
      recorder.log("segment.begin", note: name, lookup: false)
      return { fired in
        if fired == 10 {
          recorder.log("segment.end", note: name, lookup: false)
          self.after(0.5, next)
        }
      }
    }
    func timer(_ name: String, lookup: Bool, next: @escaping @MainActor () -> Void) {
      let ended = segment(name, next: next)
      var fired = 0
      Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { timer in
        MainActor.assumeIsolated {
          fired += 1
          recorder.log("fire", note: name, lookup: lookup)
          if fired == 10 {
            timer.invalidate()
          }
          ended(fired)
        }
      }
    }
    func dispatch(_ name: String, next: @escaping @MainActor () -> Void) {
      let ended = segment(name, next: next)
      var fired = 0
      func schedule() {
        after(0.1) {
          fired += 1
          recorder.log("fire", note: name, lookup: false)
          ended(fired)
          if fired < 10 {
            schedule()
          }
        }
      }
      schedule()
    }
    timer("timerLookup", lookup: true) {
      timer("timerBare", lookup: false) {
        dispatch("dispatchBare", next: done)
      }
    }
  }

  private func after(_ seconds: Double, _ body: @escaping @MainActor () -> Void) {
    DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { MainActor.assumeIsolated(body) }
  }

  private func dump() {
    Recorder.shared.log("dump")
    let screen = view.window?.windowScene?.screen
    let environment = [
      "systemVersion": UIDevice.current.systemVersion,
      "model": ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] ?? UIDevice.current.model,
      "deviceName": ProcessInfo.processInfo.environment["SIMULATOR_DEVICE_NAME"] ?? UIDevice.current.name,
      "maximumFramesPerSecond": "\(screen?.maximumFramesPerSecond ?? 0)",
      "simulator": ProcessInfo.processInfo.environment["SIMULATOR_UDID"] == nil ? "false" : "true",
      "driver": driver,
    ]
    // Name a run without the UI test, the control, apart from the test's runs
    let base = (driver == "xctest" ? "" : "\(driver)-") + "\(scenario.rawValue)-\(repetition)"
    do {
      try Recorder.shared.write(base: base, scenario: scenario, repetition: repetition, environment: environment)
      Signal.post("dumped")
    } catch {
      NSLog("TimerWaitsProbe could not write its results to Documents: \(error)")
      Signal.post("dumpFailed")
    }
  }

  // MARK: Scrolling

  @objc private func pan(_ recognizer: UIPanGestureRecognizer) {
    let state = switch recognizer.state {
    case .began: "began"
    case .changed: "changed"
    case .ended: "ended"
    case .cancelled: "cancelled"
    default: "other"
    }
    Recorder.shared.log("pan.\(state)")
  }

  func scrollViewDidScroll(_ scrollView: UIScrollView) {
    Recorder.shared.log("scroll", scroll: scrollView)
  }

  func scrollViewWillBeginDragging(_ scrollView: UIScrollView) {
    settleTimer?.invalidate()
    Recorder.shared.log("willBeginDragging", scroll: scrollView)
  }

  func scrollViewWillEndDragging(_ scrollView: UIScrollView, withVelocity velocity: CGPoint, targetContentOffset: UnsafeMutablePointer<CGPoint>) {
    Recorder.shared.log("willEndDragging", scroll: scrollView, value: velocity.y)
  }

  func scrollViewDidEndDragging(_ scrollView: UIScrollView, willDecelerate decelerate: Bool) {
    Recorder.shared.log("didEndDragging", scroll: scrollView, value: decelerate ? 1 : 0)
    if decelerate {
      Signal.post("decelerating")
      // Ask run.sh to switch to Settings now: XCUITest waits for the deceleration to end before its next action
      if gesture.hasPrefix("flingAway") {
        Signal.post("backgroundNow")
      }
    } else {
      settleSoon()
    }
  }

  func scrollViewWillBeginDecelerating(_ scrollView: UIScrollView) {
    Recorder.shared.log("willBeginDecelerating", scroll: scrollView)
  }

  func scrollViewDidEndDecelerating(_ scrollView: UIScrollView) {
    Recorder.shared.log("didEndDecelerating", scroll: scrollView)
    settleSoon()
  }

  /// Signals `settled` 1 s after the scroll stops, so the summary sees a quiet tail before the next gesture
  private func settleSoon() {
    settleTimer?.invalidate()
    settleTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: false) { _ in
      MainActor.assumeIsolated {
        Recorder.shared.log("settled")
        Signal.post("settled")
      }
    }
  }

  private func observeLifecycle() {
    let events: [(Notification.Name, String)] = [
      (UIScene.willDeactivateNotification, "willDeactivate"),
      (UIScene.didEnterBackgroundNotification, "didEnterBackground"),
      (UIScene.willEnterForegroundNotification, "willEnterForeground"),
      (UIScene.didActivateNotification, "didActivate"),
    ]
    for (name, kind) in events {
      NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
        MainActor.assumeIsolated {
          guard let self else {
            return
          }
          Recorder.shared.log(kind, scroll: self.tableView)
          if kind == "didEnterBackground", self.scenario == .scrollState {
            self.sampleInBackground()
          }
        }
      }
    }
  }

  /// Keeps the process running for 2 s in the background so the sampler can watch `isDecelerating` there
  private func sampleInBackground() {
    let task = UIApplication.shared.beginBackgroundTask(withName: "sample")
    after(2) {
      Recorder.shared.log("backgroundSampled", scroll: self.tableView)
      Signal.post("backgroundSampled")
      UIApplication.shared.endBackgroundTask(task)
    }
  }

  // MARK: Table

  func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
    10_000
  }

  func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
    let cell = tableView.dequeueReusableCell(withIdentifier: "row", for: indexPath)
    var content = cell.defaultContentConfiguration()
    content.text = "Row \(indexPath.row)"
    cell.contentConfiguration = content
    return cell
  }
}
