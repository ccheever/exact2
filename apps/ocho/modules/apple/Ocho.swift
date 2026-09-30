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
            var answer: [String: Any] = ["events": events, "now": Date().timeIntervalSince1970]
            if let message = fleet.failure { answer["failure"] = message }
            reply.send(answer)
        case "desktop":
            reply.send(["text": desktop.text, "mtime": desktop.mtime, "home": FleetHome.path, "now": Date().timeIntervalSince1970])
        case "io":
            let jobs = request["jobs"] as? [[String: Any]] ?? []
            self.jobs.run(jobs, fleet: fleet, desktop: desktop, terminals: terminals) { replies in reply.send(["replies": replies, "now": Date().timeIntervalSince1970]) }
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
    var onEvents: (() -> Void)?

    init(binary: String?) { self.binary = binary }

    func start() {
        guard let binary else {
            failure = "The fleet CLI wasn't found. Install Ocho or set FLEET_BIN."
            DispatchQueue.main.async { self.onEvents?() }
            return
        }
        let p = Process()
        p.executableURL = URL(fileURLWithPath: binary)
        p.arguments = ["snapshot", "--watch"]
        var env = ProcessInfo.processInfo.environment
        env["FLEET_HOME"] = FleetHome.path
        p.environment = env
        let out = Pipe(), inp = Pipe(), err = Pipe()
        p.standardOutput = out; p.standardInput = inp; p.standardError = err
        stdin = inp.fileHandleForWriting
        process = p
        startedAt = Date()
        var buffer = Data()
        out.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard let self else { return }
            if data.isEmpty { handle.readabilityHandler = nil; return }
            buffer.append(data)
            while let nl = buffer.firstIndex(of: 0x0A) {
                let line = buffer.subdata(in: buffer.startIndex..<nl)
                buffer.removeSubrange(buffer.startIndex...nl)
                self.queue.async { self.consume(line) }
            }
        }
        err.fileHandleForReading.readabilityHandler = { handle in _ = handle.availableData }
        p.terminationHandler = { [weak self] _ in
            guard let self, !self.stopping else { return }
            // feed.rs:58-67: 2, 2, 4, 8 … capped at 60 s; 30 s healthy resets.
            if Date().timeIntervalSince(self.startedAt) > 30 { self.backoff = 2 }
            let wait = self.backoff
            self.backoff = min(60, self.backoff * 2)
            DispatchQueue.main.asyncAfter(deadline: .now() + wait) { [weak self] in self?.start() }
        }
        do { try p.run(); failure = nil } catch {
            failure = "Couldn't start fleet: \(error.localizedDescription)"
            DispatchQueue.main.async { self.onEvents?() }
        }
    }

    func stop() {
        stopping = true
        try? stdin?.close()
        process?.terminate()
    }

    /// Ask for a fresh round now (a `\n` on stdin wakes the watcher).
    func refresh() { try? stdin?.write(contentsOf: Data("\n".utf8)) }

    private func consume(_ line: Data) {
        guard !line.isEmpty, let event = try? JSONSerialization.jsonObject(with: line) as? [String: Any] else { return }
        pending.append(event)
        if !announced {
            announced = true
            DispatchQueue.main.async { [weak self] in self?.onEvents?() }
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
                let url = URL(fileURLWithPath: FleetHome.path).appendingPathComponent("titles.json")
                let body = (try? JSONSerialization.data(withJSONObject: ["prompt": stdin])) ?? Data()
                try? body.write(to: url)
                try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
                finish(["status": 0, "stderr": "", "stdout": ""])
            case "clear-terminal", "reconnect-terminal":
                DispatchQueue.main.async {
                    if argv.count > 1 { argv[0] == "clear-terminal" ? terminals.clear(tab: argv[1]) : terminals.close(tab: argv[1]) }
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
            case "write-terminal":
                DispatchQueue.main.async {
                    if argv.count > 1 { terminals.write(tab: argv[1], text: stdin) }
                    finish(["status": 0, "stderr": "", "stdout": ""])
                }
            default:
                queue.async { [binary] in
                    guard let binary else { return finish(["status": 127, "stderr": "The fleet CLI wasn't found. Install Ocho or set FLEET_BIN.", "stdout": ""]) }
                    finish(Self.fleet(binary: binary, argv: argv, stdin: stdin))
                }
            }
        }
        group.notify(queue: .main) { done(replies) }
    }

    /// One `fleet` command to completion, or killed at 60 s (backend.rs).
    static func fleet(binary: String, argv: [String], stdin: String) -> [String: Any] {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: binary)
        p.arguments = argv
        var env = ProcessInfo.processInfo.environment
        env["FLEET_HOME"] = FleetHome.path
        p.environment = env
        let out = Pipe(), err = Pipe(), inp = Pipe()
        p.standardOutput = out; p.standardError = err; p.standardInput = inp
        do { try p.run() } catch { return ["status": 127, "stderr": error.localizedDescription, "stdout": ""] }
        if !stdin.isEmpty { inp.fileHandleForWriting.write(Data(stdin.utf8)) }
        try? inp.fileHandleForWriting.close()
        let stdoutData = DispatchQueue.global().sync { out.fileHandleForReading.readDataToEndOfFile() }
        let deadline = DispatchTime.now() + 60
        let waiter = DispatchSemaphore(value: 0)
        p.terminationHandler = { _ in waiter.signal() }
        if p.isRunning && waiter.wait(timeout: deadline) == .timedOut {
            p.terminate()
            return ["status": 124, "stderr": "fleet \(argv.joined(separator: " ")) timed out after 60 s", "stdout": String(decoding: stdoutData, as: UTF8.self)]
        }
        let stderrData = err.fileHandleForReading.readDataToEndOfFile()
        return [
            "status": Int(p.terminationStatus),
            "stdout": String(decoding: stdoutData, as: UTF8.self),
            "stderr": String(decoding: stderrData, as: UTF8.self),
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
