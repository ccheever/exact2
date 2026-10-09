#if os(macOS)
import AppKit
import WebKit

/// The previewAutomation host's executor (browser-surface part 5; X1 path B; MIT reference, see LICENSE-T3, T3 Code
/// 1e2ecbd975: apps/web/src/components/preview/PreviewAutomationHosts.tsx `handleRequest`, previewNavigationReadiness.ts,
/// previewAutomationErrors.ts; apps/desktop/src/preview/Manager.ts `automationStatus` … `automationWaitFor`).
///
/// The data module consumes the `previewAutomation.connect` stream and plans each request from its preview store
/// (browser-automation.ts: target resolution, open reuse, the host wait budget, presentation); it hands the plan to
/// this module once (`browserAutomation`) and forgets it. The work waits on the page (readiness, navigation, the
/// agent's input) and answers `previewAutomation.respond` on the connection itself, so no data-source answer has
/// to live as long as a request (a newer answer lets the old one go, and a let-go answer's replies are dropped).
/// A plan delivered twice (an answer let go after sending it) runs once: requests are keyed by connection and id.
///
/// WebKit in place of CDP: scripts run with `callAsyncJavaScript` in the host's content world, `preview_evaluate`
/// in the page's world; a page in a window gets native `NSEvent` clicks and keys (trusted), a page in no window
/// gets DOM events (untrusted); the screenshot is `takeSnapshot`; locators and the ARIA snapshot come from
/// Playwright's injected script (assets/vendor/playwright). Recording is part 3's; a fixed viewport is part 2's.
final class T3BrowserAutomation: NSObject {
    typealias Reply = ([String: Any]) -> Void
    private weak var sessions: T3BrowserSessions?
    /// One RPC on the connection a request came from: method, payload, connection generation, the fleet transport's
    /// key (nil: the focused connection), reply.
    var rpc: (String, [String: Any], Int, String?, @escaping Reply) -> Void = { _, _, _, _, done in done(["ok": false]) }
    /// Publishes a `t3.status` (the module's state changed).
    var changed: () -> Void = {}
    /// The agent's clicks and keys reach a page in a window as native events, else as DOM events.
    var nativeInput = true

    struct Tab {
        var console: [[String: Any]] = []
        var network: [[String: Any]] = []
        var actions: [[String: Any]] = []
        var audible = false, muted = false
        var colorScheme = "system", controller = "none"
        var pointer: [String: Any]?
    }
    private(set) var tabs: [String: Tab] = [:]
    private var running: Set<String> = []
    private var answered: [String] = []
    /// Tabs this host opened (`preview.open`) for the data module to adopt and present, newest last.
    private(set) var opened: [[String: Any]] = []
    private var pointerSequence = 0, actionSequence = 0
    static let diagnosticLimit = 200
    static let agentCursorMoveMs = 160, agentCursorClickLeadMs = 40
    static let maxEvaluationBytes = 64_000, maxScreenshotWidth = 1_280
    /// browser-automation-keys.ts `CLIPBOARD_PLACEHOLDER`.
    static let clipboardPlaceholder = "__T3_PREVIEW_CLIPBOARD__"

    init(sessions: T3BrowserSessions) {
        self.sessions = sessions
        super.init()
    }

    // MARK: Pages (T3BrowserSessions `created`)

    /// A new page: the diagnostics and media scripts, before its first load.
    func prepare(_ session: T3BrowserSession) {
        let controller = session.web.configuration.userContentController
        let handler = T3BrowserAutomationMessages(owner: self, id: session.id, web: session.web)
        controller.add(handler, contentWorld: .page, name: T3BrowserAutomationScripts.diagnosticsHandler)
        controller.add(handler, contentWorld: T3BrowserAutomationScripts.world, name: T3BrowserAutomationScripts.mediaHandler)
        controller.addUserScript(WKUserScript(source: T3BrowserAutomationScripts.diagnostics, injectionTime: .atDocumentStart, forMainFrameOnly: true, in: .page))
        controller.addUserScript(WKUserScript(source: T3BrowserAutomationScripts.media, injectionTime: .atDocumentStart, forMainFrameOnly: true, in: T3BrowserAutomationScripts.world))
        if tabs[session.id] == nil { tabs[session.id] = Tab() }
    }

    func forget(_ id: String) { tabs[id] = nil }

    fileprivate func message(_ id: String, _ name: String, _ body: Any) {
        guard tabs[id] != nil, let body = body as? [String: Any] else { return }
        if name == T3BrowserAutomationScripts.diagnosticsHandler, let entry = body["entry"] as? [String: Any] {
            if body["kind"] as? String == "console" { push(id, \.console, entry) } else if body["kind"] as? String == "network" { push(id, \.network, entry) }
            return
        }
        guard name == T3BrowserAutomationScripts.mediaHandler else { return }
        if let audible = body["audible"] as? Bool, tabs[id]?.audible != audible { tabs[id]?.audible = audible; changed() }
        if body["ready"] as? Bool == true, tabs[id]?.muted == true { applyMute(id) }
    }
    private func push(_ id: String, _ list: WritableKeyPath<Tab, [[String: Any]]>, _ entry: [String: Any]) {
        tabs[id]?[keyPath: list].append(entry)
        if let count = tabs[id]?[keyPath: list].count, count > Self.diagnosticLimit { tabs[id]?[keyPath: list].removeFirst(count - Self.diagnosticLimit) }
    }

