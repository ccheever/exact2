// Ocho's native module (LLP 1024, LLP 1067.000): everything that reaches
// outside the process, and nothing that decides. The model is the Rust data
// source's (`data/`, a port of the GPUI desktop's workspace); this module
//
//   - runs the fleet feed, `fleet snapshot --watch`, and hands its raw
//     NDJSON events to the model in the order they arrived (`feed`), one
//     announcement of the `feed` topic per batch;
//   - mirrors `$FLEET_HOME/desktop.json` (`desktop`), the file the existing
//     Ocho and this one share, and writes it when the model asks;
//   - runs the model's jobs (`io`): `fleet` commands with a 60 s timeout, and
//     the few host jobs (the window's title, the clipboard, a URL, a
//     notification);
//   - shows each tab's terminal, a `<ghostty-terminal>` view running the
//     argv the model names (Terminal.swift owns the libghostty surfaces).
//
// Requests arrive as `native.later` bodies `{op, …}` from the Rust source.
import Foundation
import AppKit
import UserNotifications

/// The module the host instantiates (ExactNativeModule.swift reads this name).
let exactModule: ExactModule.Type = OchoModule.self

final class OchoModule: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["ghostty-terminal": ExactNativeFactory(for: OchoModule.self) { module, props, events in
            try TerminalInstance(module: module, props: props, events: events)
        }]
    }

    let fleet: FleetFeed
    let desktop: DesktopMirror
    let jobs: JobRunner
    lazy var terminals = TerminalStore(module: self)

    required init(context: ExactModuleContext) {
        let binary = FleetBinary.locate()
        fleet = FleetFeed(binary: binary)
        desktop = DesktopMirror()
        jobs = JobRunner(binary: binary)
        super.init(context: context)
        fleet.onEvents = { [weak self] in self?.context.changed("feed") }
        desktop.onChange = { [weak self] in self?.context.changed("desktop") }
        fleet.start()
        desktop.start()
        DispatchQueue.main.async { [weak self] in
            WindowChrome.apply()
            if let self { OchoMenu.install(feed: self.fleet) }
            // Answers made before this module loaded settled empty; now
            // there is someone to ask.
            self?.context.changed("feed")
            self?.context.changed("desktop")
        }
    }

    override func destroy() {
        fleet.stop()
        desktop.stop()
        terminals.destroyAll()
    }

    override func later(_ request: [String: Any], reply: ExactReply) {
        let op = request["op"] as? String ?? ""
        switch op {
        case "feed":
            let events = fleet.take()
            var answer: [String: Any] = ["events": events, "now": Date().timeIntervalSince1970, "tz": TimeZone.current.secondsFromGMT()]
            if let message = fleet.failure { answer["failure"] = message }
            reply.send(answer)
        case "desktop":
            reply.send(["text": desktop.text, "mtime": desktop.mtime, "home": FleetHome.path, "now": Date().timeIntervalSince1970, "tz": TimeZone.current.secondsFromGMT(),
                        "read": true, "exists": FileManager.default.fileExists(atPath: desktop.url.path)])
        case "io":
            let jobs = request["jobs"] as? [[String: Any]] ?? []
            self.jobs.run(jobs, fleet: fleet, desktop: desktop, terminals: terminals) { replies in reply.send(["replies": replies, "now": Date().timeIntervalSince1970, "tz": TimeZone.current.secondsFromGMT()]) }
        default:
            reply.fail("Ocho answers no \(op)")
        }
    }
}

// MARK: - The fleet binary and home

enum FleetBinary {
    /// Where the `fleet` CLI is, in the order the existing desktop looks:
    /// `$FLEET_BIN`, the installed Ocho's copy, `~/.local/bin`, then PATH.
    static func locate() -> String? {
        let env = ProcessInfo.processInfo.environment
        var candidates: [String] = []
        if let bin = env["FLEET_BIN"], !bin.isEmpty { candidates.append(bin) }
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        candidates.append("\(home)/Applications/Ocho.app/Contents/Resources/bin/fleet")
        candidates.append("/Applications/Ocho.app/Contents/Resources/bin/fleet")
        candidates.append("\(home)/.local/bin/fleet")
        candidates.append("\(home)/Developer/fleet/bin/fleet")
        for dir in (env["PATH"] ?? "").split(separator: ":") { candidates.append("\(dir)/fleet") }
        return candidates.first { FileManager.default.isExecutableFile(atPath: $0) }
    }
}

