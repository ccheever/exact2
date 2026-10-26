import AppKit
import XCTest

// Compile with the production native-module facade, its generated app keys,
// T3WindowChrome.swift and T3Menus.swift (see README). Real AppKit menus and windows.
let exactModule: ExactModule.Type = ExactModule.self

final class MenuTests: XCTestCase {
    /// pickProjectFavicon: one image (WORKSPACE_IMAGE_PREVIEW_EXTENSIONS), starting at the workspace root.
    func testFaviconPanelPicksOneImageFromTheWorkspaceRoot() throws {
        _ = NSApplication.shared
        let root = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("t3-favicon-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let panel = T3Menus.faviconPanel(startingAt: root.path)
        XCTAssertEqual(panel.directoryURL?.standardizedFileURL.path, root.standardizedFileURL.path)
        XCTAssertFalse(panel.allowsMultipleSelection)
        XCTAssertTrue(panel.canChooseFiles)
        XCTAssertFalse(panel.canChooseDirectories)
        XCTAssertEqual(Set(panel.allowedContentTypes.compactMap(\.preferredFilenameExtension)).isSuperset(of: ["png", "svg", "gif", "jpeg", "webp", "ico"]), true)
        // Under the agent: the imports folder's first image, or nothing.
        let imports = root.appendingPathComponent("imports", isDirectory: true)
        try FileManager.default.createDirectory(at: imports, withIntermediateDirectories: true)
        try Data("x".utf8).write(to: imports.appendingPathComponent("notes.txt"))
        var picked: String?? = .none
        T3Menus().pickProjectFavicon(startingAt: root.path, importsRoot: imports) { picked = .some($0) }
        XCTAssertEqual(picked, .some(nil))
        try Data("x".utf8).write(to: imports.appendingPathComponent("icon.png"))
        T3Menus().pickProjectFavicon(startingAt: root.path, importsRoot: imports) { picked = .some($0) }
        XCTAssertEqual(picked.flatMap { $0 }.map { URL(fileURLWithPath: $0).resolvingSymlinksInPath().path }, imports.appendingPathComponent("icon.png").resolvingSymlinksInPath().path)
    }
    /// The tall title row is the host's (display_override); the module moves the lights to the reference's x.
    func testWindowChromeMovesTheLightsToTheReferencesX() {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 200), styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        let chrome = T3WindowChrome()
        chrome.attach(window)
        XCTAssertNil(window.toolbar, "the module installs no toolbar of its own")
        T3WindowChrome.positionControls(in: window)
        XCTAssertEqual(window.standardWindowButton(.closeButton)?.frame.minX ?? 0, 16, accuracy: 0.5)
        chrome.destroy()
    }

