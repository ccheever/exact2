// Remote open-in-editor on this Mac (MIT reference, see LICENSE-T3:
// apps/desktop/src/ipc/methods/window.ts probeRemoteEditors, packages/shared/src/editor.ts
// resolveEditorCommand (macOS branch), apps/desktop/src/electron/ElectronShell.ts
// parseSafeExternalUrl / openExternal, apps/web/src/remoteOpen.ts useRemoteOpenHint, at
// 1e2ecbd975). The probe lists the remote-capable editors whose command resolves on the
// login shell's PATH or inside an app bundle in ~/Applications or /Applications; only an
// http(s) URL or a remote editor deep link with no user info reaches NSWorkspace. Agent
// runs never open anything: the URL is recorded (T3_REMOTE_OPEN_LOG names a file to append
// it to), and T3_EDITOR_PATH / T3_EDITOR_APP_ROOTS replace the PATH and the app folders.
import Foundation
import AppKit

enum T3RemoteEditors {
    struct Editor { let id: String; let label: String; let commands: [String]; let installNames: [String] }
    /// REMOTE_CAPABLE_EDITOR_IDS in EDITORS order, with resolveEditorCommand's install names.
    static let remoteCapable: [Editor] = [
        Editor(id: "cursor", label: "Cursor", commands: ["cursor"], installNames: ["Cursor"]),
        Editor(id: "vscode", label: "VS Code", commands: ["code"], installNames: ["Visual Studio Code"]),
        Editor(id: "vscode-insiders", label: "VS Code Insiders", commands: ["code-insiders"], installNames: ["Visual Studio Code - Insiders"]),
        Editor(id: "vscodium", label: "VSCodium", commands: ["codium"], installNames: ["VSCodium"]),
        Editor(id: "zed", label: "Zed", commands: ["zed", "zeditor"], installNames: ["Zed"]),
    ]
    static let remoteSchemes: [String: String] = ["cursor": "cursor", "vscode": "vscode", "vscode-insiders": "vscode-insiders", "vscodium": "vscodium", "zed": "zed"]

    private static let lock = NSLock()
    private static var recorded: [String] = []
    private static var agentHintSeen = false
    private static var cachedLoginPath: String?

    static func perform(_ request: [String: Any], agent: Bool, completion: @escaping ([String: Any]) -> Void) {
        let op = request["op"] as? String ?? ""
        let environment = ProcessInfo.processInfo.environment
        DispatchQueue.global(qos: .userInitiated).async {
            switch op {
            case "remoteEditorsProbe":
                let path = agent ? (environment["T3_EDITOR_PATH"] ?? environment["PATH"] ?? "") : loginShellPath(environment)
                let roots = (agent ? environment["T3_EDITOR_APP_ROOTS"] : nil)?.split(separator: ":").map(String.init) ?? defaultAppRoots(home: NSHomeDirectory())
                completion(T3Ssh.success(["editors": probe(path: path, appRoots: roots)]))
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
    static func probe(path: String, appRoots: [String]) -> [String] {
        remoteCapable.filter { resolves($0, path: path, appRoots: appRoots) }.map(\.id)
    }

    /// resolveEditorCommand's macOS branch: a command on PATH, else the app bundle's CLI.
    static func resolves(_ editor: Editor, path: String, appRoots: [String]) -> Bool {
        let directories = path.split(separator: ":").map(String.init).filter { !$0.isEmpty }
        for command in editor.commands where directories.contains(where: { executable(($0 as NSString).appendingPathComponent(command)) }) { return true }
        let command = editor.commands[0]
        for root in appRoots {
            for name in editor.installNames {
                let contents = ((root as NSString).appendingPathComponent("\(name).app") as NSString).appendingPathComponent("Contents")
                let candidates = editor.id == "zed" ? [(contents as NSString).appendingPathComponent("MacOS/cli")]
                    : [(contents as NSString).appendingPathComponent("Resources/app/bin/\(command)"), (contents as NSString).appendingPathComponent("Resources/app/bin/code")]
                if candidates.contains(where: executable) { return true }
            }
        }
        return false
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
    /// `zed://ssh/<host>/…` with no user info; anything else is refused.
    static func safeExternalUrl(_ raw: Any?) -> String? {
        guard let raw = raw as? String, let components = URLComponents(string: raw), let scheme = components.scheme?.lowercased() else { return nil }
        if scheme == "http" || scheme == "https" { return (components.host ?? "").isEmpty ? nil : components.url?.absoluteString }
        guard remoteSchemes.values.contains(scheme), (components.user ?? "").isEmpty, (components.password ?? "").isEmpty else { return nil }
        let host = components.percentEncodedHost ?? "", path = components.percentEncodedPath
        if scheme == "zed" {
            guard host == "ssh", path.range(of: #"^/[^/@:]+/.*$"#, options: .regularExpression) != nil else { return nil }
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