enum FleetHome {
    /// `$FLEET_HOME`, else `~/.local/share/fleet` (settings.rs).
    static var path: String {
        if let home = ProcessInfo.processInfo.environment["FLEET_HOME"], !home.isEmpty { return home }
        return FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".local/share/fleet").path
    }
}

// MARK: - The feed

/// `fleet snapshot --watch`, its NDJSON events kept in arrival order until
/// the model takes them (feed.rs, without the merging: the model merges).
final class FleetFeed {
    let binary: String?
    private var process: Process?
    private var stdin: FileHandle?
    private let queue = DispatchQueue(label: "ocho.fleet", qos: .utility)
    private var pending: [[String: Any]] = []
    private var announced = false
    private(set) var failure: String?
    private var stopping = false
    private var backoff: Double = 2
    private var startedAt = Date()
    /// The mobile API this app started (serve.rs `Running`): what it
    /// announced, while it runs. The feed follows its event stream.
    private(set) var serveInfo: String?
    private var server: Process?
    /// The followed process, readable off the main thread.
    private let followLock = NSLock()
    private var followedId: ObjectIdentifier?
    private func follow(_ p: Process?) {
        followLock.lock(); followedId = p.map(ObjectIdentifier.init); followLock.unlock()
    }
    var onEvents: (() -> Void)?

    init(binary: String?) { self.binary = binary }

    /// main.rs: the mobile API starts with the app, and the watcher covers
    /// the windows until it announces itself.
    func start() {
        startServer()
        startWatcher()
    }

    private func spawn(_ arguments: [String], firstLine: ((Data) -> Bool)? = nil, onExit: @escaping (Process) -> Void) -> (Process, FileHandle)? {
        guard let binary else { return nil }
        let p = Process()
        p.executableURL = URL(fileURLWithPath: binary)
        p.arguments = arguments
        var env = ProcessInfo.processInfo.environment
        env["FLEET_HOME"] = FleetHome.path
        p.environment = env
        let out = Pipe(), inp = Pipe(), err = Pipe()
        p.standardOutput = out; p.standardInput = inp; p.standardError = err
        var buffer = Data()
        var first = firstLine
        out.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard let self else { return }
            if data.isEmpty { handle.readabilityHandler = nil; return }
            buffer.append(data)
            while let nl = buffer.firstIndex(of: 0x0A) {
                let line = buffer.subdata(in: buffer.startIndex..<nl)
                buffer.removeSubrange(buffer.startIndex...nl)
                if let check = first {
                    first = nil
                    if check(line) { continue }
                }
                self.queue.async { self.consume(line, from: p) }
            }
        }
        err.fileHandleForReading.readabilityHandler = { handle in _ = handle.availableData }
        p.terminationHandler = { proc in DispatchQueue.main.async { onExit(proc) } }
        do { try p.run() } catch { return nil }
        return (p, inp.fileHandleForWriting)
    }

    /// `fleet snapshot --watch`, restarted with feed.rs's backoff.
    private func startWatcher() {
        guard binary != nil else {
            failure = "The fleet CLI wasn't found. Install Ocho or set FLEET_BIN."
            DispatchQueue.main.async { self.onEvents?() }
            return
        }
        startedAt = Date()
        guard let (p, input) = spawn(["snapshot", "--watch"], onExit: { [weak self] proc in
            guard let self, !self.stopping, self.process === proc else { return }
            self.restart()
        }) else {
            failure = "Couldn't start fleet."
            DispatchQueue.main.async { self.onEvents?() }
            return
        }
        process = p
        follow(p)
        stdin = input
        failure = nil
    }

    /// serve.rs `start`: `fleet serve` dials the relay; its first line says
    /// what a phone needs, and its events then replace the watcher's. Without
    /// a relay secret, or with the port taken, it exits and the watcher stays.
    private func startServer() {
        guard server == nil else { return }
        let spawned = spawn(["serve", "--json", "--no-qr", "--follow-stdin", "--events"], firstLine: { [weak self] line in
            guard let self, let text = String(data: line, encoding: .utf8),
                  (try? JSONSerialization.jsonObject(with: line)) is [String: Any] else { return false }
            DispatchQueue.main.async { self.adopt(info: text) }
            return true
        }, onExit: { [weak self] proc in
            guard let self, !self.stopping, self.server === proc else { return }
            let wasFollowed = self.process === proc
            self.server = nil
            self.serveInfo = nil
            // feed.rs: the server was our watcher; bring both back.
            if wasFollowed { self.restart() }
        })
        if let (p, input) = spawned {
            server = p
            serverStdin = input
        }
    }
    private var serverStdin: FileHandle?

    /// feed.rs `adopt`: follow the server and retire the watcher.
    private func adopt(info: String) {
        guard let server, server.isRunning else { return }
        serveInfo = info
        let old = process
        process = server
        follow(server)
        stdin = serverStdin
        startedAt = Date()
        if let old, old !== server { old.terminate() }
    }

    private func restart() {
        // feed.rs:58-67: 2, 2, 4, 8 … capped at 60 s; 30 s healthy resets.
        if Date().timeIntervalSince(startedAt) > 30 { backoff = 2 }
        let wait = backoff
        backoff = min(60, backoff * 2)
        DispatchQueue.main.asyncAfter(deadline: .now() + wait) { [weak self] in
            guard let self, !self.stopping else { return }
            self.startServer()
            self.startWatcher()
        }
    }

    func stop() {
        stopping = true
        try? stdin?.close()
        try? serverStdin?.close()
        process?.terminate()
        server?.terminate()
    }

    /// Ask for a fresh round now (a `\n` on stdin wakes the watcher).
    func refresh() { try? stdin?.write(contentsOf: Data("\n".utf8)) }

    private func consume(_ line: Data, from source: Process) {
        // Only the followed process feeds the model; a retiring one is ignored.
        followLock.lock()
        let current = followedId == ObjectIdentifier(source)
        followLock.unlock()
        guard current, !line.isEmpty, let event = try? JSONSerialization.jsonObject(with: line) as? [String: Any] else { return }
        pending.append(event)
        if !announced {
            announced = true
            DispatchQueue.main.async { [weak self] in self?.onEvents?() }
        }
    }

    /// A menu bar item: delivered to the model as a `menu` event.
    func inject(_ event: [String: Any]) {
        queue.async {
            self.pending.append(event)
            if !self.announced {
                self.announced = true
                DispatchQueue.main.async { [weak self] in self?.onEvents?() }
            }
        }
    }

    /// The events since the last take, oldest first.
    func take() -> [[String: Any]] {
        queue.sync {
            let events = pending
            pending.removeAll()
            announced = false
            return events
        }
    }
}

