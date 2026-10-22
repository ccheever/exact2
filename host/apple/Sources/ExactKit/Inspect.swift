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
                                       "tuple": node["modes"] ?? NSNull(), "rows": rows, "held": NSNull(), "trimmed": false]
        MainActor.assumeIsolated { if let held = HeldState.rows(for: id, in: s.presenter) { selected["held"] = held } } // what is held on it (LLP 1122 D8)
        func envelope() -> [String: Any] {
            var body: [String: Any] = ["kind": "selected", "selected": selected, "design": BuildChannel.design ?? NSNull(), "revision": BuildChannel.revision.map { $0 as Any } ?? NSNull()]
            if let seq { body["inspect"] = seq }
            if let click { body["click"] = click }
            return body
        }
        func bytes() -> Int { (try? JSONSerialization.data(withJSONObject: envelope()))?.count ?? Int.max }
        if bytes() > reportBytes {
            rows = rows.map { $0.filter { ["name", "value", "source", "from", "fromSite", "token"].contains($0.key) } }
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
        // `select: {up, sites}` (tamber/1.16) is the web pages' alone so far: said so, inspect mode left as it is.
        if frame["select"] != nil {
            connection?.report(["kind": "unsupported", "what": "inspect", "detail": "this build cannot move its selection up (select): click the node instead"])
            return
        }
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

    /// `hold {revision, hold}` (LLP 1122 D4, D8; *1.11*): `{site, instance,
    /// state}` of the plan the app runs, the one view at that site and
    /// instance path, held as the real state (HeldState.swift); null lets it
    /// go. What it holds or let go, and why, is reported (`held`), and a state
    /// this host can't hold is said so (`unsupported`).
    func holdCommand(_ frame: [String: Any]) {
        let revision = (frame["revision"] as? NSNumber)?.intValue
        // The stream the frame came on (LLP 1122 D4; host/web/dev.js binds its holds the same way): its revisions are
        // its own. A new one (a reconnect, a restarted server, which may number from where an old one did) forgets the
        // revisions ended on the one before, and what a hold of an old stream says later (its let-go) is said to none.
        let stream = holdStream
        let live = { [weak self] in self?.holdStream == stream }
        MainActor.assumeIsolated { if HeldState.stream != stream { HeldState.stream = stream; HeldState.ended = [] } }
        let report: ([String: Any]) -> Void = { [weak self] body in
            guard live() else { return }
            var body = body
            body["revision"] = revision.map { $0 as Any } ?? NSNull()
            self?.connection?.report(body)
            MainActor.assumeIsolated { HeldState.heard?(body) }
        }
        // A revision ended here (input, disabled, gone, a refusal) is not taken again when its frame comes again (a
        // resent answer); a lower one is a new server's, and forgets them.
        let ended = MainActor.assumeIsolated { () -> Bool in
            guard let revision else { return false }
            if HeldState.ended.contains(where: { $0 > revision }) { HeldState.ended = [] }
            return HeldState.ended.contains(revision)
        }
        if ended, frame["hold"] is [String: Any] { return }
        let plan = BuildChannel.planDigest(self), addressed = frame["plan"] as? String
        // A hold addressing another plan (one swapped in since) is nothing, unsaid: the hold held and the frame kept for
        // `rehold` stay as they are, and the server's replay after the status that names this plan corresponds it.
        if frame["hold"] is [String: Any], let addressed, addressed != plan { return }
        MainActor.assumeIsolated { HeldState.frame = frame["hold"] is [String: Any] ? (frame, addressed ?? plan, stream) : nil }
        guard let h = frame["hold"] as? [String: Any], let state = h["state"] as? String else {
            MainActor.assumeIsolated { HeldState.letGo("replaced") }
            report(["kind": "held", "state": NSNull()])
            return
        }
        // A state no host holds is said so by name, and nothing is painted.
        guard MainActor.assumeIsolated({ HeldState.states.contains(state) }) else {
            MainActor.assumeIsolated { HeldState.letGo("replaced"); HeldState.frame = nil; HeldState.end(revision) }
            report(["kind": "unsupported", "what": "hold", "state": state, "detail": "this host holds no such state"])
            return
        }
        // The nodes at that site and instance path, a flat leaf's or an inline run's (no view of its own) included:
        // the press resolves through the logical tree (HeldState.hold).
        let site = (h["site"] as? NSNumber)?.intValue, instance = Inspect.canonical(h["instance"])
        var found: [(id: UInt32, presenter: Presenter)] = []
        for s in sessions where s.state != .destroyed && s.booted {
            for (id, at) in Inspect.sites(s).views where at == site && !found.contains(where: { $0.id == id && $0.presenter === s.presenter }) {
                guard let data = s.agent("{\"op\":\"node\",\"id\":\(id)}").data(using: .utf8),
                      let node = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
                      node["error"] == nil, Inspect.canonical(node["instance"]) == instance else { continue }
                found.append((id, s.presenter))
            }
        }
        // (No one view there: the one held before is let go too.)
        guard found.count == 1 else { MainActor.assumeIsolated { HeldState.letGo("replaced"); HeldState.frame = nil; HeldState.end(revision) }; report(["kind": "held", "state": NSNull(), "reason": "gone"]); return }
        MainActor.assumeIsolated { HeldState.applying = revision }
        defer { MainActor.assumeIsolated { HeldState.applying = nil } }
        let r = MainActor.assumeIsolated { HeldState.hold(state, on: found[0].id, in: found[0].presenter) { reason in
            if reason == "replaced", MainActor.assumeIsolated({ HeldState.applying }) == revision { return }
            guard live() else { return } // (a hold of an old stream: its revision is no longer the server's)
            MainActor.assumeIsolated {
                if HeldState.frame.map({ ($0.frame["revision"] as? NSNumber) == (frame["revision"] as? NSNumber) }) == true { HeldState.frame = nil }
                if reason != "replaced" { HeldState.end(revision) }
            }
            report(["kind": "held", "state": NSNull(), "reason": reason])
        } }
        if r.reason != nil { MainActor.assumeIsolated { HeldState.frame = nil; HeldState.end(revision) } }
        if r.unsupported { MainActor.assumeIsolated { HeldState.frame = nil; HeldState.end(revision) }; report(["kind": "unsupported", "what": "hold", "state": state, "detail": "this host holds no \(state)"]); return }
        report(r.reason.map { ["kind": "held", "state": NSNull(), "reason": $0] } ?? ["kind": "held", "state": state])
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
