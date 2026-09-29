// Ocho's native module (LLP 1024, LLP 1067.000): everything that reaches
// outside the process. One instance a session owns
//
//   - the fleet feed: `fleet snapshot --watch`, merged into one picture of
//     the machines, their sessions and the accounts, projected for the
//     Contract as the `fleet` answer; every change announces the `fleet`
//     topic, and the Rust source asks again;
//   - the open tabs, each a command in a terminal (`fleet attach`, `fleet
//     run --attach`, `fleet connect`), kept across the view that shows them
//     (Terminal.swift owns the libghostty surfaces, keyed by tab);
//   - the launcher's stable two-digit slots, seeded from the existing
//     desktop's `desktop.json` so the numbers a person has learned stay put;
//   - the one-shot commands (`fleet models`, `fleet interrupt`, …).
//
// Requests arrive as `native.later` bodies `{op, args}` from the Rust source.
import Foundation
import AppKit

final class OchoModule: ExactModule {
    override class var views: [String: ExactNativeFactory] {
        ["ghostty-terminal": ExactNativeFactory(for: OchoModule.self) { module, props, events in
            try TerminalInstance(module: module, props: props, events: events)
        }]
    }

    let fleet: FleetFeed
    let tabs: TabStore
    let launcher: LauncherSlots
    lazy var terminals = TerminalStore(module: self)

    required init(context: ExactModuleContext) {
        fleet = FleetFeed(binary: FleetBinary.locate())
        tabs = TabStore(file: context.data.appendingPathComponent("tabs.json"))
        launcher = LauncherSlots(file: context.data.appendingPathComponent("launcher.json"))
        super.init(context: context)
        fleet.onChange = { [weak self] in self?.announce() }
        tabs.onChange = { [weak self] in self?.announce() }
        fleet.start()
        // A `fleet` answer made before this module loaded settled empty; now
        // there is someone to ask.
        DispatchQueue.main.async { [weak self] in self?.announce() }
        colorsObserver = NotificationCenter.default.addObserver(forName: .ochoTerminalColors, object: nil, queue: .main) { [weak self] _ in self?.announce() }
    }
    private var colorsObserver: NSObjectProtocol?

    /// Say the picture changed: the `fleet` resource is asked again.
    func announce() { context.changed("fleet") }

    override func destroy() {
        fleet.stop()
        terminals.destroyAll()
    }

    override func later(_ request: [String: Any], reply: ExactReply) {
        let op = request["op"] as? String ?? ""
        let args = request["args"] as? [Any] ?? []
        func arg(_ i: Int) -> String { i < args.count ? (args[i] as? String ?? (args[i] as? NSNumber)?.stringValue ?? "") : "" }
        func num(_ i: Int) -> Double { i < args.count ? ((args[i] as? NSNumber)?.doubleValue ?? Double(arg(i)) ?? 0) : 0 }
        func flag(_ i: Int) -> Bool { i < args.count ? ((args[i] as? Bool) ?? (arg(i) == "true")) : false }
        switch op {
        case "fleet":
            // The answer is a walk over every session: built off the main
            // thread from a copy of the picture, so a scroll never waits on it.
            let picture = fleet.picture, tabs = self.tabs.all, exited = terminals.exited, usage = self.usage
            let choices = launcher.choices(accounts: picture.accounts, machines: picture.machines)
            let dirs = launcher.recentDirs(machines: picture.machines)
            refreshUsage(picture.accounts)
            let query = arg(0)
            let background = tabs.isEmpty ? "" : GhosttyRuntime.shared.background
            answerQueue.async { reply.send(Self.fleetAnswer(picture: picture, tabs: tabs, exited: exited, usage: usage, choices: choices, recentDirs: dirs, query: query, terminalBackground: background)) }
        case "openTab":
            let tab = tabs.attach(machineId: arg(0), sessionId: arg(1), readOnly: flag(2), fleet: fleet)
            NSLog("ocho: openTab %@ -> %@ (%d tabs)", arg(1), tab.id, tabs.all.count)
            announce()
            reply.send(receipt(ok: true, tab: tab.id, version: num(3)))
        case "openShell":
            let tab = tabs.shell(machineId: arg(0), fleet: fleet)
            announce()
            reply.send(receipt(ok: true, tab: tab.id, version: num(1)))
        case "toggleFolder":
            tabs.toggleFolder(arg(0))
            announce()
            reply.send(receipt(ok: true, tab: "", version: 0))
        case "closeTab":
            let id = arg(0)
            terminals.close(tab: id)
            tabs.remove(id)
            announce()
            reply.send(receipt(ok: true, tab: "", version: 0))
        case "launch":
            // args: account handle, machine id, model, cwd, version
            let handle = arg(0), machine = arg(1), model = arg(2), cwd = arg(3)
            let provider = String(handle.split(separator: ":").first ?? "")
            guard !machine.isEmpty, !model.isEmpty, !provider.isEmpty else {
                return reply.send(receipt(ok: false, tab: "", version: num(4), message: "Pick an account, a machine and a model."))
            }
            let tab = tabs.launch(machineId: machine, provider: provider, account: handle, model: model, cwd: cwd, fleet: fleet)
            launcher.remember(machine: machine, cwd: cwd, provider: provider, model: model)
            announce()
            reply.send(receipt(ok: true, tab: tab.id, version: num(4)))
        case "models":
            // args: machine id, provider, account handle, cwd
            models(machine: arg(0), provider: arg(1), account: arg(2), cwd: arg(3), reply: reply)
        case "sessionAction":
            let machine = arg(0), session = arg(1), action = arg(2)
            guard ["interrupt", "pause", "resume", "archive", "unarchive", "pin", "unpin", "stop", "terminate"].contains(action) else {
                return reply.send(receipt(ok: false, tab: "", version: 0, message: "Unknown action \(action)"))
            }
            fleet.run([action, machine, session, "--yes"]) { [weak self] result in
                DispatchQueue.main.async {
                    self?.fleet.refresh()
                    switch result {
                    case .success: reply.send(self?.receipt(ok: true, tab: "", version: 0, message: "\(action.capitalized) sent") ?? [:])
                    case .failure(let e): reply.send(self?.receipt(ok: false, tab: "", version: 0, message: e.localizedDescription) ?? [:])
                    }
                }
            }
        default:
            reply.fail("Ocho answers no \(op)")
        }
    }