// MARK: - desktop.json

/// The existing Ocho's `desktop.json`, read on change (a 2 s poll of its
/// mtime, as the GPUI app has no watcher either) and written on request.
final class DesktopMirror {
    private(set) var text = ""
    private(set) var mtime: Double = 0
    private var timer: Timer?
    private var lastWritten = ""
    var onChange: (() -> Void)?

    /// `$OCHO_DESKTOP_FILE` stands in for the real file (a drive's sandbox:
    /// closing a tab there must not touch the desktop.json the other Ocho keeps).
    var url: URL {
        if let path = ProcessInfo.processInfo.environment["OCHO_DESKTOP_FILE"], !path.isEmpty { return URL(fileURLWithPath: path) }
        return URL(fileURLWithPath: FleetHome.path).appendingPathComponent("desktop.json")
    }

    func start() {
        reload(announce: false)
        timer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in self?.reload(announce: true) }
    }

    func stop() { timer?.invalidate() }

    private func reload(announce: Bool) {
        let attrs = try? FileManager.default.attributesOfItem(atPath: url.path)
        let modified = (attrs?[.modificationDate] as? Date)?.timeIntervalSince1970 ?? 0
        guard modified != mtime else { return }
        mtime = modified
        let read = (try? String(contentsOf: url, encoding: .utf8)) ?? ""
        guard read != text else { return }
        text = read
        if announce && read != lastWritten { onChange?() }
    }

    /// Write the model's text, pretty as it came (settings.rs:90-131).
    func save(_ text: String) -> String? {
        do {
            try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
            try text.write(to: url, atomically: true, encoding: .utf8)
            lastWritten = text
            self.text = text
            mtime = (try? FileManager.default.attributesOfItem(atPath: url.path))?[.modificationDate].flatMap { ($0 as? Date)?.timeIntervalSince1970 } ?? mtime
            return nil
        } catch {
            return error.localizedDescription
        }
    }
}

// MARK: - Jobs

