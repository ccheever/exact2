// LLP 1133.023 stage 2's simulator drives of Shelf, as a person reaches it
// from outside the app: the Home Screen quick action, Spotlight, and the
// Shortcuts app's Log Pages. Each test leaves a screenshot and the
// accessibility hierarchy of each step in REACH_OUT. `apple-ui.mjs` builds
// and runs it (no Xcode project).
import XCTest

final class ShelfReachUITests: XCTestCase {
    private let out = ProcessInfo.processInfo.environment["REACH_OUT"] ?? NSTemporaryDirectory()
    private let shelf = XCUIApplication(bundleIdentifier: "com.exact.shelf")
    private let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
    private var step = 0

    override func setUp() { continueAfterFailure = false }

    private func save(_ app: XCUIApplication, _ name: String) {
        step += 1
        let base = "\(out)/\(name)-\(step)"
        try? XCUIScreen.main.screenshot().pngRepresentation.write(to: URL(fileURLWithPath: base + ".png"))
        try? app.debugDescription.write(toFile: base + ".txt", atomically: true, encoding: .utf8)
    }

    private func element(_ app: XCUIApplication, _ id: String) -> XCUIElement { app.descendants(matching: .any)[id] }

    /// Shelf's icon on the Home Screen, long pressed: Add Book opens the add sheet.
    private func quickAction(cold: Bool) {
        if cold { shelf.terminate() } else { shelf.activate(); XCUIDevice.shared.press(.home) }
        XCUIDevice.shared.press(.home)
        let icon = springboard.icons["Shelf"]
        XCTAssertTrue(icon.waitForExistence(timeout: 20), "Shelf's icon")
        icon.press(forDuration: 1.5)
        let add = springboard.buttons["Add Book"]
        save(springboard, cold ? "quick-cold" : "quick-warm")
        XCTAssertTrue(add.waitForExistence(timeout: 10), "the manifest's Add Book in the icon's menu")
        add.tap()
        XCTAssertTrue(element(shelf, "route-add").waitForExistence(timeout: 30), "the add sheet is up")
        save(shelf, cold ? "quick-cold" : "quick-warm")
    }

    func test1QuickActionCold() { quickAction(cold: true) }
    func test2QuickActionWarm() { quickAction(cold: false) }

    /// Spotlight's own search field finds a book Shelf indexed, and the result opens it.
    func test3SpotlightOpensTheBook() {
        // A launch outside the agent sets the index (Shelf's `index` task).
        shelf.launch()
        sleep(5)
        XCUIDevice.shared.press(.home)
        springboard.swipeDown()
        let field = springboard.searchFields.firstMatch
        XCTAssertTrue(field.waitForExistence(timeout: 10), "Spotlight's field")
        field.typeText(ProcessInfo.processInfo.environment["REACH_BOOK"] ?? "Piranesi")
        sleep(4)
        save(springboard, "spotlight")
        let result = springboard.cells.containing(.staticText, identifier: ProcessInfo.processInfo.environment["REACH_BOOK"] ?? "Piranesi").firstMatch
        XCTAssertTrue(result.waitForExistence(timeout: 15), "the book among Spotlight's results")
        result.tap()
        XCTAssertTrue(element(shelf, "route-book").waitForExistence(timeout: 30), "the book's screen is up")
        save(shelf, "spotlight")
    }

    /// The Shortcuts app lists Shelf's Log Pages among the actions it can add: the system read the metadata from a
    /// bundle `build.mjs` assembled and found the intent's type in the executable (the stage's kill line).
    func test4ShortcutsListsLogPages() {
        let shortcuts = XCUIApplication(bundleIdentifier: "com.apple.shortcuts")
        shortcuts.launch()
        sleep(3)
        save(shortcuts, "shortcuts")
        for label in ["Continue", "Not Now", "OK"] where shortcuts.buttons[label].exists { shortcuts.buttons[label].tap() }
        let add = shortcuts.navigationBars.buttons["Add"].exists ? shortcuts.navigationBars.buttons["Add"] : shortcuts.buttons["Add"]
        if add.waitForExistence(timeout: 10) { add.tap() }
        sleep(2)
        save(shortcuts, "shortcuts")
        let search = shortcuts.searchFields.firstMatch
        XCTAssertTrue(search.waitForExistence(timeout: 10), "the actions' search field")
        search.tap()
        search.typeText("Log Pages")
        sleep(3)
        save(shortcuts, "shortcuts")
        XCTAssertTrue(shortcuts.staticTexts["Log Pages"].waitForExistence(timeout: 20), "Shelf's Log Pages among the actions")
    }
}
