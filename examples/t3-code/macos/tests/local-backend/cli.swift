import Foundation
import AppKit
import XCTest

final class LocalCliCommandTests: XCTestCase {
    private var root: URL!
    override func setUpWithError() throws {
        root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-cli-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    }
    override func tearDown() { try? FileManager.default.removeItem(at: root) }
    private func fixture(path: String? = nil, supported: Bool = true) -> T3CliCommand {
        T3CliCommand(supported: supported, home: root.appendingPathComponent("T3's 한글 home"), accountHome: root.path,
                     executable: root.appendingPathComponent("server ' 한글").path, appExecutable: root.appendingPathComponent("app").path,
                     path: path ?? root.appendingPathComponent("preferred").path,
                     candidates: [root.appendingPathComponent("fallback").path, root.appendingPathComponent("preferred").path])
    }
    private func executable(_ path: String, text: String) throws {
        try Data(text.utf8).write(to: URL(fileURLWithPath: path))
        try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: path)
    }
    func testDistributionHandlerIsRequiredAndReleaseOptimizationAloneDoesNotEnableTheCommand() {
        let handler: [String: Any] = ["protocol": "t3code", "url": "/open?u=%s"]
        let types: [[String: Any]] = [["CFBundleURLSchemes": ["t3code"]]]
        XCTAssertTrue(T3LocalPolicy.packaged(info: ["ExactProtocolHandlers": [handler], "CFBundleURLTypes": types]))
        XCTAssertFalse(T3LocalPolicy.packaged(info: [:]))
        XCTAssertFalse(T3LocalPolicy.packaged(info: ["ExactProtocolHandlers": [handler]]))
        XCTAssertFalse(T3LocalPolicy.packaged(info: ["CFBundleURLTypes": types, "ExactProtocolHandlers": [["protocol": "other", "url": "/open?u=%s"]]]))
        XCTAssertFalse(T3LocalPolicy.packaged(info: ["ExactProtocolHandlers": [["protocol": "t3code", "url": "/other"]], "CFBundleURLTypes": types]))
    }
    func testInstallsOnPathAndOnlyRemovesOwnedLinks() throws {
        let command = fixture()
        let state = try command.install()
        let link = root.appendingPathComponent("preferred/t3").path
        XCTAssertEqual(state["installedPath"] as? String, link)
        XCTAssertEqual(state["onPath"] as? Bool, true)
        XCTAssertEqual(try FileManager.default.destinationOfSymbolicLink(atPath: link), command.launcher)
        XCTAssertEqual(try command.install()["installedPath"] as? String, link)
        XCTAssertEqual(try String(contentsOfFile: command.launcher, encoding: .utf8), command.script)
        XCTAssertTrue(FileManager.default.isExecutableFile(atPath: command.launcher))
        XCTAssertNil(try command.uninstall()["installedPath"] as? String)
        XCTAssertTrue(FileManager.default.fileExists(atPath: command.launcher))
    }
    func testShadowingAndForeignLaunchersArePreserved() throws {
        let foreign = root.appendingPathComponent("foreign", isDirectory: true)
        try FileManager.default.createDirectory(at: foreign, withIntermediateDirectories: true)
        let foreignCommand = foreign.appendingPathComponent("t3").path
        try executable(foreignCommand, text: "#!/bin/sh\nexit 0\n")
        let command = fixture(path: foreign.path)
        XCTAssertEqual(command.state()["shadowedBy"] as? String, foreignCommand)
        XCTAssertThrowsError(try command.install()) { XCTAssertTrue(String(describing: $0).contains("Another t3")) }
        XCTAssertTrue(FileManager.default.fileExists(atPath: foreignCommand))
        _ = try command.uninstall()
        XCTAssertTrue(FileManager.default.fileExists(atPath: foreignCommand))
        try "unowned".write(toFile: command.launcher, atomically: true, encoding: .utf8)
        XCTAssertThrowsError(try command.installLauncher())
        XCTAssertEqual(try String(contentsOfFile: command.launcher, encoding: .utf8), "unowned")
        XCTAssertThrowsError(try fixture(supported: false).install())
    }
    func testLauncherPreservesArgumentsAndHomeOverridesAndReportsAMovedApp() throws {
        let command = fixture()
        try executable(command.appExecutable, text: "#!/bin/sh\nexit 0\n")
        try executable(command.executable, text: "#!/bin/sh\nprintf '%s\\n' \"$T3CODE_HOME\" \"$T3CODE_CLI_PATH\" \"$@\"\n")
        _ = try command.installLauncher()
        func run(home: String?) throws -> (Int32, String) {
            let process = Process(), output = Pipe()
            process.executableURL = URL(fileURLWithPath: command.launcher)
            process.arguments = ["two words", "$(not executed)", "한글's"]
            process.environment = home.map { ["T3CODE_HOME": $0] } ?? [:]
            process.standardOutput = output; process.standardError = output
            try process.run()
            let text = String(decoding: output.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
            process.waitUntilExit()
            return (process.terminationStatus, text)
        }
        let (status, text) = try run(home: nil)
        XCTAssertEqual(status, 0)
        XCTAssertEqual(text, "\(command.home.path)\n\(command.launcher)\ntwo words\n$(not executed)\n한글's\n")
        XCTAssertTrue(try run(home: "/custom home").1.hasPrefix("/custom home\n"))
        try FileManager.default.removeItem(atPath: command.appExecutable)
        let moved = try run(home: nil)
        XCTAssertEqual(moved.0, 127)
        XCTAssertTrue(moved.1.contains(T3CliCommand.moved))
    }
    func testFaviconPickerAdmitsIcnsFiles() throws {
        _ = NSApplication.shared
        let panel = T3Menus.faviconPanel(startingAt: root.path)
        XCTAssertTrue(panel.allowedContentTypes.contains { $0.preferredFilenameExtension == "icns" })
        let icon = root.appendingPathComponent("AppIcon.icns")
        try Data("fixture".utf8).write(to: icon)
        var picked: String?
        T3Menus().pickProjectFavicon(startingAt: root.path, importsRoot: root) { picked = $0 }
        XCTAssertEqual(picked.map { URL(fileURLWithPath: $0).resolvingSymlinksInPath().path }, icon.resolvingSymlinksInPath().path)
    }
    func testBootstrapTokenRotatesAtTwelveHoursAndTheEnvelopeCarriesTheSecret() throws {
        let window = T3DesktopBootstrap.windowMs
        let token = T3DesktopBootstrap.token(secret: "desktop-secret", nowMs: 10 * window + 1)
        XCTAssertEqual(token, "5d78c2ca298011b8e4d1864d152fadd3bc83a0d5432d55cc7e97767a14fa6b02")
        XCTAssertEqual(token, T3DesktopBootstrap.token(secret: "desktop-secret", nowMs: 11 * window - 1))
        XCTAssertNotEqual(token, T3DesktopBootstrap.token(secret: "desktop-secret", nowMs: 11 * window))
        XCTAssertNotEqual(token, T3DesktopBootstrap.token(secret: "other-secret", nowMs: 10 * window))
        let config = T3LocalBackend.startConfig(versionDir: root, port: 16495, home: root, token: token, env: [:], cwd: root.path, secret: "desktop-secret")
        let value = try XCTUnwrap(JSONSerialization.jsonObject(with: config.bootstrap.jsonLine()) as? [String: Any])
        XCTAssertEqual(value["desktopBootstrapToken"] as? String, token)
        XCTAssertEqual(value["desktopBootstrapSecret"] as? String, "desktop-secret")
        XCTAssertFalse(config.args.contains(token)); XCTAssertFalse(config.env.values.contains("desktop-secret"))
        let backend = T3LocalBackend()
        var now = 10 * window
        backend.nowMs = { now }
        let first = backend.currentBootstrapToken
        XCTAssertEqual(first.count, 64)
        now += window
        XCTAssertNotEqual(first, backend.currentBootstrapToken)
    }
}