/// The model's jobs: `fleet` commands run headless (backend.rs: 60 s
/// timeout, `FLEET_HOME` set), and the host's own small jobs.
final class JobRunner {
    let binary: String?
    private let queue = DispatchQueue(label: "ocho.jobs", qos: .userInitiated, attributes: .concurrent)

    init(binary: String?) { self.binary = binary }

    func run(_ jobs: [[String: Any]], fleet: FleetFeed, desktop: DesktopMirror, terminals: TerminalStore, done: @escaping ([[String: Any]]) -> Void) {
        let group = DispatchGroup()
        var replies: [[String: Any]] = Array(repeating: [:], count: jobs.count)
        let lock = NSLock()
        for (i, job) in jobs.enumerated() {
            let id = job["id"] as? NSNumber ?? 0
            let kind = job["kind"] as? String ?? ""
            let argv = job["argv"] as? [String] ?? []
            let stdin = job["stdin"] as? String ?? ""
            group.enter()
            let finish: ([String: Any]) -> Void = { extra in
                var reply: [String: Any] = ["id": id, "kind": kind]
                for (k, v) in extra { reply[k] = v }
                lock.lock(); replies[i] = reply; lock.unlock()
                group.leave()
            }
            switch argv.first {
            case "serve-running":
                // serve.rs `info`: the server this app started, when it runs.
                DispatchQueue.main.async { finish(["status": 0, "stderr": "", "stdout": fleet.serveInfo ?? ""]) }
            case "whats-new-history":
                // The history this port follows: Fleet's first-parent log at
                // $OCHO_FLEET_COMMIT (default origin/main) in $OCHO_FLEET_REPO
                // (default ~/Developer/fleet); empty when there is no checkout.
                queue.async {
                    let env = ProcessInfo.processInfo.environment
                    let repo = env["OCHO_FLEET_REPO"] ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Developer/fleet").path
                    let commit = env["OCHO_FLEET_COMMIT"] ?? "origin/main"
                    let p = Process()
                    p.executableURL = URL(fileURLWithPath: "/usr/bin/git")
                    p.arguments = ["-C", repo, "log", "--first-parent", "-60", "--format=%H%x09%cs%x09%s", commit]
                    let out = Pipe(); p.standardOutput = out; p.standardError = Pipe()
                    var text = ""
                    if (try? p.run()) != nil {
                        let data = out.fileHandleForReading.readDataToEndOfFile()
                        p.waitUntilExit()
                        if p.terminationStatus == 0 { text = String(decoding: data, as: UTF8.self) }
                    }
                    finish(["status": 0, "stderr": "", "stdout": text])
                }
            case "theme-files":
                // themes.rs `theme_dirs`: Zed's themes, its extensions' themes, Fleet's own.
                queue.async {
                    let fm = FileManager.default
                    let home = fm.homeDirectoryForCurrentUser.path
                    var dirs = ["\(home)/.config/zed/themes", "\(FleetHome.path)/themes", "\(home)/.local/share/fleet/themes"]
                    let extensions = "\(home)/Library/Application Support/Zed/extensions/installed"
                    for ext in (try? fm.contentsOfDirectory(atPath: extensions)) ?? [] { dirs.append("\(extensions)/\(ext)/themes") }
                    var files: [[String]] = []
                    for dir in dirs {
                        for name in ((try? fm.contentsOfDirectory(atPath: dir)) ?? []).sorted() where name.hasSuffix(".json") {
                            let path = "\(dir)/\(name)"
                            if let text = try? String(contentsOfFile: path, encoding: .utf8) { files.append([path, text]) }
                        }
                    }
                    // theme.rs `initial_theme` also reads FLEET_THEME, Zed's
                    // settings and the system appearance.
                    let zed = (try? String(contentsOfFile: "\(home)/.config/zed/settings.json", encoding: .utf8)) ?? ""
                    let env = ProcessInfo.processInfo.environment["FLEET_THEME"] ?? ""
                    DispatchQueue.main.async {
                        let dark = NSApp.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
                        let body: [String: Any] = ["files": files, "zed": zed, "env": env, "dark": dark]
                        let json = (try? JSONSerialization.data(withJSONObject: body)).map { String(decoding: $0, as: UTF8.self) } ?? "[]"
                        finish(["status": 0, "stderr": "", "stdout": json])
                    }
                }
            case "refresh-feed":
                fleet.refresh()
                finish(["status": 0, "stderr": "", "stdout": ""])
            case "quit":
                DispatchQueue.main.async { NSApp.terminate(nil) }
                finish(["status": 0, "stderr": "", "stdout": ""])
            case "close-window":
                DispatchQueue.main.async {
                    NSApp.keyWindow?.performClose(nil)
                    finish(["status": 0, "stderr": "", "stdout": ""])
                }
            case "save-title-prompt":
                // title_settings.rs `save_to`: a private temp file renamed into place.
                let dir = URL(fileURLWithPath: FleetHome.path)
                let url = dir.appendingPathComponent("titles.json")
                let temp = dir.appendingPathComponent("titles.\(UUID().uuidString).tmp")
                do {
                    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
                    guard FileManager.default.createFile(atPath: temp.path, contents: Data(stdin.utf8), attributes: [.posixPermissions: 0o600]) else {
                        throw CocoaError(.fileWriteUnknown)
                    }
                    guard rename(temp.path, url.path) == 0 else { throw CocoaError(.fileWriteUnknown) }
                    finish(["status": 0, "stderr": "", "stdout": ""])
                } catch {
                    try? FileManager.default.removeItem(at: temp)
                    finish(["status": 1, "stderr": error.localizedDescription, "stdout": ""])
                }
            case "retry-connection":
                DispatchQueue.main.async {
                    if argv.count > 1 { terminals.retryConnection(tab: argv[1]) }
                    finish(["status": 0, "stderr": "", "stdout": ""])
                }
            case "clear-terminal", "reconnect-terminal":
                DispatchQueue.main.async {
                    if argv.count > 1 { argv[0] == "clear-terminal" ? terminals.clear(tab: argv[1]) : terminals.reconnect(tab: argv[1]) }
                    finish(["status": 0, "stderr": "", "stdout": ""])
                }
            case "save-desktop":
                DispatchQueue.main.async {
                    if let error = desktop.save(stdin) { finish(["status": 1, "stderr": error, "stdout": ""]) }
                    else { finish(["status": 0, "stderr": "", "stdout": ""]) }
                }
            case "window-title":
                DispatchQueue.main.async {
                    NSApp.windows.first { $0.isVisible }?.title = argv.dropFirst().joined(separator: " ")
                    finish(["status": 0, "stderr": "", "stdout": ""])
                }
            case "open-url":
                DispatchQueue.main.async {
                    if let url = URL(string: argv.dropFirst().joined()) { NSWorkspace.shared.open(url) }
                    finish(["status": 0, "stderr": "", "stdout": ""])
                }
            case "clipboard-write":
                DispatchQueue.main.async {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(stdin, forType: .string)
                    finish(["status": 0, "stderr": "", "stdout": ""])
                }
            case "clipboard-read":
                DispatchQueue.main.async {
                    finish(["status": 0, "stderr": "", "stdout": NSPasteboard.general.string(forType: .string) ?? ""])
                }
            case "notify":
                DispatchQueue.main.async {
                    // notifications.rs: nothing while the app is in front.
                    if !NSApp.isActive { Notifier.post(title: argv.count > 1 ? argv[1] : "Ocho", body: stdin, thread: argv.count > 2 ? argv[2] : "") }
                    finish(["status": 0, "stderr": "", "stdout": ""])
                }
            case "notify-remove":
                Notifier.remove(thread: argv.count > 1 ? argv[1] : "")
                finish(["status": 0, "stderr": "", "stdout": ""])
            case "close-terminal":
                DispatchQueue.main.async {
                    if argv.count > 1 { terminals.close(tab: argv[1]) }
                    finish(["status": 0, "stderr": "", "stdout": ""])
                }
            case "submit-terminal":
                DispatchQueue.main.async {
                    guard argv.count > 1 else { finish(["status": 1, "stderr": "not connected", "stdout": ""]); return }
                    terminals.submit(tab: argv[1], text: stdin) { outcome in
                        if let outcome { finish(["status": 0, "stderr": "", "stdout": outcome]) }
                        else { finish(["status": 1, "stderr": "not connected", "stdout": ""]) }
                    }
                }
            case "write-terminal":
                DispatchQueue.main.async {
                    if argv.count > 1 { terminals.write(tab: argv[1], text: stdin) }
                    finish(["status": 0, "stderr": "", "stdout": ""])
                }
            default:
                queue.async { [binary] in
                    guard let binary else { return finish(["status": 127, "stderr": "The fleet CLI wasn't found. Install Ocho or set FLEET_BIN.", "stdout": ""]) }
                    let timeout = (job["timeout"] as? NSNumber)?.doubleValue ?? 0
                    finish(Self.fleet(binary: binary, argv: argv, stdin: stdin, timeout: timeout > 0 ? timeout : 60))
                }
            }
        }
        group.notify(queue: .main) { done(replies) }
    }