    private func receipt(ok: Bool, tab: String, version: Double, message: String = "") -> [String: Any] {
        ["ok": ok, "message": message, "tabId": tab, "version": version]
    }

    // MARK: the `fleet` answer

    private let answerQueue = DispatchQueue(label: "ocho.answer", qos: .userInitiated)

    private static func stateName(_ s: String) -> String {
        switch s {
        case "running": return "Working"
        case "blocked": return "Needs you"
        case "limited": return "Rate limited"
        case "idle": return "Idle"
        case "paused": return "Paused"
        case "starting": return "Starting"
        case "closed": return "Closed"
        default: return s
        }
    }

    private static func fleetAnswer(picture: FleetPicture, tabs: [Tab], exited: Set<String>, usage: [String: String], choices: (accounts: [[String: Any]], machines: [[String: Any]]), recentDirs: [[String: Any]], query: String, terminalBackground: String) -> [String: Any] {
        let needle = query.trimmingCharacters(in: .whitespaces).lowercased()
        var sessions: [[String: Any]] = []
        var liveCount = 0
        var machines: [[String: Any]] = []
        for m in picture.machines {
            let live = m.sessions.filter { $0.shown }
            liveCount += live.filter { $0.live }.count
            machines.append([
                "id": m.id, "name": m.name, "online": m.online, "status": m.status, "index": 0,
                "sessions": live.filter { $0.live }.count, "active": live.filter { $0.state == "running" }.count,
                "detail": m.detail, "local": m.local, "home": m.home,
            ])
            let home = m.home
            func tilde(_ path: String) -> String {
                guard home.count > 1, path.hasPrefix(home) else { return path }
                let rest = path.dropFirst(home.count)
                return rest.isEmpty || rest.hasPrefix("/") ? "~" + rest : path
            }
            for s in live {
                if !needle.isEmpty {
                    let hay = [s.title, s.cwd, m.name, s.provider, s.accountEmail, s.model, s.summary, s.state].joined(separator: " ").lowercased()
                    if !hay.contains(needle) { continue }
                }
                sessions.append([
                    "id": s.id, "machineId": m.id, "machine": m.name, "title": s.title, "provider": s.provider,
                    "account": s.accountEmail, "state": s.state, "cwd": tilde(s.cwd), "model": s.model, "summary": s.summary,
                    "updated": s.updatedText, "pinned": s.pinned, "live": s.live, "startedAt": s.startedAt,
                ])
            }
        }
        // The desktop's order, which holds still as statuses tick: pinned
        // first, then newest start first.
        sessions.sort { a, b in
            let pa = a["pinned"] as! Bool, pb = b["pinned"] as! Bool
            if pa != pb { return pa }
            return (a["startedAt"] as! Double) > (b["startedAt"] as! Double)
        }
        for i in sessions.indices { sessions[i]["index"] = i }
        for i in machines.indices { machines[i]["index"] = i }
        let accounts: [[String: Any]] = picture.accounts.map {
            ["handle": $0.handle, "email": $0.email, "provider": $0.provider, "status": $0.status, "connected": $0.status == "connected", "usage": usage[$0.handle] ?? ""]
        }
        var slot = 0
        let tabRows: [[String: Any]] = tabs.map { t in
            if t.kind == .folder {
                return ["id": t.id, "slot": 0, "title": t.title, "subtitle": "", "provider": "", "machineId": "", "sessionId": "", "state": "", "status": "", "depth": t.depth, "folder": true, "collapsed": t.collapsed]
            }
            slot += 1
            let i = slot - 1
            var state = exited.contains(t.id) ? "exited" : (t.kind == .shell ? "idle" : "starting")
            if let s = picture.session(machine: t.machineId, id: t.sessionId) { state = s.state }
            if exited.contains(t.id) { state = "exited" }
            var title = t.title, subtitle = t.subtitle
            if let s = picture.session(machine: t.machineId, id: t.sessionId), !s.title.isEmpty { title = s.title }
            let provider = t.provider.isEmpty ? (picture.session(machine: t.machineId, id: t.sessionId)?.provider ?? "") : t.provider
            if let m = picture.machine(t.machineId) { subtitle = t.kind == .shell ? "Shell · \(m.name)" : "\(m.name) · \(Provider.name(provider))" }
            let session = picture.session(machine: t.machineId, id: t.sessionId)
            var status = ""
            if let session { status = session.summary.isEmpty ? Self.stateName(session.state) : session.summary }
            else if t.kind == .shell { status = "Shell" }
            if exited.contains(t.id) { status = "Ended" }
            return ["id": t.id, "slot": i + 1, "title": title, "subtitle": subtitle, "provider": provider, "machineId": t.machineId, "sessionId": t.sessionId, "state": state, "status": status, "depth": t.depth, "folder": false, "collapsed": false]
        }
        return [
            "ready": picture.ready, "message": picture.message,
            "machines": machines, "sessions": sessions, "accounts": accounts, "tabs": tabRows,
            "accountChoices": choices.accounts, "machineChoices": choices.machines, "recentDirs": recentDirs,
            "liveCount": liveCount, "revision": picture.revision, "terminalBackground": terminalBackground,
        ]
    }

    // MARK: usage

    private var usage: [String: String] = [:]
    private var usageAskedAt: [String: Double] = [:]