    // MARK: Mute (RightPanelTabs `tabMuteMenuItem`; WebKit: the document's media elements)

    func setMuted(_ id: String, _ muted: Bool) -> Bool {
        guard sessions?.sessions[id] != nil, tabs[id] != nil else { return false }
        tabs[id]?.muted = muted
        applyMute(id)
        changed()
        return true
    }
    private func applyMute(_ id: String) {
        guard let web = sessions?.sessions[id]?.web, let muted = tabs[id]?.muted else { return }
        web.callAsyncJavaScript("return globalThis.__t3PreviewMedia ? globalThis.__t3PreviewMedia.setMuted(muted) : null;", arguments: ["muted": muted], in: nil, in: T3BrowserAutomationScripts.world) { _ in }
    }

    /// The module's status (`presentation.browserAutomation`): each page's audio, appearance, controller and the
    /// agent's last pointer; the tabs this host opened.
    var status: [String: Any] {
        ["browserAutomation": ["tabs": tabs.mapValues { tab -> [String: Any] in
            ["audible": tab.audible, "muted": tab.muted, "colorScheme": tab.colorScheme, "controller": tab.controller, "pointer": tab.pointer ?? NSNull()]
        }, "opened": opened, "answered": Array(answered.suffix(8))]]
    }

    // MARK: Requests

    /// The data module's `browserAutomation` op. Answers at once; the request runs on and responds by itself.
    func perform(_ plan: [String: Any]) -> [String: Any] {
        let context = Context(plan)
        guard !context.requestId.isEmpty, !context.connectionId.isEmpty else { return ["accepted": false] }
        guard !running.contains(context.key), !answered.contains(context.key) else { return ["accepted": false, "duplicate": true] }
        running.insert(context.key)
        note("\(context.operation) \(context.requestId) tab=\(context.tabId ?? "-")")
        Task { @MainActor [weak self] in
            guard let self else { return }
            let outcome = await self.run(context)
            self.respond(context, outcome)
        }
        return ["accepted": true]
    }

    private func respond(_ context: Context, _ outcome: Result<Any?, HostError>) {
        var payload: [String: Any] = ["clientId": context.clientId, "connectionId": context.connectionId, "requestId": context.requestId]
        switch outcome {
        case .success(let value): payload["ok"] = true; if let value { payload["result"] = value }
        case .failure(let error): payload["ok"] = false; payload["error"] = error.json(context)
        }
        running.remove(context.key)
        answered.append(context.key)
        if answered.count > 64 { answered.removeFirst(answered.count - 64) }
        let summary: String = { if case .failure(let error) = outcome { return error.tag } else { return "ok" } }()
        note("\(context.operation) \(context.requestId) \(summary)")
        rpc("previewAutomation.respond", payload, context.generation, context.fleet) { [weak self] reply in
            if reply["ok"] as? Bool != true { self?.note("respond \(context.requestId) failed: \((reply["error"] as? [String: Any])?["message"] ?? "")") }
        }
        changed()
    }

    @MainActor private func run(_ context: Context) async -> Result<Any?, HostError> {
        if let failure = context.failure { return .failure(.planned(failure)) }
        do {
            switch context.operation {
            case "status": return .success(await status(context, tabId: context.tabId, runtimeId: context.runtimeId))
            case "open": return .success(try await open(context))
            case "navigate":
                let session = try await requireReady(context)
                guard let url = (context.plan["navigate"] as? [String: Any])?["url"] as? String, let target = URL(string: url) else { throw HostError.operation("no navigation URL") }
                session.navigate(target)
                let input = context.input
                try await waitForNavigation(context, session, readiness: input["readiness"] as? String ?? "load", timeoutMs: input["timeoutMs"] as? Int ?? context.timeoutMs)
                return .success(await status(context, tabId: context.tabId, runtimeId: context.runtimeId))
            case "resize": return .success(try await resize(context))
            case "setColorScheme":
                let session = try await requireReady(context)
                let scheme = context.input["colorScheme"] as? String ?? "system"
                session.web.appearance = scheme == "dark" ? NSAppearance(named: .darkAqua) : scheme == "light" ? NSAppearance(named: .aqua) : nil
                tabs[session.id]?.colorScheme = scheme
                changed()
                return .success(["tabId": context.tabId ?? "", "colorScheme": scheme])
            case "snapshot": return .success(try await controlled(context, "snapshot") { try await self.snapshot(context, $0) })
            case "click": try await controlled(context, "click") { try await self.click(context, $0) }; return .success([String: Any]())
            case "type": try await controlled(context, "type") { try await self.type(context, $0) }; return .success([String: Any]())
            case "press": try await controlled(context, "press") { try await self.press(context, $0) }; return .success([String: Any]())
            case "scroll": try await controlled(context, "scroll") { try await self.scroll(context, $0) }; return .success([String: Any]())
            case "evaluate": return .success(try await controlled(context, "evaluate") { try await self.evaluate(context, $0) })
            case "waitFor": try await controlled(context, "waitFor") { try await self.waitFor(context, $0) }; return .success([String: Any]())
            case "recordingStart":
                _ = try await requireReady(context)
                throw HostError.operation("recording arrives with browser-surface part 3 (capture)")
            case "recordingStop": throw HostError.recordingNotActive
            default: throw HostError.operation("unsupported operation \(context.operation)")
            }
        } catch let error as HostError { return .failure(error) }
        catch { return .failure(.operation(String(describing: error))) }
    }

