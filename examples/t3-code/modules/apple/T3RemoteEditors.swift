// Remote open-in-editor on this Mac (MIT reference, see LICENSE-T3:
// apps/desktop/src/ipc/methods/window.ts probeRemoteEditors, packages/shared/src/editor.ts
// resolveEditorCommand (macOS branch), apps/desktop/src/electron/ElectronShell.ts
// parseSafeExternalUrl / openExternal, apps/web/src/remoteOpen.ts useRemoteOpenHint, at
// 9c161462c9). The probe lists the remote-capable editors whose command resolves on the
// login shell's PATH, inside an app bundle in ~/Applications or /Applications (a JetBrains
// Toolbox bundle carries its version, `IntelliJ IDEA 2026.1.4.app`), or among Toolbox's
// scripts; only an http(s) URL or a remote editor deep link with no user info reaches
// NSWorkspace. Agent runs never open anything: the URL is recorded (T3_REMOTE_OPEN_LOG names a
// file to append it to), and T3_EDITOR_PATH / T3_EDITOR_APP_ROOTS / T3_EDITOR_HOME replace the
// PATH, the app folders and the home whose Toolbox scripts are read (none without it).
import Foundation
import AppKit

enum T3RemoteEditors {
    struct Editor {
        let id: String; let label: String; let commands: [String]; let installNames: [String]
        /// A JetBrains IDE (`launchStyle: "line-column"`): its bundle's CLI is `Contents/MacOS/<command>`.
        var jetbrains = false
    }
    private static func jetbrains(_ id: String, _ label: String, _ installNames: [String]? = nil) -> Editor {
        Editor(id: id, label: label, commands: [id], installNames: installNames ?? [label], jetbrains: true)
    }
    /// REMOTE_CAPABLE_EDITOR_IDS in EDITORS order, with resolveEditorCommand's install names.
    static let remoteCapable: [Editor] = [
        Editor(id: "cursor", label: "Cursor", commands: ["cursor"], installNames: ["Cursor"]),
        Editor(id: "vscode", label: "VS Code", commands: ["code"], installNames: ["Visual Studio Code"]),
        Editor(id: "vscode-insiders", label: "VS Code Insiders", commands: ["code-insiders"], installNames: ["Visual Studio Code - Insiders"]),
        Editor(id: "vscodium", label: "VSCodium", commands: ["codium"], installNames: ["VSCodium"]),
        Editor(id: "zed", label: "Zed", commands: ["zed", "zeditor"], installNames: ["Zed"]),
        jetbrains("idea", "IntelliJ IDEA", ["IntelliJ IDEA", "IntelliJ IDEA CE", "IntelliJ IDEA Ultimate"]), jetbrains("aqua", "Aqua"),
        jetbrains("clion", "CLion"), jetbrains("datagrip", "DataGrip"), jetbrains("dataspell", "DataSpell"), jetbrains("goland", "GoLand"),
        jetbrains("phpstorm", "PhpStorm"), jetbrains("pycharm", "PyCharm", ["PyCharm", "PyCharm CE"]), jetbrains("rider", "Rider", ["Rider", "JetBrains Rider"]),
        jetbrains("rubymine", "RubyMine"), jetbrains("rustrover", "RustRover"), jetbrains("webstorm", "WebStorm"),
    ]
    static let remoteSchemes: [String: String] = Dictionary(uniqueKeysWithValues: remoteCapable.map { ($0.id, $0.jetbrains ? "jetbrains" : $0.id) })

    private static let lock = NSLock()
    private static var recorded: [String] = []
    private static var agentHintSeen = false
    private static var cachedLoginPath: String?

    /// ElectronShell.openExternal for module code off the main thread (T3CodexAuth.swift): an agent
    /// run records the URL as `remoteEditorsOpen` does; otherwise NSWorkspace opens it.
    static func openOrRecord(_ value: String, agent: Bool) -> Bool {
        guard let url = safeExternalUrl(value) else { return false }
        if agent {
            lock.lock(); recorded.append(url); lock.unlock()
            if let log = ProcessInfo.processInfo.environment["T3_REMOTE_OPEN_LOG"], !log.isEmpty { append(url + "\n", to: log) }
            return true
        }
        return DispatchQueue.main.sync { URL(string: url).map { NSWorkspace.shared.open($0) } ?? false }
    }

