// Inspect mode on the Apple hosts (LLP 1122 D4, D5; T2c): the design
// tool's `inspect {on}` turns it on in a development build. A tap then
// selects the plan node it lands on, reported from the runner's own `node`
// op, so its rows are the kernel's, as the wasm page's are (host/web/dev.js).
// `outline {sites, plan}` outlines every copy of the selected node. A tap
// never reaches the app while inspect mode is on. The platform halves (the
// banner, the hit test, the outlines) are InspectIOS.swift and
// InspectMac.swift. Compiled into development builds only (`EXACT_INSPECT`,
// host/apple/Package.swift); a production bake has none of it.
#if EXACT_INSPECT
import Foundation

enum Inspect {
    /// Whether inspect mode is on, and the number of the command that set it
    /// (an exit names it, so a later owner's command is not undone).
    nonisolated(unsafe) static var on = false
    nonisolated(unsafe) static var seq: Int?
    /// This inspect session's click tokens (tamber/1.7): a random prefix
    /// and a counter, so the outline answering a click of the session is
    /// known however many are on their way.
    nonisolated(unsafe) static var session: String?
    nonisolated(unsafe) static var clicks = 0
    /// The node last selected, found again by site and instance path on a
    /// refresh after another generation.
    nonisolated(unsafe) static var selected: (session: ObjectIdentifier, generation: Int, id: UInt32, click: String?, site: Int?, plan: String?, instance: String)?
    /// The command a local exit (the banner's close) left, kept until the
    /// server hears it: a replay of that command on registering again is
    /// the exit again, not on (as host/web/dev.js does). Process memory: a
    /// relaunched app is a new build id, which the server never tells an
    /// earlier build's command (scripts/tamber-builds.mjs keys inspect mode
    /// by build id), so nothing is left to replay, and no seq a restarted
    /// server reuses is mistaken for it.
    nonisolated(unsafe) static var left: Int?
    /// An instance path as compared across generations: its JSON with sorted keys.
    static func canonical(_ instance: Any?) -> String {
        (try? JSONSerialization.data(withJSONObject: instance ?? [], options: [.sortedKeys])).flatMap { String(data: $0, encoding: .utf8) } ?? "[]"
    }
    /// 60 KiB, leaving room under the server's 64 for the plan and nonce added on the way.
    static let reportBytes = 60 * 1024
    /// The digest of the plan the app runs, as its first session's runner
    /// said it in that generation (BuildChannel.planDigest).
    nonisolated(unsafe) static var digest: (session: ObjectIdentifier, generation: Int, digest: String?)?

    /// A `selected` report for node `id` of `s`: the runner's `node` op
    /// with its plan digest, trimmed to 64 KiB, or an `unsupported` one.
    static func report(_ s: ExactSession, id: UInt32, click: String?) -> [String: Any] {
        guard let data = s.agent("{\"op\":\"node\",\"id\":\(id),\"plan\":true}").data(using: .utf8),
              let node = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return ["kind": "unsupported", "what": "inspect", "detail": "node #\(id): unreadable"] }
        if let error = node["error"] as? String { return ["kind": "unsupported", "what": "inspect", "detail": error] }
        let style = node["style"] as? [String: Any] ?? [:]
        var rows: [[String: Any]] = style.keys.sorted().map { name in ["name": name].merging(style[name] as? [String: Any] ?? [:]) { a, _ in a } }
        var selected: [String: Any] = ["node": Int(id), "type": node["type"] ?? NSNull(), "testId": (node["props"] as? [String: Any])?["testId"] ?? NSNull(),
                                       "site": node["site"] ?? NSNull(), "instance": node["instance"] ?? [], "planDigest": node["planDigest"] ?? NSNull(),
                                       "tuple": node["modes"] ?? NSNull(), "rows": rows, "trimmed": false]
        func envelope() -> [String: Any] {
            var body: [String: Any] = ["kind": "selected", "selected": selected, "design": BuildChannel.design ?? NSNull(), "revision": BuildChannel.revision.map { $0 as Any } ?? NSNull()]
            if let seq { body["inspect"] = seq }
            if let click { body["click"] = click }
            return body
        }
        func bytes() -> Int { (try? JSONSerialization.data(withJSONObject: envelope()))?.count ?? Int.max }
        if bytes() > reportBytes {
            rows = rows.map { $0.filter { ["name", "value", "source", "from", "token"].contains($0.key) } }
            selected["rows"] = rows; selected["trimmed"] = true
        }
        while bytes() > reportBytes, !rows.isEmpty { rows = Array(rows.prefix(rows.count / 2)); selected["rows"] = rows }
        if bytes() > reportBytes { return ["kind": "unsupported", "what": "inspect", "detail": "the selected node's identity alone is larger than a report"] }
        return envelope()
    }

    /// A click's selection: its token minted, the node remembered, the
    /// report made. Nil when inspect mode is off.
    static func select(_ s: ExactSession, id: UInt32, clicked: Bool, click: String? = nil) -> [String: Any]? {
        guard on else { return nil }
        var click = click
        if clicked {
            if session == nil { session = String(UInt64.random(in: 1...UInt64.max), radix: 36) }
            clicks += 1
            click = "\(session!).\(clicks)"
        }
        let body = report(s, id: id, click: click)
        if let sel = body["selected"] as? [String: Any] {
            selected = (ObjectIdentifier(s), s.generation, id, click, sel["site"] as? Int, sel["planDigest"] as? String, canonical(sel["instance"]))
        }
        return body
    }

    /// Whether an outline answers a click of this inspect session: then it
    /// scrolls nothing (the copy clicked is under the finger).
    static func answersClick(_ frame: [String: Any]) -> Bool {
        guard let session, let click = frame["click"] as? String else { return false }
        return click.hasPrefix("\(session).")
    }

    /// Inspect mode set by a command (`inspect {on, seq}`), or by the
    /// banner's close (`local`), which reports the exit.
    static func set(_ value: Bool, seq number: Int?) {
        if number != seq { selected = nil }
        seq = number
        guard value != on else { return }
        on = value
        if !value { session = nil; selected = nil }
    }

    /// The sites of the plan views of `s`: `{planDigest, sites: [[view, site]]}`
    /// from the runner's `sites` op.
    static func sites(_ s: ExactSession) -> (plan: String?, views: [(id: UInt32, site: Int)]) {
        guard let data = s.agent("{\"op\":\"sites\"}").data(using: .utf8),
              let o = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return (nil, []) }
        let pairs = (o["sites"] as? [[Any]] ?? []).compactMap { p -> (UInt32, Int)? in
            guard p.count == 2, let v = (p[0] as? NSNumber)?.uint32Value, let site = (p[1] as? NSNumber)?.intValue else { return nil }
            return (v, site)
        }
        return (o["planDigest"] as? String, pairs.map { (id: $0.0, site: $0.1) })
    }
}