    /// `fleet accounts usage <handle>` for each connected account, at most
    /// every ten minutes, announced when it lands.
    private func refreshUsage(_ accounts: [AccountPicture]) {
        let now = Date().timeIntervalSince1970
        for a in accounts where a.status == "connected" && now - (usageAskedAt[a.handle] ?? 0) > 600 {
            usageAskedAt[a.handle] = now
            fleet.run(["accounts", "usage", a.handle]) { [weak self] result in
                guard case .success(let text) = result,
                      let json = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any] else { return }
                func pct(_ key: String) -> String? {
                    guard let w = json[key] as? [String: Any], let p = w["used_percent"] as? NSNumber else { return nil }
                    return "\(p.intValue)%"
                }
                var parts: [String] = []
                if let five = pct("five_hour") { parts.append("5h \(five)") }
                if let week = pct("weekly") { parts.append("week \(week)") }
                let line = parts.joined(separator: " · ")
                DispatchQueue.main.async {
                    guard let self, self.usage[a.handle] != line else { return }
                    self.usage[a.handle] = line
                    self.announce()
                }
            }
        }
    }

    // MARK: models

    private func models(machine: String, provider: String, account: String, cwd: String, reply: ExactReply) {
        let key = "\(machine)|\(provider)|\(account)|\(cwd)"
        func answer(_ options: [ModelOption], message: String, ready: Bool) -> [String: Any] {
            let rows: [[String: Any]] = options.enumerated().map { i, o in
                let code = String(format: "%02d", i + 1)
                return ["id": o.id, "code": code, "group": String(code.prefix(1)), "label": o.name.isEmpty ? o.id : o.name, "detail": o.description, "provider": provider]
            }
            return ["key": key, "ready": ready, "message": message, "models": rows]
        }
        if let cached = launcher.catalog(machine: machine, provider: provider, account: account) {
            reply.send(answer(cached, message: "", ready: true))
            // A fresh catalog for next time, quietly.
            fleet.models(machine: machine, provider: provider, account: account, cwd: cwd) { [weak self] result in
                if case .success(let options) = result { DispatchQueue.main.async { self?.launcher.cache(machine: machine, provider: provider, account: account, options: options) } }
            }
            return
        }
        fleet.models(machine: machine, provider: provider, account: account, cwd: cwd) { [weak self] result in
            DispatchQueue.main.async {
                switch result {
                case .success(let options):
                    self?.launcher.cache(machine: machine, provider: provider, account: account, options: options)
                    reply.send(answer(options, message: "", ready: true))
                case .failure(let error):
                    reply.send(answer([], message: "Couldn't load models: \(error.localizedDescription)", ready: true))
                }
            }
        }
    }
}
let exactModule: ExactModule.Type = OchoModule.self

enum Provider {
    static func name(_ p: String) -> String {
        switch p { case "claude": return "Claude"; case "codex": return "Codex"; case "opencode": return "OpenCode"; case "shell": return "Shell"; default: return p }
    }
}

// MARK: - The fleet binary

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

struct ModelOption { let id: String; let name: String; let description: String; let isDefault: Bool }

struct FleetError: LocalizedError { let text: String; var errorDescription: String? { text } }

// MARK: - The feed

/// One machine as the feed shows it, with the projections the Contract needs.
struct MachinePicture {
    var id: String, name: String, local: Bool, error: String, addresses: [String]
    var last: [String: Any]?
    var sessions: [SessionPicture]
    var online: Bool { error.isEmpty && last != nil }
    var status: String { !error.isEmpty ? "unreachable" : (last == nil ? "pending" : "online") }
    var home: String { last?["home"] as? String ?? "~" }
    var detail: String {
        guard let last else { return error.isEmpty ? (addresses.first ?? "Waiting for the first snapshot…") : error }
        var parts: [String] = []
        if let platform = last["platform"] as? String, !platform.isEmpty { parts.append(platform) }
        if let cores = last["cores"] as? NSNumber, cores.intValue > 0 { parts.append("\(cores.intValue) cores") }
        if let used = last["memory_used"] as? NSNumber, let total = last["memory_total"] as? NSNumber, total.doubleValue > 0 {
            parts.append(String(format: "%.0f/%.0f GB", used.doubleValue / 1e9, total.doubleValue / 1e9))
        }
        if let load = last["load"] as? NSNumber { parts.append(String(format: "load %.1f", load.doubleValue)) }
        else if let load = last["load"] as? [NSNumber], let one = load.first { parts.append(String(format: "load %.1f", one.doubleValue)) }
        if !error.isEmpty { parts.append(error) }
        return parts.joined(separator: " · ")
    }
}