    /// One `fleet` command to completion, or killed at 60 s (backend.rs).
    /// backend.rs `run_bounded`: one deadline for the whole command (60 s,
    /// or the job's own), stdout and stderr drained together so neither pipe
    /// can stall it. A command still running at the deadline is killed and
    /// reported, so a hung CLI cannot leave a spinner up for good.
    static func fleet(binary: String, argv: [String], stdin: String, timeout: Double = 60) -> [String: Any] {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: binary)
        p.arguments = argv
        var env = ProcessInfo.processInfo.environment
        env["FLEET_HOME"] = FleetHome.path
        p.environment = env
        let out = Pipe(), err = Pipe(), inp = Pipe()
        p.standardOutput = out; p.standardError = err; p.standardInput = inp
        let exited = DispatchSemaphore(value: 0)
        p.terminationHandler = { _ in exited.signal() }
        do { try p.run() } catch { return ["status": 127, "stderr": error.localizedDescription, "stdout": ""] }
        final class Box { var data = Data() }
        let stdoutBox = Box(), stderrBox = Box()
        let drained = DispatchGroup()
        for (pipe, box) in [(out, stdoutBox), (err, stderrBox)] {
            drained.enter()
            DispatchQueue.global().async { box.data = pipe.fileHandleForReading.readDataToEndOfFile(); drained.leave() }
        }
        if !stdin.isEmpty { inp.fileHandleForWriting.write(Data(stdin.utf8)) }
        try? inp.fileHandleForWriting.close()
        if exited.wait(timeout: .now() + timeout) == .timedOut {
            p.terminate()
            _ = drained.wait(timeout: .now() + 2)
            let seconds = Int(timeout)
            return ["status": 124, "stderr": "fleet \(argv.first ?? "") timed out after \(seconds) s", "stdout": ""]
        }
        // A process it left behind may hold the pipes open; don't wait on it for long.
        _ = drained.wait(timeout: .now() + 2)
        return [
            "status": Int(p.terminationStatus),
            "stdout": String(decoding: stdoutBox.data, as: UTF8.self),
            "stderr": String(decoding: stderrBox.data, as: UTF8.self),
        ]
    }
}