    // MARK: Readiness (requireReadyTab, waitForDesktopOverlay, waitForNavigationReadiness)

    /// The tab's page, once the data module has made it, within the request's host deadline (the overlay wait).
    @MainActor func requireReady(_ context: Context, tabId: String? = nil, runtimeId: String? = nil) async throws -> T3BrowserSession {
        guard let runtimeId = runtimeId ?? context.runtimeId, (tabId ?? context.tabId) != nil else { throw HostError.targetUnavailable }
        let budget = max(0, Int((context.deadline.timeIntervalSinceNow * 1000).rounded()))
        let ready = await Self.waitForHostReadiness(deadline: context.deadline) { [weak self] in
            guard let session = self?.sessions?.sessions[runtimeId] else { return false }
            return !session.closed
        }
        guard ready, let session = sessions?.sessions[runtimeId] else { throw HostError.overlayTimeout(budget) }
        return session
    }

    /// previewAutomationHostBudget `waitForHostReadiness`: probes and polling delays share the deadline.
    @MainActor static func waitForHostReadiness(deadline: Date, _ isReady: () -> Bool) async -> Bool {
        while Date() < deadline {
            if isReady() { return true }
            let wait = min(0.05, deadline.timeIntervalSinceNow)
            if wait <= 0 { break }
            try? await Task.sleep(nanoseconds: UInt64(wait * 1_000_000_000))
        }
        return false
    }

    /// `assertPreviewRuntimeCurrent`: the page is still the tab's.
    private func current(_ session: T3BrowserSession) throws {
        guard sessions?.sessions[session.id] === session, !session.closed else { throw HostError.targetUnavailable }
    }

    @MainActor private func waitForNavigation(_ context: Context, _ session: T3BrowserSession, readiness: String, timeoutMs: Int) async throws {
        try current(session)
        if readiness == "none" { return }
        let deadline = Date().addingTimeInterval(Double(timeoutMs) / 1000)
        while Date() <= deadline {
            try current(session)
            if readiness == "domContentLoaded" {
                let state = try? await session.web.callAsyncJavaScript(T3BrowserAutomationScripts.readyState, arguments: [:], in: nil, contentWorld: T3BrowserAutomationScripts.world) as? String
                if state == "interactive" || state == "complete" { return }
            } else if session.navigation.kind != "Loading" { return }
            try? await Task.sleep(nanoseconds: 50_000_000)
        }
        throw HostError.navigationTimeout(readiness: readiness, ms: timeoutMs)
    }

    // MARK: Status and open (currentStatus, the "open" case)

    @MainActor func status(_ context: Context, tabId: String?, runtimeId: String?) async -> [String: Any] {
        var value: [String: Any] = ["tabId": tabId ?? NSNull()]
        if tabId != nil, let setting = context.plan["viewportSetting"] as? [String: Any] { value["viewportSetting"] = setting }
        if let runtimeId, let session = sessions?.sessions[runtimeId], !session.closed {
            let visible = Self.visible(session)
            let nav = session.navigation
            value["available"] = true
            value["visible"] = visible
            value["url"] = nav.url.isEmpty ? (session.web.url?.absoluteString ?? NSNull()) : nav.url
            value["title"] = (session.web.title ?? "").isEmpty ? NSNull() : session.web.title!
            value["loading"] = nav.kind == "Loading"
            if visible, let viewport = await measure(session) { value["viewport"] = viewport }
            return value
        }
        let fallback = context.plan["fallback"] as? [String: Any] ?? [:]
        value["available"] = false
        value["visible"] = false
        value["url"] = fallback["url"] as? String ?? NSNull()
        value["title"] = fallback["title"] as? String ?? NSNull()
        value["loading"] = fallback["loading"] as? Bool ?? false
        return value
    }

    /// The page shows: the tab's view holds it in a visible window (the reference's browser surface `visible`).
    static func visible(_ session: T3BrowserSession) -> Bool {
        guard let window = session.web.window, session.web.superview != nil else { return false }
        return window.isVisible && !session.web.isHiddenOrHasHiddenAncestor
    }

    @MainActor private func measure(_ session: T3BrowserSession) async -> [String: Int]? {
        guard let value = try? await session.web.callAsyncJavaScript(T3BrowserAutomationScripts.viewport, arguments: [:], in: nil, contentWorld: T3BrowserAutomationScripts.world) as? [String: Any],
              let width = (value["width"] as? NSNumber)?.intValue, let height = (value["height"] as? NSNumber)?.intValue, width > 0, height > 0 else { return nil }
        return ["width": width, "height": height]
    }