struct SessionPicture {
    var raw: [String: Any]
    var accountEmail: String
    var id: String { raw["id"] as? String ?? "" }
    var title: String {
        let t = raw["title"] as? String ?? ""
        if !t.isEmpty { return t }
        let cwd = self.cwd
        return cwd.isEmpty ? Provider.name(provider) : "\(Provider.name(provider)) · \((cwd as NSString).lastPathComponent)"
    }
    var provider: String { raw["provider"] as? String ?? "" }
    var pid: Int { (raw["pid"] as? NSNumber)?.intValue ?? 0 }
    /// The state as the desktop shows it (`session_state`): a paused or
    /// starting session is on its way, no process means closed, and the
    /// older working/awaiting words map onto running/blocked.
    var state: String {
        let s = raw["state"] as? String ?? ""
        if provider == "shell" { return pid == 0 && !["running", "idle"].contains(s) ? "closed" : "idle" }
        if s == "paused" && !historical { return "paused" }
        if s == "starting" && !historical { return "starting" }
        if historical || pid == 0 || s == "closed" || s == "exited" { return "closed" }
        switch s {
        case "working", "running": return "running"
        case "awaiting approval", "awaiting input", "blocked": return "blocked"
        case "limited": return "limited"
        case "idle": return "idle"
        default: return "unknown"
        }
    }
    var cwd: String { raw["cwd"] as? String ?? "" }
    var model: String { raw["model"] as? String ?? "" }
    var pinned: Bool { raw["pinned"] as? Bool ?? false }
    var historical: Bool { raw["historical"] as? Bool ?? false }
    var hidden: Bool { raw["hidden"] as? Bool ?? false }
    var archived: Bool { raw["archived"] as? Bool ?? false }
    var managed: Bool { raw["managed"] as? Bool ?? false }
    var tracked: Bool { raw["tracked"] as? Bool ?? false }
    /// The desktop's default view: Ocho-launched or tracked, unarchived sessions.
    var shown: Bool { !historical && !hidden && !archived && (managed || tracked) }
    var live: Bool { state != "closed" }
    var startedAt: Double { Dates.parse(raw["started"] as? String) ?? 0 }
    var rank: Int {
        switch state {
        case "running": return 0
        case "blocked", "limited": return 1
        case "starting": return 2
        case "idle": return 3
        case "paused": return 4
        case "closed", "exited": return 7
        default: return 5
        }
    }
    var summary: String {
        let text = (raw["status_text"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? (raw["last_message"] as? String ?? "")
        let line = text.split(whereSeparator: \.isNewline).map { $0.trimmingCharacters(in: .whitespaces) }.first { !$0.isEmpty } ?? ""
        return line.count > 160 ? String(line.prefix(157)) + "…" : line
    }
    var updatedAt: Double { Dates.parse(raw["updated"] as? String) ?? Dates.parse(raw["started"] as? String) ?? 0 }
    var updatedText: String { Dates.relative(updatedAt) }
}

struct AccountPicture { let handle: String, email: String, provider: String, status: String, name: String }

struct FleetPicture {
    var machines: [MachinePicture] = []
    var accounts: [AccountPicture] = []
    var ready = false
    var message = ""
    var revision = 0
    func machine(_ id: String) -> MachinePicture? { machines.first { $0.id == id } }
    func session(machine: String, id: String) -> SessionPicture? {
        guard !id.isEmpty else { return nil }
        return self.machine(machine)?.sessions.first { $0.id == id }
    }
}

/// `fleet snapshot --watch`: one process, NDJSON events, a newline on its stdin
/// refreshes now. The merge follows the existing desktop's rules: `state`
/// replaces the saved fleet, `machine` one machine's observation,
/// `session-indicators` patches sessions.
final class FleetFeed {
    let binary: String?
    private var process: Process?
    private var stdin: FileHandle?
    private let queue = DispatchQueue(label: "ocho.fleet", qos: .utility)
    private var rawMachines: [[String: Any]] = []
    private var rawAccounts: [[String: Any]] = []
    private(set) var picture = FleetPicture()
    private var lastDigest = ""
    var onChange: (() -> Void)?
    private var stopping = false

    init(binary: String?) { self.binary = binary }

    func start() {
        guard let binary else {
            picture.message = "The fleet CLI wasn't found. Install Ocho or set FLEET_BIN."
            picture.ready = false
            onChange?()
            return
        }
        let p = Process()
        p.executableURL = URL(fileURLWithPath: binary)
        p.arguments = ["snapshot", "--watch", "--interval", "6s"]
        let out = Pipe(), inp = Pipe(), err = Pipe()
        p.standardOutput = out; p.standardInput = inp; p.standardError = err
        stdin = inp.fileHandleForWriting
        process = p
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
            DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in self?.start() }
        }
        do { try p.run() } catch {
            picture.message = "Couldn't start fleet: \(error.localizedDescription)"
            onChange?()
        }
    }

    func stop() {
        stopping = true
        try? stdin?.close()
        process?.terminate()
    }

    /// Ask for a fresh snapshot now.
    func refresh() { try? stdin?.write(contentsOf: Data("\n".utf8)) }

    private func consume(_ line: Data) {
        guard !line.isEmpty, let event = try? JSONSerialization.jsonObject(with: line) as? [String: Any] else { return }
        let kind = event["type"] as? String ?? ""
        switch kind {
        case "state":
            // The saved fleet. Only the stream's first state carries snapshots;
            // a round-closing one relies on the machine events before it, so
            // a machine already observed keeps what it has (the desktop's
            // `merge_state`).
            guard let state = event["state"] as? [String: Any] else { return }
            var incoming = state["machines"] as? [[String: Any]] ?? rawMachines
            for i in incoming.indices {
                guard let id = incoming[i]["id"] as? String, let old = rawMachines.first(where: { ($0["id"] as? String) == id }), old["last"] != nil else { continue }
                incoming[i]["last"] = old["last"]
                incoming[i]["error"] = old["error"]
            }
            rawMachines = incoming
            rawAccounts = state["accounts"] as? [[String: Any]] ?? rawAccounts
            let accountError = event["account_error"] as? String ?? ""
            project(ready: true, message: accountError)
        case "machine":
            // A fresh observation of one machine (the desktop's `merge_machine`):
            // an `unchanged` event carries the vitals without the sessions, so
            // the sessions shown move into it; a partial scan (no live
            // inventory) keeps the sessions it did not visit.
            guard var machine = event["machine"] as? [String: Any], let id = machine["id"] as? String else { return }
            let previous = rawMachines.first(where: { ($0["id"] as? String) == id })
            let previousSessions = (previous?["last"] as? [String: Any])?["sessions"] as? [[String: Any]] ?? []
            if var last = machine["last"] as? [String: Any] {
                var sessions = last["sessions"] as? [[String: Any]] ?? []
                if event["unchanged"] as? Bool == true {
                    sessions = previousSessions
                } else if last["live_inventory"] as? Bool != true {
                    for old in previousSessions where !sessions.contains(where: { ($0["id"] as? String) == (old["id"] as? String) }) { sessions.append(old) }
                }
                last["sessions"] = sessions
                machine["last"] = last
            } else if let previousLast = previous?["last"] {
                machine["last"] = previousLast
            }
            if let i = rawMachines.firstIndex(where: { ($0["id"] as? String) == id }) { rawMachines[i] = machine } else { rawMachines.append(machine) }
            project(ready: true, message: nil)
        case "session-indicators":
            guard let ind = event["indicators"] as? [String: Any], let id = ind["machine_id"] as? String,
                  let i = rawMachines.firstIndex(where: { ($0["id"] as? String) == id }) else { return }
            var machine = rawMachines[i]
            var last = machine["last"] as? [String: Any] ?? [:]
            var sessions = last["sessions"] as? [[String: Any]] ?? []
            let statusKeys = ["status_observed_at", "state", "pid", "status_text", "status_style", "status_source", "last_message"]
            for patch in ind["sessions"] as? [[String: Any]] ?? [] {
                guard let sid = patch["id"] as? String else { continue }
                if let j = sessions.firstIndex(where: { ($0["id"] as? String) == sid }) {
                    let was = (sessions[j]["status_observed_at"] as? NSNumber)?.doubleValue ?? 0
                    let now = (patch["status_observed_at"] as? NSNumber)?.doubleValue ?? 0
                    guard now > was || was == 0 else { continue }
                    for k in statusKeys { if let v = patch[k] { sessions[j][k] = v } }
                } else { sessions.append(patch) }
            }
            last["sessions"] = sessions
            machine["last"] = last
            rawMachines[i] = machine
            project(ready: true, message: nil)
        case "error":
            project(ready: picture.ready, message: event["error"] as? String ?? "")
        default:
            break
        }
    }