// MARK: - Notifications

/// macOS notifications (notifications.rs): a completed turn or input
/// required, threaded by `machine:session`.
enum Notifier {
    private static var asked = false

    static func post(title: String, body: String, thread: String) {
        let center = UNUserNotificationCenter.current()
        if !asked {
            asked = true
            center.requestAuthorization(options: [.alert, .sound]) { _, _ in }
        }
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.sound = .default
        if !thread.isEmpty { content.threadIdentifier = thread }
        let id = UUID().uuidString
        if !thread.isEmpty { remove(thread: thread); posted[thread] = id }
        center.add(UNNotificationRequest(identifier: id, content: content, trigger: nil))
    }

    private static var posted: [String: String] = [:]

    /// The thread's notification goes (the session runs again).
    static func remove(thread: String) {
        guard let id = posted.removeValue(forKey: thread) else { return }
        UNUserNotificationCenter.current().removeDeliveredNotifications(withIdentifiers: [id])
        UNUserNotificationCenter.current().removePendingNotificationRequests(withIdentifiers: [id])
    }
}

// MARK: - Window chrome

/// windows.rs:43-47: no title bar, the traffic lights at (14, 14).
enum WindowChrome {
    static func apply() {
        guard let window = NSApp.windows.first(where: { $0.isVisible }) ?? NSApp.windows.first else { return }
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.styleMask.insert(.fullSizeContentView)
        window.title = "Ocho"
        window.isMovableByWindowBackground = false
        position(window)
        NotificationCenter.default.addObserver(forName: NSWindow.didResizeNotification, object: window, queue: .main) { _ in position(window) }
    }

    private static func position(_ window: NSWindow) {
        let buttons: [NSWindow.ButtonType] = [.closeButton, .miniaturizeButton, .zoomButton]
        var x: CGFloat = 14
        for kind in buttons {
            guard let button = window.standardWindowButton(kind), let bar = button.superview else { continue }
            var frame = button.frame
            frame.origin.x = x
            frame.origin.y = bar.bounds.height - 14 - frame.height
            button.setFrameOrigin(frame.origin)
            x += frame.width + 6
        }
    }
}