    func testEditGainsPasteAsText() {
        _ = NSApplication.shared
        let bar = NSMenu()
        let edit = NSMenu(title: "Edit")
        for title in ["Undo", "Redo"] { edit.addItem(withTitle: title, action: nil, keyEquivalent: "") }
        edit.addItem(.separator())
        for title in ["Cut", "Copy", "Paste", "Delete", "Select All"] { edit.addItem(withTitle: title, action: nil, keyEquivalent: "") }
        // The host's Edit ends with its own Speech since exact2 #226 (DevMenuMac.swift).
        edit.addItem(.separator())
        let speech = NSMenu(title: "Speech")
        for title in ["Start Speaking", "Stop Speaking"] { speech.addItem(withTitle: title, action: nil, keyEquivalent: "") }
        edit.addItem(withTitle: "Speech", action: nil, keyEquivalent: "").submenu = speech
        bar.addItem(withTitle: "Edit", action: nil, keyEquivalent: "").submenu = edit
        let menus = T3Menus()
        menus.augment(bar)
        menus.augment(bar)
        // DesktopApplicationMenu.ts's Edit.
        XCTAssertEqual(edit.items.map { $0.isSeparatorItem ? "—" : $0.title }, ["Undo", "Redo", "—", "Cut", "Copy", "Paste", "Paste as Text", "Delete", "—", "Select All", "—", "Speech"])
        let pasteText = edit.item(withTitle: "Paste as Text")!
        XCTAssertEqual(pasteText.keyEquivalent, "v")
        XCTAssertEqual(pasteText.keyEquivalentModifierMask, [.command, .shift])
        XCTAssertNotEqual(pasteText.action, #selector(NSTextView.pasteAsPlainText(_:)), "not the action `paste-and-match-style` removes")
        // It sends the first responder's own `pasteAsPlainText:`, and is enabled only where a responder takes it.
        XCTAssertEqual(menus.validateMenuItem(pasteText), NSApp.target(forAction: #selector(NSTextView.pasteAsPlainText(_:)), to: nil, from: pasteText) != nil)
    }

    /// Chosen with the pointer, Paste as Text arms the composer's paste for one second, once (DESKTOP_PASTE_AS_TEXT_EVENT,
    /// armPasteAsTextShortcut, consumePasteAsTextShortcut): TypeScript asks for it with `composerPasteAsText`.
    func testPasteAsTextChosenFromTheMenuArmsOnePaste() {
        _ = NSApplication.shared
        let menus = T3Menus()
        XCTAssertFalse(menus.takePasteAsText(), "not armed before the item is chosen")
        menus.pasteAsText(nil)
        let now = ProcessInfo.processInfo.systemUptime
        XCTAssertTrue(menus.takePasteAsText(now: now))
        XCTAssertFalse(menus.takePasteAsText(now: now), "one paste consumes it")
        menus.pasteAsText(nil)
        XCTAssertFalse(menus.takePasteAsText(now: ProcessInfo.processInfo.systemUptime + 1.5), "the deadline is one second")
    }

    /// Check for Updates… is Contract's (settings-shortcuts.contract UpdateMenuItems); its press shows the
    /// reference's informational box, with no parent window.
    func testCheckForUpdatesShowsTheUnavailableBox() {
        _ = NSApplication.shared
        let menus = T3Menus()
        menus.updatesDisabledReason = { "Automatic updates are not available because no update feed is configured." }
        var shown: [NSAlert] = []
        menus.presentAlert = { shown.append($0) }
        menus.checkForUpdates(nil)
        XCTAssertEqual(shown.count, 1)
        XCTAssertEqual(shown[0].messageText, "Automatic updates are not available right now.")
        XCTAssertEqual(shown[0].informativeText, "Automatic updates are not available because no update feed is configured.")
        XCTAssertEqual(shown[0].alertStyle, .informational)
        XCTAssertEqual(shown[0].buttons.map(\.title), ["OK"])
        // A build linking the update store has no hook here: nothing shows.
        menus.updatesDisabledReason = { nil }
        menus.checkForUpdates(nil)
        XCTAssertEqual(shown.count, 1)
    }
    func testUpdatesDisabledReasonReadsTheReceiptComposition() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("t3-menus-\(ProcessInfo.processInfo.processIdentifier)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let embedded = directory.appendingPathComponent("embedded.json"), updating = directory.appendingPathComponent("updating.json")
        try Data(#"{"composition":"embedded"}"#.utf8).write(to: embedded)
        try Data(#"{"composition":"updating"}"#.utf8).write(to: updating)
        XCTAssertEqual(T3Menus.updatesDisabledReason(receipt: embedded), "Automatic updates are not available because no update feed is configured.")
        XCTAssertNil(T3Menus.updatesDisabledReason(receipt: updating))
        XCTAssertNotNil(T3Menus.updatesDisabledReason(receipt: directory.appendingPathComponent("missing.json")))
    }
}


final class InstalledFontTests: XCTestCase {
    func testCatalogContainsInstalledFamiliesAndFixedPitchTraits() {
        let catalog = T3InstalledFonts.catalog()
        let names = catalog.compactMap { $0["family"] as? String }
        XCTAssertEqual(Set(names), Set(NSFontManager.shared.availableFontFamilies))
        XCTAssertEqual(names.count, Set(names).count)
        XCTAssertEqual(catalog.first { $0["family"] as? String == "Arial" }?["monospace"] as? Bool, false)
        XCTAssertEqual(catalog.first { $0["family"] as? String == "Menlo" }?["monospace"] as? Bool, true)
        let arial = NSFontManager.shared.font(withFamily: "Arial", traits: [], weight: 5, size: 14)
        let menlo = NSFontManager.shared.font(withFamily: "Menlo", traits: [], weight: 5, size: 14)
        XCTAssertEqual(arial?.familyName, "Arial")
        XCTAssertEqual(menlo?.familyName, "Menlo")
        XCTAssertNotEqual(arial?.fontName, menlo?.fontName)
    }
}

let suite = XCTestSuite(name: "Menus")
suite.addTest(MenuTests.defaultTestSuite)
suite.addTest(InstalledFontTests.defaultTestSuite)
suite.run()
guard let run = suite.testRun, run.executionCount == 7 else { print("Menus: unexpected test count \(suite.testRun?.executionCount ?? 0)"); exit(1) }
print("Menus: \(run.executionCount) tests, \(run.totalFailureCount) failures")
exit(run.hasSucceeded ? 0 : 1)