    private func project(ready: Bool, message: String?) {
        let accounts: [AccountPicture] = rawAccounts.map {
            let provider = $0["provider"] as? String ?? "", email = $0["email"] as? String ?? ""
            return AccountPicture(handle: "\(provider):\(email)", email: email, provider: provider, status: $0["status"] as? String ?? "", name: $0["name"] as? String ?? "")
        }
        let byName = Dictionary(accounts.map { ($0.name, $0.email) }, uniquingKeysWith: { a, _ in a })
        let byHandle = Dictionary(accounts.map { ($0.handle, $0.email) }, uniquingKeysWith: { a, _ in a })
        let machines: [MachinePicture] = rawMachines.map { m in
            let last = m["last"] as? [String: Any]
            let sessions = (last?["sessions"] as? [[String: Any]] ?? []).map { s -> SessionPicture in
                let account = s["account"] as? String ?? ""
                let email = byName[account] ?? byHandle[account] ?? (account.contains("@") ? String(account.split(separator: ":").last ?? "") : (account == "default" || account.isEmpty ? "" : account))
                return SessionPicture(raw: s, accountEmail: email)
            }
            return MachinePicture(id: m["id"] as? String ?? "", name: m["name"] as? String ?? "", local: m["local"] as? Bool ?? false,
                                  error: m["error"] as? String ?? "", addresses: m["addresses"] as? [String] ?? [], last: last, sessions: sessions)
        }.sorted { a, b in a.local != b.local ? a.local : a.name.localizedCaseInsensitiveCompare(b.name) == .orderedAscending }
        // What the Contract can see, as one string: an event that changed
        // none of it (a per-second indicator tick, a round-closing state)
        // announces nothing, so the app never re-diffs the list for nothing.
        var digest = ready ? "r" : "-"
        digest += message ?? ""
        for m in machines {
            digest += "|\(m.id)\(m.status)\(m.detail)"
            for s in m.sessions where s.shown {
                digest += ";\(s.id)\(s.state)\(s.title)\(s.summary)\(s.updatedText)\(s.pinned)\(s.model)\(s.cwd)"
            }
        }
        for a in accounts { digest += "|\(a.handle)\(a.status)" }
        DispatchQueue.main.async {
            self.picture.machines = machines
            self.picture.accounts = accounts.sorted { ($0.provider, $0.email) < ($1.provider, $1.email) }
            self.picture.ready = ready
            if let message { self.picture.message = message }
            guard digest != self.lastDigest else { return }
            self.lastDigest = digest
            self.picture.revision += 1
            self.onChange?()
        }
    }