    static func perform(_ request: [String: Any], agent: Bool, completion: @escaping ([String: Any]) -> Void) {
        let op = request["op"] as? String ?? ""
        let environment = ProcessInfo.processInfo.environment
        DispatchQueue.global(qos: .userInitiated).async {
            switch op {
            case "remoteEditorsProbe":
                let path = agent ? (environment["T3_EDITOR_PATH"] ?? environment["PATH"] ?? "") : loginShellPath(environment)
                let roots = (agent ? environment["T3_EDITOR_APP_ROOTS"] : nil)?.split(separator: ":").map(String.init) ?? defaultAppRoots(home: NSHomeDirectory())
                let home = agent ? environment["T3_EDITOR_HOME"] : NSHomeDirectory()
                completion(T3Ssh.success(["editors": probe(path: path, appRoots: roots, home: home)]))
            case "remoteEditorsOpen":
                guard let url = safeExternalUrl(request["url"]) else { completion(T3Ssh.success(["opened": false])); return }
                if agent {
                    lock.lock(); recorded.append(url); lock.unlock()
                    if let log = environment["T3_REMOTE_OPEN_LOG"], !log.isEmpty { append(url + "\n", to: log) }
                    completion(T3Ssh.success(["opened": true, "recorded": true]))
                    return
                }
                DispatchQueue.main.async {
                    let opened = URL(string: url).map { NSWorkspace.shared.open($0) } ?? false
                    completion(T3Ssh.success(["opened": opened]))
                }
            case "remoteEditorsHint":
                let key = "t3.remoteOpen.hintSeen"
                if request["seen"] as? Bool == true {
                    if agent { lock.lock(); agentHintSeen = true; lock.unlock() } else { UserDefaults.standard.set(true, forKey: key) }
                }
                lock.lock(); let seen = agent ? agentHintSeen : UserDefaults.standard.bool(forKey: key); lock.unlock()
                completion(T3Ssh.success(["seen": seen]))
            case "remoteEditorsOpened":
                lock.lock(); let urls = recorded; lock.unlock()
                completion(T3Ssh.success(["urls": urls]))
            default:
                completion(T3Ssh.failure(T3Failure(kind: "Arguments", message: "Unknown remote editor operation.")))
            }
        }
    }

    static func defaultAppRoots(home: String) -> [String] { [(home as NSString).appendingPathComponent("Applications"), "/Applications"] }

    /// probeRemoteEditors: the remote-capable editors that resolve here, in EDITORS order.
    static func probe(path: String, appRoots: [String], home: String? = nil) -> [String] {
        remoteCapable.filter { resolves($0, path: path, appRoots: appRoots, home: home) }.map(\.id)
    }

    /// `<name> <version>.app`; a plain prefix match would let stable VS Code take `Visual Studio Code - Insiders.app`.
    static func isVersionedBundle(_ entry: String, _ name: String) -> Bool {
        guard entry.hasPrefix("\(name) "), entry.hasSuffix(".app") else { return false }
        let version = entry.dropFirst(name.count + 1).dropLast(".app".count)
        return version.first?.isNumber == true && version.allSatisfy { $0.isNumber || $0 == "." }
    }