    @MainActor private func open(_ context: Context) async throws -> [String: Any] {
        let plan = context.plan["open"] as? [String: Any] ?? [:]
        var tabId = plan["tabId"] as? String, runtimeId = plan["runtimeId"] as? String
        let reused = tabId != nil
        var needsOverlay = plan["needsOverlay"] as? Bool ?? false
        if !reused {
            guard let create = plan["create"] as? [String: Any] else { throw HostError.operation("no tab to reuse and nothing to create") }
            let reply = await call("preview.open", create, context)
            guard reply["ok"] as? Bool == true, let snapshot = reply["value"] as? [String: Any], let created = snapshot["tabId"] as? String else {
                throw HostError.operation("preview.open failed: \((reply["error"] as? [String: Any])?["message"] ?? "")")
            }
            tabId = created
            runtimeId = Self.runtimeId(environment: context.environmentId, thread: context.threadId, epoch: plan["epoch"] as? String, tab: created)
            let idle = (snapshot["navStatus"] as? [String: Any])?["_tag"] as? String ?? "Idle"
            needsOverlay = create["url"] != nil || idle != "Idle" // previewAutomationOpenNeedsOverlay
            opened.append(["requestId": context.requestId, "connectionId": context.connectionId, "environmentId": context.environmentId, "threadId": context.threadId, "fleet": context.fleet ?? "",
                           "epoch": plan["epoch"] ?? NSNull(), "snapshot": snapshot, "present": plan["present"] as? Bool ?? false])
            if opened.count > 16 { opened.removeFirst(opened.count - 16) }
            note("opened \(created) for \(context.requestId)")
            changed()
        }
        if needsOverlay { _ = try await requireReady(context, tabId: tabId, runtimeId: runtimeId) }
        if plan["present"] as? Bool == true, let runtimeId {
            // waitForPreviewPresentation: settle briefly so an active-thread open reports visible=true.
            let settle = Date().addingTimeInterval(0.5)
            _ = await Self.waitForHostReadiness(deadline: settle) { [weak self] in self?.sessions?.sessions[runtimeId].map(Self.visible) ?? false }
        }
        if reused, let url = (plan["url"] as? String).flatMap(URL.init(string:)), let runtimeId {
            let session = try await requireReady(context, tabId: tabId, runtimeId: runtimeId)
            session.navigate(url)
            try await waitForNavigation(context, session, readiness: "load", timeoutMs: context.timeoutMs)
        }
        return await status(context, tabId: tabId, runtimeId: runtimeId)
    }

    /// previewRuntimeTabId: `JSON.stringify([environmentId, threadId, serverEpoch, tabId])`.
    static func runtimeId(environment: String, thread: String, epoch: String?, tab: String) -> String {
        let parts: [Any] = [environment, thread, epoch.map { $0 as Any } ?? NSNull(), tab]
        guard let data = try? JSONSerialization.data(withJSONObject: parts, options: [.withoutEscapingSlashes]), let text = String(data: data, encoding: .utf8) else { return "" }
        return text
    }

    @MainActor private func call(_ method: String, _ payload: [String: Any], _ context: Context) async -> [String: Any] {
        await withCheckedContinuation { continuation in
            rpc(method, payload, context.generation, context.fleet) { reply in DispatchQueue.main.async { continuation.resume(returning: reply) } }
        }
    }

    // MARK: Resize (the "resize" case; a fixed viewport is part 2's)

    @MainActor private func resize(_ context: Context) async throws -> [String: Any] {
        let session = try await requireReady(context)
        guard let setting = context.plan["viewport"] as? [String: Any], let tag = setting["_tag"] as? String else { throw HostError.operation("no viewport setting") }
        // Hook: browser-surface part 2 (navigation) renders a freeform or preset viewport (the device toolbar).
        guard tag == "fill" || T3BrowserViewport.renders(setting, session) else { throw HostError.operation("a \(tag) viewport arrives with browser-surface part 2 (the device toolbar)") }
        let reply = await call("preview.resize", ["threadId": context.threadId, "tabId": context.tabId ?? "", "viewport": setting], context)
        guard reply["ok"] as? Bool == true else { throw HostError.operation("preview.resize failed") }
        let timeoutMs = context.input["timeoutMs"] as? Int ?? context.timeoutMs
        let deadline = Date().addingTimeInterval(Double(timeoutMs) / 1000)
        var measured: [String: Int]?
        while Date() <= deadline {
            try current(session)
            if let viewport = await measure(session), T3BrowserViewport.matches(setting, viewport) { measured = viewport; break }
            try? await Task.sleep(nanoseconds: 50_000_000)
        }
        guard let measured else { throw HostError.viewportTimeout(timeoutMs) }
        return ["tabId": context.tabId ?? "", "setting": setting, "viewport": measured]
    }

    // MARK: The control session (withControlSession): the controller, the action timeline