    /// One `fleet` command to completion, off the main thread; stdout as text.
    func run(_ arguments: [String], completion: @escaping (Result<String, Error>) -> Void) {
        guard let binary else { return completion(.failure(FleetError(text: "fleet CLI not found"))) }
        queue.async {
            let p = Process()
            p.executableURL = URL(fileURLWithPath: binary)
            p.arguments = arguments
            let out = Pipe(), err = Pipe()
            p.standardOutput = out; p.standardError = err
            do {
                try p.run()
                let data = out.fileHandleForReading.readDataToEndOfFile()
                let errData = err.fileHandleForReading.readDataToEndOfFile()
                p.waitUntilExit()
                if p.terminationStatus == 0 { completion(.success(String(decoding: data, as: UTF8.self))) }
                else {
                    let text = String(decoding: errData.isEmpty ? data : errData, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
                    completion(.failure(FleetError(text: text.isEmpty ? "fleet exited \(p.terminationStatus)" : String(text.split(whereSeparator: \.isNewline).last ?? ""))))
                }
            } catch { completion(.failure(error)) }
        }
    }

    func models(machine: String, provider: String, account: String, cwd: String, completion: @escaping (Result<[ModelOption], Error>) -> Void) {
        var args = ["models", machine, "--provider", provider]
        if !account.isEmpty { args += ["--account", account] }
        if !cwd.isEmpty { args += ["--cwd", cwd] }
        run(args) { result in
            completion(result.flatMap { text in
                guard let json = try? JSONSerialization.jsonObject(with: Data(text.utf8)) else { return .failure(FleetError(text: "models: not JSON")) }
                return .success(ModelCatalog.options(from: json))
            })
        }
    }
}

enum ModelCatalog {
    /// The catalog's options, from `fleet models` output or the desktop's cache.
    static func options(from json: Any) -> [ModelOption] {
        var list: [[String: Any]] = []
        if let array = json as? [[String: Any]] { list = array }
        else if let object = json as? [String: Any] {
            list = (object["options"] ?? object["models"]) as? [[String: Any]] ?? []
        }
        return list.compactMap { o in
            guard let id = (o["id"] as? String) ?? (o["model"] as? String) ?? (o["name"] as? String), !id.isEmpty else { return nil }
            return ModelOption(id: id, name: o["name"] as? String ?? id, description: (o["description"] as? String) ?? "", isDefault: o["default"] as? Bool ?? false)
        }
    }
}

enum Dates {
    private static let iso: ISO8601DateFormatter = { let f = ISO8601DateFormatter(); f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]; return f }()
    private static let isoPlain: ISO8601DateFormatter = { let f = ISO8601DateFormatter(); f.formatOptions = [.withInternetDateTime]; return f }()
    static func parse(_ text: String?) -> Double? {
        guard let text, !text.isEmpty, !text.hasPrefix("0001") else { return nil }
        return (iso.date(from: text) ?? isoPlain.date(from: text))?.timeIntervalSince1970
    }
    static func relative(_ at: Double) -> String {
        guard at > 0 else { return "" }
        let delta = Date().timeIntervalSince1970 - at
        if delta < 45 { return "now" }
        if delta < 3600 { return "\(Int(delta / 60))m" }
        if delta < 86400 { return "\(Int(delta / 3600))h" }
        if delta < 86400 * 7 { return "\(Int(delta / 86400))d" }
        let f = DateFormatter(); f.dateFormat = "MMM d"
        return f.string(from: Date(timeIntervalSince1970: at))
    }
}

// MARK: - Tabs

enum TabKind: String, Codable { case attach, run, shell, folder }

struct Tab: Codable {
    var id: String
    var kind: TabKind
    var title: String
    var subtitle: String
    var provider: String
    var machineId: String
    var sessionId: String
    var command: [String]
    var env: [String: String]
    var depth = 0
    var collapsed = false
    /// From the existing Ocho's saved windows, not opened here.
    var mirrored = false

    init(id: String, kind: TabKind, title: String, subtitle: String, provider: String, machineId: String, sessionId: String, command: [String], env: [String: String], depth: Int = 0, collapsed: Bool = false, mirrored: Bool = false) {
        self.id = id; self.kind = kind; self.title = title; self.subtitle = subtitle; self.provider = provider
        self.machineId = machineId; self.sessionId = sessionId; self.command = command; self.env = env
        self.depth = depth; self.collapsed = collapsed; self.mirrored = mirrored
    }

    private enum Keys: String, CodingKey { case id, kind, title, subtitle, provider, machineId, sessionId, command, env, depth, collapsed, mirrored }
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        id = try c.decode(String.self, forKey: .id)
        kind = try c.decode(TabKind.self, forKey: .kind)
        title = try c.decode(String.self, forKey: .title)
        subtitle = try c.decodeIfPresent(String.self, forKey: .subtitle) ?? ""
        provider = try c.decodeIfPresent(String.self, forKey: .provider) ?? ""
        machineId = try c.decodeIfPresent(String.self, forKey: .machineId) ?? ""
        sessionId = try c.decodeIfPresent(String.self, forKey: .sessionId) ?? ""
        command = try c.decodeIfPresent([String].self, forKey: .command) ?? []
        env = try c.decodeIfPresent([String: String].self, forKey: .env) ?? [:]
        depth = try c.decodeIfPresent(Int.self, forKey: .depth) ?? 0
        collapsed = try c.decodeIfPresent(Bool.self, forKey: .collapsed) ?? false
        mirrored = try c.decodeIfPresent(Bool.self, forKey: .mirrored) ?? false
    }

    /// One session, wherever it is listed.
    var sessionKey: String { "\(machineId):\(sessionId)" }
}

/// The open tabs: a live mirror of the existing Ocho's rail (`desktop.json`
/// → `windows[].tabs`, folders and all, re-read as that file changes) plus the
/// tabs opened here. Closing a mirrored tab hides it here until Ocho drops
/// it; a folder's collapse can be overridden here. Tabs opened here are
/// saved so a relaunch reattaches them (a `run` tab as an attach).
final class TabStore {
    private struct Saved: Codable {
        var local: [Tab] = []
        var hidden: [String] = []
        var collapsed: [String: Bool] = [:]
    }
    private let file: URL
    private var saved = Saved()
    private var mirrored: [Tab] = []
    private let desktop: URL
    private var desktopStamp: Date?
    private var timer: Timer?
    var onChange: (() -> Void)?

    init(file: URL) {
        self.file = file
        let home = FileManager.default.homeDirectoryForCurrentUser
        let fleetHome = ProcessInfo.processInfo.environment["FLEET_HOME"].map { URL(fileURLWithPath: $0) } ?? home.appendingPathComponent(".local/share/fleet")
        desktop = fleetHome.appendingPathComponent("desktop.json")
        if let data = try? Data(contentsOf: file), let s = try? JSONDecoder().decode(Saved.self, from: data) { saved = s }
        else if let data = try? Data(contentsOf: file), let old = try? JSONDecoder().decode([Tab].self, from: data) { saved.local = old.filter { !$0.mirrored } }
        reloadDesktop()
        timer = Timer.scheduledTimer(withTimeInterval: 2, repeats: true) { [weak self] _ in self?.pollDesktop() }
    }

    deinit { timer?.invalidate() }