/// The menu bar (main.rs `set_menus`): Ocho, File, Edit, View, Help, as
/// upstream lays them out. An item reaches the model as a `menu` event
/// naming its command; chords keep going through the contract's declared
/// shortcuts, which the host routes before the menu. Edit stays with the
/// responder chain so text fields keep undo, cut, copy and paste.
final class OchoMenu: NSObject {
    private static let shared = OchoMenu()
    private weak var feed: FleetFeed?

    static func install(feed: FleetFeed) {
        shared.feed = feed
        // The host installs its own bar at launch; replace it once it has.
        DispatchQueue.main.async { NSApp.mainMenu = shared.build(develop: NSApp.mainMenu?.item(withTitle: "Develop")) }
    }

    @objc private func run(_ sender: NSMenuItem) {
        guard let id = sender.representedObject as? String else { return }
        feed?.inject(["type": "menu", "command": id])
    }

    private func item(_ title: String, _ command: String, _ key: String = "", _ mods: NSEvent.ModifierFlags = [.command]) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: #selector(run(_:)), keyEquivalent: key)
        item.keyEquivalentModifierMask = mods
        item.target = self
        item.representedObject = command
        return item
    }

    private func menu(_ title: String, _ items: [NSMenuItem]) -> NSMenuItem {
        let menu = NSMenu(title: title)
        items.forEach(menu.addItem)
        let holder = NSMenuItem(title: title, action: nil, keyEquivalent: "")
        holder.submenu = menu
        return holder
    }

    private func build(develop: NSMenuItem?) -> NSMenu {
        let bar = NSMenu()
        let services = NSMenu(title: "Services")
        NSApp.servicesMenu = services
        let servicesItem = NSMenuItem(title: "Services", action: nil, keyEquivalent: "")
        servicesItem.submenu = services
        bar.addItem(menu("Ocho", [
            servicesItem,
            .separator(),
            item("Check for Updates…", "check-for-updates"),
            item("What’s New…", "whats-new"),
            item("Welcome to Ocho…", "welcome"),
            item("Settings…", "settings", ","),
            .separator(),
            item("Quit Ocho", "quit", "q"),
        ]))
        bar.addItem(menu("File", [
            item("New Window", "new-window", "n", [.command, .option]),
            item("Launch Session…", "launch", "n"),
            item("Quick Launch…", "quick-launch", "n", [.command, .shift]),
            .separator(),
            item("Close Tab", "close-tab", "w"),
            item("Reopen Closed Tab", "reopen-closed-tab", "t", [.command, .shift]),
            item("Close Window", "close-window", "w", [.command, .shift]),
        ]))
        let edit: [NSMenuItem] = [
            NSMenuItem(title: "Undo", action: Selector(("undo:")), keyEquivalent: "z"),
            { let i = NSMenuItem(title: "Redo", action: Selector(("redo:")), keyEquivalent: "z"); i.keyEquivalentModifierMask = [.command, .shift]; return i }(),
            .separator(),
            NSMenuItem(title: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x"),
            NSMenuItem(title: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c"),
            NSMenuItem(title: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v"),
            .separator(),
            NSMenuItem(title: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a"),
        ]
        bar.addItem(menu("Edit", edit))
        bar.addItem(menu("View", [
            item("Command Palette", "palette", "p"),
            item("Open Session for PR…", "pull-requests", "p", [.command, .option]),
            item("Find Session by Topic…", "conversations", "f", [.command, .shift]),
            item("Refresh Ocho", "refresh", "r"),
            item("Clear Terminal Scrollback", "clear-scrollback", "k"),
            item("Change Theme…", "themes"),
            item("Pair Phone…", "pair-phone"),
            item("Pair iMessage…", "pair-imessage"),
            .separator(),
            item("Ocho Manager", "tab-0", "0"),
            item("Next Tab", "next-tab", "]", [.command, .shift]),
            item("Previous Tab", "prev-tab", "[", [.command, .shift]),
        ]))
        bar.addItem(menu("Help", [item("Keyboard Help", "help", "/")]))
        if let develop {
            develop.menu?.removeItem(develop)
            bar.addItem(develop)
        }
        return bar
    }
}