    @MainActor private func controlled<A>(_ context: Context, _ action: String, _ body: (T3BrowserSession) async throws -> A) async throws -> A {
        let session = try await requireReady(context)
        actionSequence += 1
        let id = "action-\(actionSequence)", started = Self.iso()
        var entry: [String: Any] = ["id": id, "action": action, "status": "running", "startedAt": started]
        tabs[session.id]?.actions.append(entry)
        if let count = tabs[session.id]?.actions.count, count > Self.diagnosticLimit { tabs[session.id]?.actions.removeFirst(count - Self.diagnosticLimit) }
        tabs[session.id]?.controller = "agent"
        changed()
        // A page that focuses an element makes its view the window's first responder; the agent never takes the
        // person's focus (previewClickFocus.ts `runPreviewClickKeepingHostFocus`, for every action here).
        let window = session.web.window, focus = window?.firstResponder
        defer {
            tabs[session.id]?.controller = "none"; changed()
            if let window, focus !== session.web, window.firstResponder === session.web { T3BrowserAutomationInput.restore(focus, in: window) }
        }
        do {
            let value = try await body(session)
            entry["status"] = "succeeded"; entry["completedAt"] = Self.iso()
            replace(session.id, entry)
            return value
        } catch {
            entry["status"] = "failed"; entry["completedAt"] = Self.iso(); entry["error"] = (error as? HostError)?.tag ?? "PreviewAutomationExecutionError"
            replace(session.id, entry)
            throw error
        }
    }
    private func replace(_ id: String, _ entry: [String: Any]) {
        guard let index = tabs[id]?.actions.lastIndex(where: { $0["id"] as? String == entry["id"] as? String }) else { return }
        tabs[id]?.actions[index] = entry
    }

    // MARK: Page scripts

    @MainActor func script(_ session: T3BrowserSession, _ body: String, world: WKContentWorld = T3BrowserAutomationScripts.world) async throws -> Any? {
        do { return try await session.web.callAsyncJavaScript(body, arguments: [:], in: nil, contentWorld: world) }
        catch { throw HostError.operation("script: \((error as NSError).userInfo["WKJavaScriptExceptionMessage"] ?? error.localizedDescription)") }
    }

    /// ensurePlaywrightInjected: the injected script in the host's world, once per document.
    @MainActor func ensurePlaywright(_ session: T3BrowserSession) async throws {
        if try await script(session, T3BrowserAutomationScripts.playwrightInstalled) as? Bool == true { return }
        guard let source = T3BrowserAutomationScripts.playwrightSource else { throw HostError.operation("the Playwright injected script is missing from the app's assets") }
        do { _ = try await session.web.evaluateJavaScript(T3BrowserAutomationScripts.playwrightInstall(source), in: nil, contentWorld: T3BrowserAutomationScripts.world) }
        catch { throw HostError.operation("Playwright install: \(error.localizedDescription)") }
    }

    static func locator(_ input: [String: Any]) -> String? {
        if let locator = input["locator"] as? String { return locator }
        if let selector = input["selector"] as? String { return "css=\(selector)" }
        return nil
    }
    static func selectorDiagnostics(_ input: [String: Any]) -> (kind: String, length: Int?) {
        if let locator = input["locator"] as? String { return ("locator", locator.count) }
        if let selector = input["selector"] as? String { return ("selector", selector.count) }
        return ("focused-element", nil)
    }

    // MARK: Snapshot

    @MainActor private func snapshot(_ context: Context, _ session: T3BrowserSession) async throws -> [String: Any] {
        try? await ensurePlaywright(session) // the ARIA snapshot only; the server drops it from the agent's text
        guard let page = try await script(session, T3BrowserAutomationScripts.snapshot) as? [String: Any] else { throw HostError.operation("snapshot script") }
        let shot = try await screenshot(session)
        let tab = tabs[session.id] ?? Tab()
        return ["url": page["url"] as? String ?? "", "title": page["title"] as? String ?? "", "loading": page["loading"] as? Bool ?? false,
                "visibleText": page["visibleText"] as? String ?? "", "interactiveElements": page["interactiveElements"] as? [Any] ?? [],
                "accessibilityTree": page["accessibilityTree"] ?? NSNull(), "consoleEntries": tab.console, "networkEntries": tab.network,
                "actionTimeline": tab.actions, "screenshot": shot]
    }