    /// Every tab in rail order: Ocho's, then the ones opened only here.
    var all: [Tab] {
        var out: [Tab] = []
        var hiddenDepth: Int?
        for t in mirrored {
            if let d = hiddenDepth { if t.depth > d { continue } else { hiddenDepth = nil } }
            if t.kind == .folder {
                var f = t
                f.collapsed = saved.collapsed[t.id] ?? t.collapsed
                out.append(f)
                if f.collapsed { hiddenDepth = t.depth }
                continue
            }
            if saved.hidden.contains(t.sessionKey) { continue }
            out.append(t)
        }
        let listed = Set(mirrored.map(\.sessionKey))
        out += saved.local.filter { $0.kind == .shell || !listed.contains($0.sessionKey) }
        return out
    }

    private func pollDesktop() {
        let stamp = (try? FileManager.default.attributesOfItem(atPath: desktop.path))?[.modificationDate] as? Date
        guard stamp != desktopStamp else { return }
        reloadDesktop()
        onChange?()
    }

    private func reloadDesktop() {
        desktopStamp = (try? FileManager.default.attributesOfItem(atPath: desktop.path))?[.modificationDate] as? Date
        guard let data = try? Data(contentsOf: desktop),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let windows = json["windows"] as? [[String: Any]] else { mirrored = []; return }
        var tabs: [Tab] = []
        var seen = Set<String>()
        for window in windows {
            for t in window["tabs"] as? [[String: Any]] ?? [] {
                let depth = (t["depth"] as? NSNumber)?.intValue ?? 0
                let key = (t["key"] as? String ?? UUID().uuidString).replacingOccurrences(of: ":", with: "-")
                if t["folder"] as? Bool == true {
                    tabs.append(Tab(id: key, kind: .folder, title: t["title"] as? String ?? "Folder", subtitle: "", provider: "", machineId: "", sessionId: "", command: [], env: [:],
                                    depth: depth, collapsed: t["collapsed"] as? Bool ?? false, mirrored: true))
                    continue
                }
                guard let reconnect = t["reconnect"] as? [String], reconnect.count >= 3, reconnect[0] == "attach" else { continue }
                let machine = reconnect[1], session = reconnect[2]
                guard seen.insert("\(machine):\(session)").inserted else { continue }
                let title = (t["title"] as? String ?? "Session").components(separatedBy: " · ").first ?? "Session"
                tabs.append(Tab(id: key, kind: .attach, title: title, subtitle: "", provider: "", machineId: machine, sessionId: session,
                                command: ["attach", machine, session] + reconnect.dropFirst(3), env: [:], depth: depth, mirrored: true))
            }
        }
        mirrored = tabs
    }

    private func save() {
        var s = saved
        s.local = saved.local.map { t -> Tab in
            guard t.kind == .run, !t.sessionId.isEmpty else { return t }
            var a = t; a.kind = .attach; a.command = ["attach", t.machineId, t.sessionId]; return a
        }
        try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        if let data = try? JSONEncoder().encode(s) { try? data.write(to: file, options: .atomic) }
    }

    func tab(_ id: String) -> Tab? { all.first { $0.id == id } ?? mirrored.first { $0.id == id } }

    func attach(machineId: String, sessionId: String, readOnly: Bool, fleet: FleetFeed) -> Tab {
        let key = "\(machineId):\(sessionId)"
        if let m = mirrored.first(where: { $0.kind == .attach && $0.sessionKey == key }) {
            if saved.hidden.contains(key) { saved.hidden.removeAll { $0 == key }; save() }
            return m
        }
        if let existing = saved.local.first(where: { $0.kind != .shell && $0.sessionKey == key }) { return existing }
        let session = fleet.picture.session(machine: machineId, id: sessionId)
        let machine = fleet.picture.machine(machineId)
        var command = ["attach", machineId, sessionId]
        if readOnly { command.append("--read-only") }
        let tab = Tab(id: UUID().uuidString, kind: .attach, title: session?.title ?? "Session", subtitle: "\(machine?.name ?? machineId) · \(Provider.name(session?.provider ?? ""))",
                      provider: session?.provider ?? "", machineId: machineId, sessionId: sessionId, command: command, env: [:])
        saved.local.append(tab); save()
        return tab
    }

    func shell(machineId: String, fleet: FleetFeed) -> Tab {
        let machine = fleet.picture.machine(machineId)
        let tab = Tab(id: UUID().uuidString, kind: .shell, title: "Shell", subtitle: "Shell · \(machine?.name ?? machineId)", provider: "shell",
                      machineId: machineId, sessionId: "", command: ["connect", machineId], env: [:])
        saved.local.append(tab); save()
        return tab
    }

    func launch(machineId: String, provider: String, account: String, model: String, cwd: String, fleet: FleetFeed) -> Tab {
        let machine = fleet.picture.machine(machineId)
        // The request id is the session id fleet gives the launch (the
        // desktop's tabs are keyed the same way), so the tab finds its
        // session in the feed and reattaches after a relaunch.
        let requestId = UUID().uuidString.lowercased().replacingOccurrences(of: "-", with: "")
        var command = ["run", machineId, "--request-id", requestId, "--provider", provider, "--account", account, "--model", model, "--attach"]
        if !cwd.isEmpty { command += ["--cwd", cwd] }
        let folder = (cwd as NSString).lastPathComponent
        let home = machine?.home ?? "~"
        let place = folder.isEmpty || folder == "~" || cwd == home ? model : folder
        let tab = Tab(id: UUID().uuidString, kind: .run, title: "\(Provider.name(provider)) · \(place)",
                      subtitle: "\(machine?.name ?? machineId) · \(model)", provider: provider, machineId: machineId, sessionId: requestId, command: command, env: [:])
        saved.local.append(tab); save()
        return tab
    }

    func remove(_ id: String) {
        if let m = mirrored.first(where: { $0.id == id }) {
            if m.kind != .folder, !saved.hidden.contains(m.sessionKey) { saved.hidden.append(m.sessionKey) }
        } else {
            saved.local.removeAll { $0.id == id }
        }
        save()
    }

