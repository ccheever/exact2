import XCTest

/// Drives the probe app with real touches. Each test launches the app once per repetition with one scenario, runs its
/// script (a still window and two fades), performs the gestures and asks the app to write its log. Waits are on Darwin
/// notifications the app posts, which, unlike polling the accessibility tree, add no work to the app.
final class TimerWaitsProbeUITests: XCTestCase {
  static let prefix = "dev.exact.timer-waits."

  enum Gesture: String {
    /// Press, drag up slowly, hold still for 500 ms, then lift without velocity
    case heldDrag
    /// A fast swipe up, which decelerates
    case fling
    /// A fast swipe up, a switch to Settings during deceleration, then reactivation
    case flingAway
    /// The same again: the switch takes 0.5 to 4 s on a loaded host, so either may reach the background first. A
    /// slower deceleration is not available: iOS 27.1's `decelerationRate` setter snaps to `.normal` or `.fast`.
    case flingAwayAgain
  }

  override func setUp() {
    continueAfterFailure = false
  }

  func testUpdateLink() {
    run("updateLink", gestures: [.heldDrag, .fling])
  }

  func testRunLoopModes() {
    run("runLoop", gestures: [.heldDrag, .fling])
  }

  func testScrollState() {
    // More repetitions: only some app switches reach the background before the deceleration ends
    run("scrollState", gestures: [.heldDrag, .fling, .flingAway, .flingAwayAgain], repetitions: 5)
  }

  func testDisplayLink() {
    run("displayLink", gestures: [.fling])
  }

  private func run(_ scenario: String, gestures: [Gesture], repetitions: Int = 3) {
    for repetition in 1...repetitions {
      let app = XCUIApplication()
      app.launchArguments = ["-scenario", scenario, "-rep", "\(repetition)"]
      app.launch()
      let table = app.tables.firstMatch
      XCTAssertTrue(table.waitForExistence(timeout: 10), "The probe's table did not appear within 10 s of launch")

      require(signal("scripted"), after: { post("script") }, timeout: 15)
      for gesture in gestures {
        post("gesture.\(gesture.rawValue)")
        perform(gesture, on: table, in: app)
      }
      require(signal("dumped"), after: { post("dump") }, timeout: 10)
      app.terminate()
    }
  }

  private func perform(_ gesture: Gesture, on table: XCUIElement, in app: XCUIApplication) {
    switch gesture {
    case .heldDrag:
      let start = table.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.75))
      let end = table.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.35))
      require(signal("settled"), after: {
        start.press(forDuration: 0.05, thenDragTo: end, withVelocity: XCUIGestureVelocity(300), thenHoldForDuration: 0.5)
      }, timeout: 10)
    case .fling:
      require(signal("settled"), after: { table.swipeUp(velocity: .fast) }, timeout: 15)
    case .flingAway, .flingAwayAgain:
      // The app asks run.sh to switch to Settings once the fling decelerates. The test cannot do it: XCUITest waits
      // for the app to idle after a swipe, which is after the deceleration, and on the iOS 27.1 iPhone Duo simulator
      // `XCUIDevice.shared.press(.home)` did not background the app and once crashed SpringBoard.
      let sampled = signal("backgroundSampled")
      let settled = signal("settled")
      table.swipeUp(velocity: .fast)
      note(XCTWaiter().wait(for: [sampled], timeout: 30), "backgroundSampled within 30 s of the swipe")
      app.activate()
      note(XCTWaiter().wait(for: [settled], timeout: 15), "settled within 15 s of reactivation")
    }
  }

  private func note(_ result: XCTWaiter.Result, _ what: String) {
    XCTContext.runActivity(named: result == .completed ? "Saw \(what)" : "No \(what)") { _ in }
  }

  private func signal(_ name: String) -> XCTDarwinNotificationExpectation {
    XCTDarwinNotificationExpectation(notificationName: Self.prefix + name)
  }

  /// Runs `action`, then waits for `expectation`, which must have been created before `action` so it cannot miss it
  private func require(_ expectation: XCTDarwinNotificationExpectation, after action: () -> Void, timeout: TimeInterval) {
    action()
    let result = XCTWaiter().wait(for: [expectation], timeout: timeout)
    XCTAssertEqual(result, .completed, "The app did not post \(expectation.notificationName) within \(timeout) s; see the app's log in Documents")
  }

  private func post(_ name: String) {
    let center = CFNotificationCenterGetDarwinNotifyCenter()
    CFNotificationCenterPostNotification(center, CFNotificationName((Self.prefix + name) as CFString), nil, nil, true)
  }
}