    /// resolveEditorCommand's macOS branch: a command on PATH, else the app bundle's CLI, else a JetBrains
    /// Toolbox script.
    static func resolves(_ editor: Editor, path: String, appRoots: [String], home: String? = nil) -> Bool {
        let directories = path.split(separator: ":").map(String.init).filter { !$0.isEmpty }
        let onPath = { editor.commands.contains { command in directories.contains { executable(($0 as NSString).appendingPathComponent(command)) } } }
        if onPath() { return true }
        let command = editor.commands[0]
        var candidates: [String] = []
        for root in appRoots {
            let entries = (try? FileManager.default.contentsOfDirectory(atPath: root)) ?? []
            var bundles = editor.installNames.map { "\($0).app" }
            for entry in entries where !bundles.contains(entry) && editor.installNames.contains(where: { isVersionedBundle(entry, $0) }) { bundles.append(entry) }
            for bundle in bundles {
                let contents = ((root as NSString).appendingPathComponent(bundle) as NSString).appendingPathComponent("Contents")
                candidates += editor.jetbrains || editor.id == "zed" ? [(contents as NSString).appendingPathComponent("MacOS/\(editor.id == "zed" ? "cli" : command)")]
                    : [(contents as NSString).appendingPathComponent("Resources/app/bin/\(command)"), (contents as NSString).appendingPathComponent("Resources/app/bin/code")]
            }
        }
        if let home, editor.jetbrains { candidates.append((home as NSString).appendingPathComponent("Library/Application Support/JetBrains/Toolbox/scripts/\(command)")) }
        return candidates.contains(where: executable)
    }

    static func executable(_ path: String) -> Bool {
        var directory: ObjCBool = false
        return FileManager.default.fileExists(atPath: path, isDirectory: &directory) && !directory.boolValue && FileManager.default.isExecutableFile(atPath: path)
    }

    /// The login shell's PATH (an app started from Finder has only launchd's), read once.
    static func loginShellPath(_ environment: [String: String]) -> String {
        lock.lock(); if let cached = cachedLoginPath { lock.unlock(); return cached }; lock.unlock()
        let fallback = environment["PATH"] ?? "/usr/bin:/bin:/usr/sbin:/sbin"
        let shell = environment["SHELL"].flatMap { $0.hasPrefix("/") ? $0 : nil } ?? "/bin/zsh"
        let process = Process(), output = Pipe()
        process.executableURL = URL(fileURLWithPath: shell)
        process.arguments = ["-ilc", "printf '%s' \"$PATH\""]
        process.standardOutput = output; process.standardError = FileHandle.nullDevice; process.standardInput = FileHandle.nullDevice
        var value = fallback
        if (try? process.run()) != nil {
            let done = DispatchSemaphore(value: 0)
            var data = Data()
            DispatchQueue.global().async { data = output.fileHandleForReading.readDataToEndOfFile(); done.signal() }
            if done.wait(timeout: .now() + 3) == .timedOut { process.terminate() }
            let text = String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
            if !text.isEmpty, !text.contains("\n") { value = text + ":" + fallback }
        }
        lock.lock(); cachedLoginPath = value; lock.unlock()
        return value
    }

    /// parseSafeExternalUrl: http(s), or `<scheme>://vscode-remote/ssh-remote+<host>…` /
    /// `zed://ssh/<host>/…` / `jetbrains://gateway/ssh/environment?h=<host>…` with no user info;
    /// anything else is refused.
    static func safeExternalUrl(_ raw: Any?) -> String? {
        guard let raw = raw as? String, let components = URLComponents(string: raw), let scheme = components.scheme?.lowercased() else { return nil }
        if scheme == "http" || scheme == "https" { return (components.host ?? "").isEmpty ? nil : components.url?.absoluteString }
        guard remoteSchemes.values.contains(scheme), (components.user ?? "").isEmpty, (components.password ?? "").isEmpty else { return nil }
        let host = components.percentEncodedHost ?? "", path = components.percentEncodedPath
        if scheme == "zed" {
            guard host == "ssh", path.range(of: #"^/[^/@:]+/.*$"#, options: .regularExpression) != nil else { return nil }
        } else if scheme == "jetbrains" {
            guard host == "gateway", path == "/ssh/environment", !(components.queryItems?.first(where: { $0.name == "h" })?.value ?? "").isEmpty else { return nil }
        } else {
            guard host == "vscode-remote", path.hasPrefix("/ssh-remote+"), path.count > "/ssh-remote+".count else { return nil }
        }
        return components.url?.absoluteString
    }

    private static func append(_ line: String, to path: String) {
        if !FileManager.default.fileExists(atPath: path) { FileManager.default.createFile(atPath: path, contents: nil) }
        guard let handle = FileHandle(forWritingAtPath: path) else { return }
        handle.seekToEndOfFile(); handle.write(Data(line.utf8)); try? handle.close()
    }
}