    func toggleFolder(_ id: String) {
        guard let f = mirrored.first(where: { $0.id == id && $0.kind == .folder }) else { return }
        saved.collapsed[id] = !(saved.collapsed[id] ?? f.collapsed)
        save()
    }
}

// MARK: - Launcher slots

/// Two-digit codes that never move: a list per step, append-only, seeded
/// from the existing desktop's `desktop.json` so a person's muscle memory
/// carries over. Model catalogs are cached the same way the desktop does.
final class LauncherSlots {
    private struct Saved: Codable {
        var accounts: [String] = []   // "provider:email"
        var machines: [String] = []   // machine id
        var catalogs: [String: [[String: String]]] = [:]  // "machine|provider|account" → options
        var recent: [[String: String]] = []  // {machine, cwd}
    }
    private let file: URL
    private var saved = Saved()
    private var desktop: [String: Any] = [:]

    init(file: URL) {
        self.file = file
        let home = FileManager.default.homeDirectoryForCurrentUser
        let fleetHome = ProcessInfo.processInfo.environment["FLEET_HOME"].map { URL(fileURLWithPath: $0) } ?? home.appendingPathComponent(".local/share/fleet")
        if let data = try? Data(contentsOf: fleetHome.appendingPathComponent("desktop.json")), let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] { desktop = json }
        if let data = try? Data(contentsOf: file), let s = try? JSONDecoder().decode(Saved.self, from: data) { saved = s }
        else {
            // Seed from the desktop's slots: accounts are "provider\nprovider:email".
            let shortcuts = desktop["launch_shortcuts"] as? [String: Any] ?? [:]
            saved.accounts = (shortcuts["accounts"] as? [String] ?? []).map { String($0.split(separator: "\n").last ?? "") }
            saved.machines = shortcuts["machines"] as? [String] ?? []
        }
    }

    private func save() {
        try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        if let data = try? JSONEncoder().encode(saved) { try? data.write(to: file, options: .atomic) }
    }

    private static func code(_ index: Int) -> String { String(format: "%02d", index + 1) }

    func choices(accounts: [AccountPicture], machines: [MachinePicture]) -> (accounts: [[String: Any]], machines: [[String: Any]]) {
        var changed = false
        for a in accounts where !saved.accounts.contains(a.handle) { saved.accounts.append(a.handle); changed = true }
        for m in machines where !saved.machines.contains(m.id) { saved.machines.append(m.id); changed = true }
        if changed { save() }
        let accountRows: [[String: Any]] = saved.accounts.enumerated().compactMap { i, handle in
            guard let a = accounts.first(where: { $0.handle == handle }) else { return nil }
            let code = Self.code(i)
            return ["id": a.handle, "code": code, "group": String(code.prefix(1)), "label": a.email, "detail": Provider.name(a.provider), "provider": a.provider]
        }
        let machineRows: [[String: Any]] = saved.machines.enumerated().compactMap { i, id in
            guard let m = machines.first(where: { $0.id == id }) else { return nil }
            let code = Self.code(i)
            return ["id": m.id, "code": code, "group": String(code.prefix(1)), "label": m.name + (m.local ? " (this Mac)" : ""), "detail": m.home, "provider": ""]
        }
        return (accountRows, machineRows)
    }

    func recentDirs(machines: [MachinePicture]) -> [[String: Any]] {
        var seen = Set<String>()
        var rows: [[String: Any]] = []
        let desktopRecent = (desktop["recent_projects"] as? [[String: Any]] ?? []).compactMap { r -> [String: String]? in
            guard let m = r["machine"] as? String, let c = r["cwd"] as? String else { return nil }
            return ["machine": m, "cwd": c]
        }
        for r in saved.recent + desktopRecent {
            guard let m = r["machine"], let c = r["cwd"], !c.isEmpty else { continue }
            let key = "\(m)|\(c)"
            if seen.insert(key).inserted, rows.filter({ ($0["detail"] as! String) == m }).count < 6 {
                rows.append(["id": c, "code": "", "group": "", "label": c, "detail": m, "provider": ""])
            }
        }
        return rows
    }

    func remember(machine: String, cwd: String, provider: String, model: String) {
        saved.recent.removeAll { $0["machine"] == machine && $0["cwd"] == cwd }
        saved.recent.insert(["machine": machine, "cwd": cwd], at: 0)
        if saved.recent.count > 24 { saved.recent.removeLast(saved.recent.count - 24) }
        save()
    }

    func catalog(machine: String, provider: String, account: String) -> [ModelOption]? {
        if let rows = saved.catalogs["\(machine)|\(provider)|\(account)"] {
            return rows.map { ModelOption(id: $0["id"] ?? "", name: $0["name"] ?? "", description: $0["description"] ?? "", isDefault: $0["default"] == "true") }
        }
        // The desktop's cache, by machine + provider (+ account when it has it).
        let catalogs = desktop["model_catalogs"] as? [[String: Any]] ?? []
        let match = catalogs.first { ($0["machine"] as? String) == machine && ($0["provider"] as? String) == provider && ($0["account"] as? String) == account }
            ?? catalogs.first { ($0["machine"] as? String) == machine && ($0["provider"] as? String) == provider }
            ?? catalogs.first { ($0["provider"] as? String) == provider }
        guard let match else { return nil }
        let options = ModelCatalog.options(from: match)
        return options.isEmpty ? nil : options
    }

    func cache(machine: String, provider: String, account: String, options: [ModelOption]) {
        guard !options.isEmpty else { return }
        saved.catalogs["\(machine)|\(provider)|\(account)"] = options.map { ["id": $0.id, "name": $0.name, "description": $0.description, "default": $0.isDefault ? "true" : "false"] }
        save()
    }
}
