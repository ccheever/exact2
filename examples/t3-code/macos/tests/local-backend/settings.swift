import Foundation
import XCTest

// Decision U7 (2026-10-08): the desktop settings live in `<T3 home>/userdata/desktop-settings.json`, as the
// original app keeps them (T3DesktopSettings.swift). The first tests port T3 Code's own with their names
// (MIT, see LICENSE-T3; reference 1e2ecbd975: apps/desktop/src/settings/DesktopAppSettings.test.ts, the
// cases that apply on macOS); the rest cover the clone's wiring: the read at attach, the status, the
// `desktopSettingsSet` setters and the one-time carry-over from t3-code.json.
final class LocalDesktopSettingsTests: XCTestCase {
    private final class Owner {}
    private let owner = Owner()
    private let nightly = "0.0.17-nightly.20260415.1"

    private func scratch(_ name: String) -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("t3-settings-\(name)-\(UUID().uuidString)", isDirectory: true)
        try! FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url.resolvingSymlinksInPath()
    }
    /// A store over `<home>/userdata/desktop-settings.json` (the reference tests' `withSettings`, app version 0.0.17).
    private func store(_ home: URL, appVersion: String = "0.0.17") -> (T3DesktopSettingsStore, URL) {
        let store = T3DesktopSettingsStore(appVersion: appVersion), path = T3DesktopSettings.path(home: home)
        store.load(path: path)
        return (store, path)
    }
    private func write(_ text: String, _ path: URL) {
        try! FileManager.default.createDirectory(at: path.deletingLastPathComponent(), withIntermediateDirectories: true)
        try! text.write(to: path, atomically: true, encoding: .utf8)
    }
    private func read(_ path: URL) -> String { (try? String(contentsOf: path, encoding: .utf8)) ?? "" }
    private func set(_ store: T3DesktopSettingsStore, _ change: @escaping (inout T3DesktopSettings) -> Void) throws -> Bool {
        try store.persist { var next = $0; change(&next); return next }.changed
    }

    func testPersistsDisablingAndReEnablingLocalExecutionWithoutClearingBackendSettings() throws {
        let (settings, path) = store(scratch("home"))
        XCTAssertTrue(try set(settings) { $0.serverExposureMode = "network-accessible" })
        let before = settings.settings
        XCTAssertTrue(try set(settings) { $0.localEnvironmentEnabled = false })
        settings.load(path: path)
        var expected = before; expected.localEnvironmentEnabled = false
        XCTAssertEqual(settings.settings, expected)
        XCTAssertFalse(try set(settings) { $0.localEnvironmentEnabled = false })
        XCTAssertTrue(try set(settings) { $0.localEnvironmentEnabled = true })
        settings.load(path: path)
        XCTAssertEqual(settings.settings, before)
    }

    func testLoadsDefaultsWhenNoSettingsFileExists() {
        let (settings, path) = store(scratch("home"))
        XCTAssertEqual(settings.settings, T3DesktopSettings())
        XCTAssertFalse(FileManager.default.fileExists(atPath: path.path), "a read creates nothing")
    }

    func testDefaultsPackagedNightlyBuildsToTheNightlyUpdateChannel() {
        var expected = T3DesktopSettings(); expected.updateChannel = "nightly"
        XCTAssertEqual(T3DesktopSettings.defaults(appVersion: nightly), expected)
        // This client mirrors a Nightly (version-skew.ts CLIENT_VERSION).
        XCTAssertEqual(T3DesktopSettings.defaults().updateChannel, "nightly")
    }

    func testLoadsPersistedSettingsAndAppliesSemanticUpdates() throws {
        let home = scratch("home"), path = T3DesktopSettings.path(home: home)
        write(#"{"linuxPasswordStore":"gnome-libsecret","serverExposureMode":"network-accessible","tailscaleServeEnabled":true,"tailscaleServePort":8443,"updateChannel":"latest","updateChannelConfiguredByUser":true}"# + "\n", path)
        let (settings, _) = store(home)
        var expected = T3DesktopSettings()
        expected.linuxPasswordStore = "gnome-libsecret"; expected.serverExposureMode = "network-accessible"; expected.tailscaleServeEnabled = true
        expected.tailscaleServePort = 8443; expected.updateChannel = "latest"; expected.updateChannelConfiguredByUser = true
        XCTAssertEqual(settings.settings, expected)
        XCTAssertTrue(try set(settings) { $0.serverExposureMode = "local-only" })
        XCTAssertEqual(settings.settings.serverExposureMode, "local-only")
        XCTAssertTrue(try set(settings) { $0.tailscaleServeEnabled = true; $0.tailscaleServePort = 9443 })
        XCTAssertEqual(settings.settings.tailscaleServePort, 9443)
    }

    func testReportsTheFailedDesktopSettingsWriteOperationAndPath() {
        let home = scratch("home"), path = T3DesktopSettings.path(home: home)
        try! FileManager.default.createDirectory(at: path, withIntermediateDirectories: true)
        let (settings, _) = store(home)
        XCTAssertThrowsError(try set(settings) { $0.serverExposureMode = "network-accessible" }) { error in
            let failure = error as? T3DesktopSettingsWriteError
            XCTAssertEqual(failure?.operation, "replace-settings-file")
            XCTAssertEqual(failure?.path, path.path)
            XCTAssertEqual(failure?.message, "Desktop settings write failed during replace-settings-file at \(path.path).")
        }
        XCTAssertEqual(settings.settings.serverExposureMode, "local-only", "a failed write changes nothing in memory")
    }

    func testDoesNotPersistNoOpSemanticUpdates() throws {
        let (settings, path) = store(scratch("home"))
        XCTAssertFalse(try set(settings) { $0.serverExposureMode = "local-only" })
        XCTAssertFalse(try set(settings) { $0.tailscaleServeEnabled = false })
        XCTAssertFalse(FileManager.default.fileExists(atPath: path.path))
    }

    func testFallsBackToDefaultsWhenTheSettingsFileIsMalformed() {
        let home = scratch("home")
        write("{not-json", T3DesktopSettings.path(home: home))
        XCTAssertEqual(store(home).0.settings, T3DesktopSettings())
    }

    func testLoadsLenientPersistedDesktopSettingsJSON() {
        let home = scratch("home")
        write("""
        {
          // JSONC-style comments and trailing commas match server settings parsing.
          "serverExposureMode": "network-accessible",
          "tailscaleServeEnabled": true,
          "tailscaleServePort": 8443,
          "mainWindowBounds": { "x": 120, "y": 80, "width": 1280, "height": 900 },
        }

        """, T3DesktopSettings.path(home: home))
        var expected = T3DesktopSettings()
        expected.mainWindowBounds = T3WindowBounds(x: 120, y: 80, width: 1280, height: 900)
        expected.serverExposureMode = "network-accessible"; expected.tailscaleServeEnabled = true; expected.tailscaleServePort = 8443
        XCTAssertEqual(store(home).0.settings, expected)
        // A string keeps a `//` or a `,}` inside it.
        XCTAssertEqual(T3DesktopSettings.stripLenient(#"{"wslDistro":"a//b,}", /* c */ }"#), #"{"wslDistro":"a//b,}"  }"#)
    }

    func testRejectsWindowBoundsThatDoNotSatisfyTheDomainSchema() {
        let home = scratch("home")
        write(#"{"mainWindowBounds":{"x":10.5,"y":20,"width":839,"height":620},"mainWindowMaximized":true,"serverExposureMode":"network-accessible"}"#, T3DesktopSettings.path(home: home))
        let loaded = store(home).0.settings
        XCTAssertNil(loaded.mainWindowBounds)
        XCTAssertFalse(loaded.mainWindowMaximized)
        XCTAssertEqual(loaded.serverExposureMode, "network-accessible")
    }

    func testNormalizesUnsupportedLinuxPasswordStoreValuesWithoutDroppingOtherSettings() {
        let home = scratch("home")
        write(#"{"linuxPasswordStore":"unsupported-store","serverExposureMode":"network-accessible","tailscaleServeEnabled":true,"tailscaleServePort":8443,"updateChannel":"nightly","updateChannelConfiguredByUser":true}"#, T3DesktopSettings.path(home: home))
        var expected = T3DesktopSettings()
        expected.serverExposureMode = "network-accessible"; expected.tailscaleServeEnabled = true; expected.tailscaleServePort = 8443
        expected.updateChannel = "nightly"; expected.updateChannelConfiguredByUser = true
        XCTAssertEqual(store(home).0.settings, expected)
    }

    func testPersistsSparseDesktopSettingsDocuments() throws {
        let (settings, path) = store(scratch("home"))
        XCTAssertTrue(try set(settings) { $0.mainWindowBounds = T3WindowBounds(x: -1200, y: 40, width: 1440, height: 960); $0.mainWindowMaximized = true })
        XCTAssertTrue(try set(settings) { $0.serverExposureMode = "network-accessible" })
        // Compact JSON in the schema's key order and a newline (`JSON.stringify` of the sparse document).
        XCTAssertEqual(read(path), #"{"mainWindowBounds":{"x":-1200,"y":40,"width":1440,"height":960},"mainWindowMaximized":true,"serverExposureMode":"network-accessible"}"# + "\n")
        XCTAssertTrue(try set(settings) { $0.mainWindowBounds = nil; $0.mainWindowMaximized = false; $0.serverExposureMode = "local-only" })
        XCTAssertEqual(read(path), "{}\n", "all defaults")
    }

    func testSavesThroughASymlinkedSettingsFileWithoutReplacingTheLink() throws {
        let home = scratch("home"), dotfiles = scratch("dotfiles"), linked = dotfiles.appendingPathComponent("desktop-settings.json"), path = T3DesktopSettings.path(home: home)
        write("{}\n", linked)
        try! FileManager.default.createDirectory(at: path.deletingLastPathComponent(), withIntermediateDirectories: true)
        try! FileManager.default.createSymbolicLink(atPath: path.path, withDestinationPath: linked.path)
        let (settings, _) = store(home)
        XCTAssertTrue(try set(settings) { $0.serverExposureMode = "network-accessible" })
        XCTAssertEqual(try FileManager.default.destinationOfSymbolicLink(atPath: path.path), linked.path)
        XCTAssertEqual(read(linked), #"{"serverExposureMode":"network-accessible"}"# + "\n")
    }

    func testMigratesLegacyImplicitUpdateChannelsToTheRuntimeDefault() {
        let home = scratch("home")
        write(#"{"serverExposureMode":"local-only","updateChannel":"latest"}"#, T3DesktopSettings.path(home: home))
        var expected = T3DesktopSettings(); expected.updateChannel = "nightly"
        XCTAssertEqual(store(home, appVersion: nightly).0.settings, expected)
    }

    func testPreservesExplicitStableUpdateChannelOnNightlyBuilds() {
        let home = scratch("home")
        write(#"{"serverExposureMode":"local-only","updateChannel":"latest","updateChannelConfiguredByUser":true}"#, T3DesktopSettings.path(home: home))
        var expected = T3DesktopSettings(); expected.updateChannelConfiguredByUser = true
        XCTAssertEqual(store(home, appVersion: nightly).0.settings, expected)
    }

    func testNormalizesInvalidPersistedTailscaleServePorts() {
        let home = scratch("home")
        write(#"{"tailscaleServeEnabled":true,"tailscaleServePort":0}"#, T3DesktopSettings.path(home: home))
        var expected = T3DesktopSettings(); expected.tailscaleServeEnabled = true
        XCTAssertEqual(store(home).0.settings, expected)
    }

    func testMigratesLegacyWslModeWslToWslBackendEnabledOnLoad() throws {
        let home = scratch("home"), path = T3DesktopSettings.path(home: home)
        write(#"{"wslMode":"wsl"}"#, path)
        let (settings, _) = store(home)
        XCTAssertTrue(settings.settings.wslBackendEnabled)
        XCTAssertTrue(try set(settings) { $0.serverExposureMode = "network-accessible" })
        XCTAssertEqual(read(path), #"{"serverExposureMode":"network-accessible","wslBackendEnabled":true}"# + "\n", "the legacy key drops out on the next write")
    }

    func testDropsInvalidPersistedWslDistroValuesOnLoad() {
        let home = scratch("home")
        write(#"{"wslDistro":"bad;name"}"#, T3DesktopSettings.path(home: home))
        XCTAssertNil(store(home).0.settings.wslDistro)
        write(#"{"wslDistro":"Ubuntu-22.04"}"#, T3DesktopSettings.path(home: home))
        XCTAssertEqual(store(home).0.settings.wslDistro, "Ubuntu-22.04")
    }

    // MARK: The clone's wiring

    func testOneKeyOfTheWrongTypeMakesTheWholeDocumentInvalid() {
        // The reference decodes the Struct first: a failed key fails it, and `orElseSucceed` gives every default.
        for text in [#"{"localEnvironmentEnabled":"no","serverExposureMode":"network-accessible"}"#, #"{"serverExposureMode":"foo","tailscaleServeEnabled":true}"#,
                     #"{"tailscaleServePort":"8443","localEnvironmentEnabled":false}"#, #"{"localEnvironmentEnabled":null}"#, #"[]"#, #""""#] {
            XCTAssertEqual(T3DesktopSettings.read(text, appVersion: "0.0.17"), T3DesktopSettings(), text)
        }
        // Unknown keys are ignored, and dropped from the next write.
        XCTAssertEqual(T3DesktopSettings.read(#"{"somethingNew":1,"localEnvironmentEnabled":false}"#, appVersion: "0.0.17").localEnvironmentEnabled, false)
    }

    func testTheBackendReadsTheFileAtAttachAndPublishesItsKeys() throws {
        let home = scratch("home"), data = scratch("data")
        write(#"{"localEnvironmentEnabled":false,"tailscaleServePort":8443}"#, T3DesktopSettings.path(home: home))
        let backend = T3LocalBackend()
        backend.environment = { ["T3_LOCAL_HOME": home.path, "T3_LOCAL_PORT": "16899", "PATH": "/usr/bin:/bin"] }
        backend.resources = { nil }
        backend.attach(owner, dataRoot: data, changed: { _ in })
        let status = backend.statusValue()
        XCTAssertEqual(status["enabled"] as? Bool, false, "read before attach returns: no default flash")
        XCTAssertEqual(NSDictionary(dictionary: status["desktopSettings"] as? [String: Any] ?? [:]),
                       NSDictionary(dictionary: ["localEnvironmentEnabled": false, "serverExposureMode": "local-only", "tailscaleServeEnabled": false, "tailscaleServePort": 8443]))
        // desktopSettingsSet: one setter at a time, persisted before it answers.
        let value = try backend.setDesktopSettings(["serverExposureMode": "network-accessible"])
        XCTAssertEqual(value["changed"] as? Bool, true)
        XCTAssertEqual(read(T3DesktopSettings.path(home: home)), #"{"localEnvironmentEnabled":false,"serverExposureMode":"network-accessible","tailscaleServePort":8443}"# + "\n")
        XCTAssertEqual(try backend.setDesktopSettings(["serverExposureMode": "network-accessible"])["changed"] as? Bool, false)
        _ = try backend.setDesktopSettings(["tailscaleServeEnabled": true, "tailscaleServePort": 70_000])
        XCTAssertEqual((backend.statusValue()["desktopSettings"] as? [String: Any])?["tailscaleServePort"] as? Int, 443, "normalizeTailscaleServePort")
        XCTAssertThrowsError(try backend.setDesktopSettings(["serverExposureMode": "everywhere"]))
        backend.detach(owner)
    }

    func testTheClonesOldKeysAreCarriedOverOnceAndTheDesktopFileWins() {
        let home = scratch("home"), data = scratch("data"), path = T3DesktopSettings.path(home: home)
        // T3 Code wrote its own choice for the port; the clone's t3-code.json still has all four keys.
        write(#"{"tailscaleServePort":9443,"mainWindowBounds":{"x":0,"y":0,"width":1100,"height":780}}"#, path)
        try! #"{"version":1,"localEnvironmentEnabled":false,"serverExposureMode":"network-accessible","tailscaleServeEnabled":true,"tailscaleServePort":8443}"#
            .write(to: data.appendingPathComponent("t3-code.json"), atomically: true, encoding: .utf8)
        let backend = T3LocalBackend()
        backend.environment = { ["T3_LOCAL_HOME": home.path, "T3_LOCAL_PORT": "16899", "PATH": "/usr/bin:/bin"] }
        backend.resources = { nil }
        backend.attach(owner, dataRoot: data, changed: { _ in })
        let settings = backend.settings.settings
        XCTAssertEqual([settings.localEnvironmentEnabled, settings.serverExposureMode == "network-accessible", settings.tailscaleServeEnabled], [false, true, true])
        XCTAssertEqual(settings.tailscaleServePort, 9443, "a key the desktop file has is the desktop file's")
        XCTAssertEqual(read(path), #"{"localEnvironmentEnabled":false,"mainWindowBounds":{"x":0,"y":0,"width":1100,"height":780},"serverExposureMode":"network-accessible","tailscaleServeEnabled":true,"tailscaleServePort":9443}"# + "\n",
                       "T3 Code's other keys are kept")
        XCTAssertEqual(backend.statusValue()["enabled"] as? Bool, false)
        backend.detach(owner)
    }

    func testARefusedDevelopmentBuildKeepsTheSettingsInMemory() throws {
        let data = scratch("data")
        let backend = T3LocalBackend()
        backend.environment = { ["PATH": "/usr/bin:/bin"] }
        backend.resources = { nil }
        backend.attach(owner, dataRoot: data, changed: { _ in })
        XCTAssertNil(backend.settings.path, "no T3 home")
        XCTAssertEqual(try backend.setDesktopSettings(["localEnvironmentEnabled": false])["changed"] as? Bool, true)
        XCTAssertEqual(backend.settings.settings.localEnvironmentEnabled, false)
        backend.detach(owner)
    }
}
