// DesktopCliCommand.ts and DesktopCliShim.ts (2359643cff; MIT, see LICENSE-T3).
// Exact's bundled server is an executable, so its launcher invokes the installed SEA directly.
import Foundation
import Darwin

final class T3CliCommand {
    private static let lock = NSRecursiveLock()
    static let marker = "Written by T3 Code: runs the desktop app's bundled t3 CLI."
    static let moved = "T3 Code has moved or been removed. Open the app once to update this command."
    let supported: Bool
    let home: URL
    let executable: String
    let appExecutable: String
    let path: String
    let candidates: [String]
    let ensureRuntime: () throws -> Void
    var launcher: String { home.appendingPathComponent("bin/t3").path }

    init(supported: Bool, home: URL, accountHome: String, executable: String, appExecutable: String,
         path: String, candidates: [String]? = nil, ensureRuntime: @escaping () throws -> Void = {}) {
        self.supported = supported; self.home = home
        self.executable = executable; self.appExecutable = appExecutable; self.path = path
        self.candidates = candidates ?? ["/opt/homebrew/bin", "/usr/local/bin", "\(accountHome)/.local/bin", "\(accountHome)/bin"]
        self.ensureRuntime = ensureRuntime
    }

    private static func word(_ value: String) -> String { "'" + value.replacingOccurrences(of: "'", with: "'\"'\"'") + "'" }
    var script: String {
        ["#!/bin/sh", "# \(Self.marker)", "export T3CODE_CLI_PATH=\(Self.word(launcher))",
         "t3_home=\(Self.word(home.path))", "export T3CODE_HOME=\"${T3CODE_HOME:-$t3_home}\"",
         "app=\(Self.word(appExecutable))", "cli=\(Self.word(executable))",
         "if [ ! -x \"$app\" ] || [ ! -x \"$cli\" ]; then", "  echo \"\(Self.moved)\" >&2", "  exit 127", "fi",
         "exec \"$cli\" \"$@\"", ""].joined(separator: "\n")
    }

    private func isOurLink(_ link: String) -> Bool {
        let fs = FileManager.default
        guard (try? fs.destinationOfSymbolicLink(atPath: link)) != nil,
              let attributes = try? fs.attributesOfItem(atPath: URL(fileURLWithPath: link).resolvingSymlinksInPath().path),
              attributes[.type] as? FileAttributeType == .typeRegular,
              let size = attributes[.size] as? NSNumber, size.intValue <= 16_384,
              let text = try? String(contentsOfFile: link, encoding: .utf8) else { return false }
        return text.contains(Self.marker)
    }
    /// PATH can reach the launcher itself (its own folder on PATH); removal still takes only `isOurLink`.
    private func runsOurLauncher(_ candidate: String) -> Bool {
        if isOurLink(candidate) { return true }
        let resolved = URL(fileURLWithPath: candidate).resolvingSymlinksInPath().path
        guard resolved == URL(fileURLWithPath: launcher).resolvingSymlinksInPath().path,
              let attributes = try? FileManager.default.attributesOfItem(atPath: resolved),
              attributes[.type] as? FileAttributeType == .typeRegular,
              let size = attributes[.size] as? NSNumber, size.intValue <= 16_384,
              let text = try? String(contentsOfFile: resolved, encoding: .utf8) else { return false }
        return text.contains(Self.marker)
    }
    private var installed: String? { candidates.map { ($0 as NSString).appendingPathComponent("t3") }.first(where: isOurLink) }
    /// The first `t3` on PATH a shell would run: a regular file, through symlinks, that is executable.
    private var firstOnPath: String? {
        let fs = FileManager.default
        return path.split(separator: ":").map { (String($0) as NSString).appendingPathComponent("t3") }.first { candidate in
            let target = URL(fileURLWithPath: candidate).resolvingSymlinksInPath().path
            return (try? fs.attributesOfItem(atPath: target))?[.type] as? FileAttributeType == .typeRegular && fs.isExecutableFile(atPath: candidate)
        }
    }
    private var shadowed: String? { firstOnPath.flatMap { runsOurLauncher($0) ? nil : $0 } }
    func state() -> [String: Any] {
        Self.lock.lock(); defer { Self.lock.unlock() }
        guard supported else { return ["supported": false, "installedPath": NSNull(), "onPath": false] }
        let installed = self.installed
        var value: [String: Any] = ["supported": true, "installedPath": installed.map { $0 as Any } ?? NSNull(),
                                    "onPath": firstOnPath.map(runsOurLauncher) == true]
        if let shadowed { value["shadowedBy"] = shadowed }
        return value
    }
    private func failure(_ message: String) -> T3Failure { T3Failure(kind: "CliCommand", message: message) }