    /// The page as a PNG at most 1,280 pixels wide (`MAX_SCREENSHOT_WIDTH`), from `takeSnapshot`.
    @MainActor func screenshot(_ session: T3BrowserSession) async throws -> [String: Any] {
        let configuration = WKSnapshotConfiguration()
        configuration.afterScreenUpdates = true
        let image: NSImage
        do { image = try await session.web.takeSnapshot(configuration: configuration) }
        catch { throw HostError.operation("takeSnapshot: \(error.localizedDescription)") }
        guard let source = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else { throw HostError.operation("no snapshot image") }
        let width = min(source.width, Self.maxScreenshotWidth), height = max(1, Int((Double(source.height) * Double(width) / Double(max(1, source.width))).rounded()))
        guard let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0),
              let graphics = NSGraphicsContext(bitmapImageRep: rep) else { throw HostError.operation("no bitmap") }
        graphics.cgContext.interpolationQuality = .high
        graphics.cgContext.draw(source, in: CGRect(x: 0, y: 0, width: width, height: height))
        guard let png = rep.representation(using: .png, properties: [:]) else { throw HostError.operation("no PNG") }
        return ["mimeType": "image/png", "data": png.base64EncodedString(), "width": width, "height": height]
    }

    // MARK: Click (performAutomationClick)

    @MainActor private func click(_ context: Context, _ session: T3BrowserSession) async throws {
        let input = context.input
        let point: (x: Double, y: Double)
        if let locator = Self.locator(input) {
            try await ensurePlaywright(session)
            guard let result = try await script(session, T3BrowserAutomationScripts.clickPoint(locator)) as? [String: Any] else { throw HostError.operation("click target") }
            if result["invalidSelector"] != nil { throw HostError.operation("invalid selector") }
            if result["notFound"] != nil { throw HostError.operation("target not found") }
            point = ((result["x"] as? NSNumber)?.doubleValue ?? 0, (result["y"] as? NSNumber)?.doubleValue ?? 0)
        } else {
            point = ((input["x"] as? NSNumber)?.doubleValue ?? 0, (input["y"] as? NSNumber)?.doubleValue ?? 0)
        }
        guard let viewport = await measure(session) else { throw HostError.operation("no viewport") }
        if point.x < 0 || point.y < 0 || point.x > Double(viewport["width"]!) || point.y > Double(viewport["height"]!) { throw HostError.operation("coordinates outside the viewport") }
        pointer(session.id, "move", point)
        try? await Task.sleep(nanoseconds: UInt64(Self.agentCursorMoveMs) * 1_000_000)
        pointer(session.id, "click", point)
        try? await Task.sleep(nanoseconds: UInt64(Self.agentCursorClickLeadMs) * 1_000_000)
        try current(session)
        if nativeInput, Self.visible(session), let window = session.web.window {
            try await delivered(session, "mouseup") { T3BrowserAutomationInput.click(session.web, window: window, x: point.x, y: point.y) }
        } else {
            let result = try await script(session, T3BrowserAutomationScripts.syntheticClick(x: point.x, y: point.y), world: .page) as? [String: Any]
            if result?["missed"] != nil { throw HostError.operation("nothing at the point") }
        }
    }

    private func pointer(_ id: String, _ phase: String, _ point: (x: Double, y: Double)) {
        pointerSequence += 1
        tabs[id]?.pointer = ["phase": phase, "x": point.x, "y": point.y, "sequence": pointerSequence, "createdAt": Self.iso()]
        if let web = sessions?.sessions[id]?.web { T3BrowserAgentCursor.show(on: web, phase: phase, x: point.x, y: point.y, sequence: pointerSequence, controller: tabs[id]?.controller ?? "none") }
        changed()
    }

    /// Native events reach the page after the call returns: the answer waits until the page has handled them
    /// (CDP's dispatch resolves once the renderer has; the reference's native key receipt), at most 5 s.
    @MainActor private func delivered(_ session: T3BrowserSession, _ event: String, _ send: () throws -> Void) async throws {
        _ = try await script(session, T3BrowserAutomationScripts.armReceipt(event))
        try send()
        let received = try await script(session, T3BrowserAutomationScripts.awaitReceipt) as? Bool
        if received != true { throw HostError.operation("the page did not confirm the \(event)") }
    }

    // MARK: Type (typeIntoAutomationTarget: editing in the page, as the reference does for hidden guests too)

    @MainActor private func type(_ context: Context, _ session: T3BrowserSession) async throws {
        let input = context.input, locator = Self.locator(input)
        if locator != nil { try await ensurePlaywright(session) }
        let result = try await script(session, T3BrowserAutomationScripts.type(locator: locator, text: input["text"] as? String ?? "", clear: input["clear"] as? Bool ?? false)) as? [String: Any] ?? [:]
        if result["invalidSelector"] != nil { throw HostError.operation("invalid selector") }
        if result["notFound"] != nil { throw HostError.operation("target not found") }
        if result["notEditable"] != nil { let diagnostics = Self.selectorDiagnostics(input); throw HostError.notEditable(kind: diagnostics.kind, length: diagnostics.length) }
    }

    // MARK: Press (performAutomationPress)

    @MainActor private func press(_ context: Context, _ session: T3BrowserSession) async throws {
        guard let key = context.plan["key"] as? [String: Any] else { throw HostError.operation("no key sequence") }
        if let editing = key["editing"] as? String, !editing.isEmpty {
            // macOS editing shortcuts edit the focused element in the page, without native focus.
            // The data module's expression (`previewAutomationEditingCommandExpression`) names the clipboard's formats
            // as a placeholder; the system clipboard is read here, as the reference's main process reads it.
            let paste = (key["commands"] as? [String])?.contains("paste") == true
            let formats = (try? JSONSerialization.data(withJSONObject: paste ? T3BrowserAutomationInput.clipboard() : [])).flatMap { String(data: $0, encoding: .utf8) } ?? "[]"
            do { _ = try await session.web.evaluateJavaScript(editing.replacingOccurrences(of: Self.clipboardPlaceholder, with: formats), in: nil, contentWorld: .page) }
            catch { throw HostError.operation("editing: \((error as NSError).userInfo["WKJavaScriptExceptionMessage"] ?? error.localizedDescription)") }
            return
        }
        if nativeInput, Self.visible(session), let window = session.web.window {
            try await delivered(session, "keyup") { try T3BrowserAutomationInput.press(session.web, window: window, key: key) }
        } else if let dom = key["dom"] as? String {
            _ = try await script(session, dom, world: .page)
        }
    }

    // MARK: Scroll, evaluate, wait for

    @MainActor private func scroll(_ context: Context, _ session: T3BrowserSession) async throws {
        let input = context.input, locator = Self.locator(input)
        if locator != nil { try await ensurePlaywright(session) }
        let result = try await script(session, T3BrowserAutomationScripts.scroll(locator: locator, deltaX: (input["deltaX"] as? NSNumber)?.doubleValue ?? 0, deltaY: (input["deltaY"] as? NSNumber)?.doubleValue ?? 0)) as? [String: Any] ?? [:]
        if result["invalidSelector"] != nil { throw HostError.operation("invalid selector") }
        if result["notFound"] != nil { throw HostError.operation("target not found") }
    }

    @MainActor private func evaluate(_ context: Context, _ session: T3BrowserSession) async throws -> Any? {
        let expression = context.input["expression"] as? String ?? ""
        let awaitPromise = context.input["awaitPromise"] as? Bool ?? true
        var text: String?
        do { text = try await session.web.callAsyncJavaScript(T3BrowserAutomationScripts.evaluate(expression, awaitPromise: awaitPromise), arguments: [:], in: nil, contentWorld: .page) as? String }
        catch {
            // A script that is no expression is a SyntaxError in an expression position: run it as a script.
            let message = (error as NSError).userInfo["WKJavaScriptExceptionMessage"] as? String ?? ""
            guard message.contains("SyntaxError") else { throw HostError.operation("evaluation: \(message)") }
            do { text = try await session.web.evaluateJavaScript(T3BrowserAutomationScripts.evaluateScript(expression), in: nil, contentWorld: .page) as? String }
            catch { throw HostError.operation("evaluation: \((error as NSError).userInfo["WKJavaScriptExceptionMessage"] ?? error.localizedDescription)") }
        }
        guard let text, text != T3BrowserAutomationScripts.undefinedMark else { return nil }
        if text.utf8.count > Self.maxEvaluationBytes { throw HostError.operation("result larger than \(Self.maxEvaluationBytes) bytes") }
        return try? JSONSerialization.jsonObject(with: Data(text.utf8), options: [.fragmentsAllowed])
    }

    @MainActor private func waitFor(_ context: Context, _ session: T3BrowserSession) async throws {
        let input = context.input, locator = Self.locator(input)
        if locator != nil { try await ensurePlaywright(session) }
        let timeoutMs = input["timeoutMs"] as? Int ?? 15_000
        let body = T3BrowserAutomationScripts.waitFor(locator: locator, text: input["text"] as? String, urlIncludes: input["urlIncludes"] as? String)
        let deadline = Date().addingTimeInterval(Double(timeoutMs) / 1000)
        while Date() <= deadline {
            try current(session)
            if locator != nil { try? await ensurePlaywright(session) } // a navigation replaced the document
            let result = (try? await script(session, body)) as? [String: Any] ?? [:]
            if result["invalidSelector"] != nil { throw HostError.operation("invalid selector") }
            if result["matched"] as? Bool == true { return }
            try? await Task.sleep(nanoseconds: 100_000_000)
        }
        throw HostError.operation("wait timed out after \(timeoutMs)ms")
    }

    // MARK: Support

    static func iso() -> String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter.string(from: Date())
    }
    private func note(_ line: String) { FileHandle.standardError.write(Data("t3.browser: automation \(line.prefix(300))\n".utf8)) }

    /// One request's plan, read once.
    struct Context {
        let plan: [String: Any]
        let requestId, connectionId, clientId, operation, environmentId, threadId: String
        let tabId, runtimeId: String?
        let input: [String: Any]
        let timeoutMs, generation: Int
        let deadline: Date
        let failure: [String: Any]?
        /// A background environment's fleet transport (nil: the focused connection).
        let fleet: String?
        var key: String { (fleet ?? "") + "\u{0}" + connectionId + "\u{0}" + requestId }
        init(_ plan: [String: Any]) {
            self.plan = plan
            requestId = plan["requestId"] as? String ?? ""; connectionId = plan["connectionId"] as? String ?? ""; clientId = plan["clientId"] as? String ?? ""
            operation = plan["operation"] as? String ?? ""; environmentId = plan["environmentId"] as? String ?? ""; threadId = plan["threadId"] as? String ?? ""
            tabId = plan["tabId"] as? String; runtimeId = plan["runtimeId"] as? String
            input = plan["input"] as? [String: Any] ?? [:]
            timeoutMs = (plan["timeoutMs"] as? NSNumber)?.intValue ?? 15_000
            generation = (plan["generation"] as? NSNumber)?.intValue ?? 0
            // The host wait budget (resolveHostWaitBudgetMs) from the plan's receipt: a data source has no clock.
            let budget = (plan["budgetMs"] as? NSNumber)?.doubleValue ?? Double(timeoutMs)
            deadline = Date().addingTimeInterval(budget / 1000)
            failure = plan["failure"] as? [String: Any]
            fleet = (plan["fleet"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        }
    }
}

/// previewAutomationErrors.ts: the host's errors, answered with the public tag the server classifies
/// (`serializePreviewAutomationHostError`): the message and the context fields as `detail`, never a cause.
enum T3BrowserHostError: Error {
    case planned([String: Any])
    case targetUnavailable
    case overlayTimeout(Int)
    case navigationTimeout(readiness: String, ms: Int)
    case viewportTimeout(Int)
    case recordingNotActive
    case notEditable(kind: String, length: Int?)
    case operation(String)

    var tag: String {
        switch self {
        case .planned(let error): return error["_tag"] as? String ?? "PreviewAutomationExecutionError"
        case .targetUnavailable: return "PreviewAutomationTabNotFoundError"
        case .overlayTimeout, .navigationTimeout, .viewportTimeout: return "PreviewAutomationTimeoutError"
        case .notEditable: return "PreviewAutomationTargetNotEditableError"
        case .recordingNotActive, .operation: return "PreviewAutomationExecutionError"
        }
    }

    func json(_ context: T3BrowserAutomation.Context) -> [String: Any] {
        if case .planned(let error) = self { return error }
        let tab = context.tabId ?? "unassigned"
        let base: [String: Any] = ["requestId": context.requestId, "environmentId": context.environmentId, "threadId": context.threadId]
        var detail = base, message: String
        switch self {
        case .planned: message = ""
        case .targetUnavailable:
            detail["operation"] = context.operation; detail["tabId"] = context.tabId ?? NSNull(); detail["bridgeAvailable"] = true
            message = "Preview automation target for \(context.operation) request \(context.requestId) is unavailable on environment \(context.environmentId) thread \(context.threadId) (tab \(tab), bridge available)."
        case .overlayTimeout(let ms):
            detail["timeoutMs"] = ms
            message = "Preview webview for request \(context.requestId) on environment \(context.environmentId) thread \(context.threadId) did not register within \(ms)ms."
        case .navigationTimeout(let readiness, let ms):
            detail["tabId"] = tab; detail["readiness"] = readiness; detail["timeoutMs"] = ms
            message = "Preview navigation for request \(context.requestId) on environment \(context.environmentId) thread \(context.threadId) tab \(tab) did not reach \(readiness) readiness within \(ms)ms."
        case .viewportTimeout(let ms):
            detail["tabId"] = tab; detail["timeoutMs"] = ms
            message = "Preview viewport for request \(context.requestId) on environment \(context.environmentId) thread \(context.threadId) tab \(tab) was not rendered within \(ms)ms."
        case .recordingNotActive:
            detail["tabId"] = context.tabId ?? NSNull()
            message = "Preview automation request \(context.requestId) found no active recording for tab \(tab) on environment \(context.environmentId) thread \(context.threadId)."
        case .notEditable(let kind, let length):
            detail["operation"] = context.operation; detail["tabId"] = context.tabId ?? NSNull(); detail["selectorKind"] = kind
            if let length { detail["selectorLength"] = length }
            message = "Preview automation \(context.operation) request \(context.requestId) requires an editable target in tab \(tab)."
        case .operation(let cause):
            // The cause stays in the host log (`t3.browser: automation`), never in the response.
            FileHandle.standardError.write(Data("t3.browser: automation \(context.operation) \(context.requestId) cause: \(cause.prefix(300))\n".utf8))
            detail["operation"] = context.operation; detail["tabId"] = context.tabId ?? NSNull()
            message = "Preview automation \(context.operation) request \(context.requestId) failed on environment \(context.environmentId) thread \(context.threadId) (tab \(tab))."
        }
        return ["_tag": tag, "message": message, "detail": detail]
    }
}
typealias HostError = T3BrowserHostError

/// The page's script messages, held weakly (the content controller retains its handlers); a pop-up shares the
/// tab's configuration, so only the tab's own page is heard.
final class T3BrowserAutomationMessages: NSObject, WKScriptMessageHandler {
    weak var owner: T3BrowserAutomation?
    weak var web: WKWebView?
    let id: String
    init(owner: T3BrowserAutomation, id: String, web: WKWebView) { self.owner = owner; self.id = id; self.web = web }
    func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
        guard message.webView === web, message.frameInfo.isMainFrame else { return }
        owner?.message(id, message.name, message.body)
    }
}

/// A fixed viewport (freeform or a device preset) is browser-surface part 2's device toolbar. Part 2 replaces this
/// hook with the view that renders one; until then only Fill renders.
enum T3BrowserViewport {
    static func renders(_ setting: [String: Any], _ session: T3BrowserSession) -> Bool { setting["_tag"] as? String == "fill" }
    /// isPreviewViewportReady: Fill is whatever the panel gives; a fixed size matches within a pixel.
    static func matches(_ setting: [String: Any], _ viewport: [String: Int]) -> Bool {
        guard setting["_tag"] as? String != "fill" else { return true }
        let width = (setting["width"] as? NSNumber)?.intValue ?? 0, height = (setting["height"] as? NSNumber)?.intValue ?? 0
        return abs((viewport["width"] ?? 0) - width) <= 1 && abs((viewport["height"] ?? 0) - height) <= 1
    }
}
#endif