#if !os(tvOS) // (tvOS has no inspect mode: no touches, no UIKit close button)
extension ExactApp {
    /// `inspect {on, seq}` or `{refresh}`, and `outline`, from the build
    /// channel (BuildChannel.swift).
    func inspectCommand(_ frame: [String: Any]) {
        if frame["kind"] as? String == "outline" { InspectHost.outline(self, frame); return }
        if frame["refresh"] as? Bool == true {
            // (`status: true`: a status lost on the way; sent again first.)
            if frame["status"] as? Bool == true { BuildChannel.lastStatus = nil; scheduleStatus() }
            DispatchQueue.main.async { [weak self] in self?.refreshSelection() }
            return
        }
        let on = frame["on"] as? Bool == true, seq = (frame["seq"] as? NSNumber)?.intValue
        // A replay of the command this build left itself, before the server heard the exit.
        if on, let seq, seq == Inspect.left { reportExit(seq); return }
        Inspect.left = nil
        Inspect.set(on, seq: seq)
        InspectHost.show(self, on: on)
    }

    private func reportExit(_ seq: Int?) {
        connection?.report(["kind": "selected", "selected": NSNull(), "inspect": seq.map { $0 as Any } ?? NSNull(), "plan": connection?.plan ?? NSNull(), "revision": BuildChannel.revision.map { $0 as Any } ?? NSNull()])
    }

    /// A selection report, under the generation the app runs.
    func reportSelected(_ body: [String: Any]) {
        var body = body
        if body["kind"] as? String == "selected" { body["plan"] = connection?.plan ?? NSNull() }
        connection?.report(body)
    }

    /// A tap in inspect mode on node `id` of `s`.
    func inspectTapped(_ s: ExactSession, id: UInt32) {
        if let body = Inspect.select(s, id: id, clicked: true) { reportSelected(body) }
    }

    /// The banner's close (D4's local way out): inspect mode off here, and
    /// the exit reported under the command it left.
    func leaveInspect() {
        guard Inspect.on else { return }
        let seq = Inspect.seq
        Inspect.left = seq
        Inspect.set(false, seq: seq)
        InspectHost.show(self, on: false)
        reportExit(seq)
    }

    /// The selection reported again (the server's `{refresh}`): its node in
    /// the same generation, or the one node at its site and instance path
    /// in a later generation of the same plan; else it is said gone (a new
    /// runner numbers its views afresh, so an id alone names nothing then).
    func refreshSelection() {
        guard Inspect.on, let sel = Inspect.selected, let s = sessions.first(where: { ObjectIdentifier($0) == sel.session }) else { return }
        var id = sel.id
        if s.generation != sel.generation || s.presenter.views[id] == nil {
            let now = Inspect.sites(s)
            guard now.plan == sel.plan, let site = sel.site else {
                Inspect.selected = nil
                connection?.report(["kind": "unsupported", "what": "inspect", "detail": "the selected node is gone in this generation: select it again"])
                return
            }
            let found = now.views.filter { $0.site == site }.map(\.id).filter { candidate in
                guard let data = s.agent("{\"op\":\"node\",\"id\":\(candidate)}").data(using: .utf8),
                      let node = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] else { return false }
                return Inspect.canonical(node["instance"]) == sel.instance
            }
            guard found.count == 1 else {
                Inspect.selected = nil
                connection?.report(["kind": "unsupported", "what": "inspect", "detail": "the selected node is gone in this generation: select it again"])
                return
            }
            id = found[0]
        }
        if let body = Inspect.select(s, id: id, clicked: false, click: sel.click) { reportSelected(body) }
    }
}
#endif
#endif