    @discardableResult func installLauncher() throws -> String {
        Self.lock.lock(); defer { Self.lock.unlock() }
        guard supported else { throw failure("The t3 command needs an installed app.") }
        do {
            try ensureRuntime()
            let fs = FileManager.default
            if fs.fileExists(atPath: launcher), (try? String(contentsOfFile: launcher, encoding: .utf8).contains(Self.marker)) != true {
                throw failure("Could not set up the t3 launcher at \(launcher).")
            }
            if (try? String(contentsOfFile: launcher, encoding: .utf8)) != script {
                try fs.createDirectory(at: home.appendingPathComponent("bin"), withIntermediateDirectories: true)
                let staging = "\(launcher).\(getpid()).tmp"
                defer { try? fs.removeItem(atPath: staging) }
                try Data(script.utf8).write(to: URL(fileURLWithPath: staging))
                try fs.setAttributes([.posixPermissions: 0o755], ofItemAtPath: staging)
                guard rename(staging, launcher) == 0 else { throw failure("Could not set up the t3 launcher at \(launcher).") }
            }
            return launcher
        } catch { throw failure("Could not set up the t3 launcher at \(launcher).") }
    }

    func install() throws -> [String: Any] {
        Self.lock.lock(); defer { Self.lock.unlock() }
        _ = try installLauncher()
        let fs = FileManager.default
        if let installed {
            if (try? fs.destinationOfSymbolicLink(atPath: installed)) == launcher { return state() }
            do { try fs.removeItem(atPath: installed) } catch { throw failure("Could not replace \(installed).") }
        }
        if let shadowed {
            throw failure("Another t3 at \(shadowed) runs first in a new terminal. Remove it, or run the launcher directly at \(launcher).")
        }
        let entries = path.split(separator: ":").map(String.init)
        for directory in candidates.filter({ entries.contains($0) }) + candidates.filter({ !entries.contains($0) }) {
            do {
                if fs.fileExists(atPath: directory) {
                    guard fs.isWritableFile(atPath: directory) else { continue }
                } else { try fs.createDirectory(atPath: directory, withIntermediateDirectories: true) }
                try fs.createSymbolicLink(atPath: (directory as NSString).appendingPathComponent("t3"), withDestinationPath: launcher)
                return state()
            } catch { continue }
        }
        throw failure("Another t3 command is already installed, or no folder on your PATH is writable. Run the launcher directly at \(launcher).")
    }
    func uninstall() throws -> [String: Any] {
        Self.lock.lock(); defer { Self.lock.unlock() }
        guard supported else { throw failure("The t3 command needs an installed app.") }
        for directory in candidates {
            let link = (directory as NSString).appendingPathComponent("t3")
            if isOurLink(link) {
                do { try FileManager.default.removeItem(atPath: link) } catch { throw failure("Could not remove \(link).") }
            }
        }
        return state()
    }
}


extension T3LocalBackend {
    func cliCommand() -> T3CliCommand {
        let env = environment()
        let supported = T3LocalPolicy.packaged(info: bundleInfo())
        let accountHome = NSHomeDirectory()
        let home = URL(fileURLWithPath: T3LocalShellEnvironment.trimmed(env["T3CODE_HOME"]) ?? "\(accountHome)/.t3", isDirectory: true)
        let runtime = T3LocalRuntime(home: home, bundleDir: resources()?.appendingPathComponent("t3-runtime", isDirectory: true))
        let version = runtime.pin()?.version ?? ""
        let executable = T3LocalRuntime.paths(home: home, version: version).entry.path
        return T3CliCommand(supported: !version.isEmpty && supported,
                            home: home, accountHome: accountHome, executable: executable,
                            appExecutable: Bundle.main.executableURL?.path ?? "", path: (supported ? serverEnvironment(env).env : env)["PATH"] ?? "") {
            switch runtime.ensure() {
            case .ready: return
            case let .missing(message), let .failed(message): throw T3Failure(kind: "CliCommand", message: message)
            }
        }
    }
}
