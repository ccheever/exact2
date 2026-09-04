// The hostile sample host, macOS (LLP 1031 D10): a native AppKit app that is
// not Exact's — a split view whose left pane is a native table — hosting
// two sessions of the one plan the archive carries, side by side. It is the
// fixture `scripts/smoke.mjs host` drives through the existing carrier (each
// request names its session by label — routing, not a ninth operation); the
// external consumer is the proof. What it exercises, in the smoke's order:
// two sessions with overlapping node ids answering apart; interleaved
// operations; a command from a session pushing a native screen over the
// other (unmount) and popping it (remount) with both sessions intact; a bad
// candidate plan refused with the running apps kept; a session destroyed
// under the other, its handle refused by name after. A control file
// (EXACT_HOST_CONTROL=<path>, one command per appended line) stands in for
// the native buttons a real host has: `destroy <label>`, `unmount <label>`,
// `mount <label>`, `apply <plan path>`.
import AppKit
import ExactKit

let app = NSApplication.shared
app.setActivationPolicy(ExactEnv.agentMode ? .accessory : .regular)
setvbuf(stdout, nil, _IOLBF, 0)
let exact = ExactApp.shared
/// The host's own lines go to stderr: under the agent, stdout is the protocol.
func log(_ line: String) { FileHandle.standardError.write(Data((line + "\n").utf8)) }

/// The native side: rows the host owns, unrelated to Exact.
final class NativeRows: NSObject, NSTableViewDataSource, NSTableViewDelegate {
    let rows = ["Native row 1", "Native row 2", "Native row 3", "Native row 4", "Native row 5"]
    func numberOfRows(in tableView: NSTableView) -> Int { rows.count }
    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let cell = NSTextField(labelWithString: rows[row])
        cell.identifier = NSUserInterfaceItemIdentifier("row")
        return cell
    }
}
let rows = NativeRows()
let table = NSTableView(frame: .zero)
let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("native"))
column.title = "Native"
column.width = 160
table.addTableColumn(column)
table.dataSource = rows
table.delegate = rows
let tableScroll = NSScrollView(frame: .zero)
tableScroll.documentView = table
tableScroll.hasVerticalScroller = true

/// Two sessions of the one plan (LLP 1031 D1), each in its own pane, each
/// with its own clock under the agent.
let a = exact.makeSession(label: "a")
let b = exact.makeSession(label: "b")
if ExactEnv.agentMode { a.clock = 0; b.clock = 0 }
let viewA = ExactView(session: a)
let viewB = ExactView(session: b)
let sessions: [(String, ExactSession)] = [("a", a), ("b", b)]
var views: [String: ExactView] = ["a": viewA, "b": viewB]
/// Where each session's pane sits: a container the host owns, so a native
/// screen can replace the Exact view inside it (a push) and give it back (a pop).
var panes: [String: NSView] = [:]

let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1040, height: 720), styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
window.title = "Host (not Exact)"
let split = NSSplitView(frame: window.contentView!.bounds)
split.isVertical = true
split.dividerStyle = .thin
split.autoresizingMask = [.width, .height]
// The split view shares its width in proportion to the panes' initial
// frames, so each pane starts at the width it should keep.
tableScroll.frame = NSRect(x: 0, y: 0, width: 180, height: 720)
split.addArrangedSubview(tableScroll)
for (i, (label, view)) in [("a", viewA), ("b", viewB)].enumerated() {
    let pane = NSView(frame: NSRect(x: 180 + i * 430, y: 0, width: 430, height: 720))
    view.frame = pane.bounds
    view.autoresizingMask = [.width, .height]
    pane.addSubview(view)
    panes[label] = pane
    split.addArrangedSubview(pane)
}
split.adjustSubviews()
window.contentView = split
window.initialFirstResponder = viewA
window.autorecalculatesKeyViewLoop = false
window.center()

/// A native "screen" pushed over a session's pane: the Exact view leaves
/// its container (unmounted, its session alive and unmounted, D1), a native
/// view takes its place; a pop reverses it.
final class NativeScreen: NSView {
    let label: String
    init(over label: String) {
        self.label = label
        super.init(frame: .zero)
        wantsLayer = true
        layer?.backgroundColor = NSColor.windowBackgroundColor.cgColor
        let text = NSTextField(labelWithString: "Native screen over \(label)")
        text.frame = NSRect(x: 16, y: 16, width: 300, height: 24)
        addSubview(text)
    }
    required init?(coder: NSCoder) { nil }
}
var pushed: [String: NativeScreen] = [:]
func push(over label: String) {
    guard pushed[label] == nil, let pane = panes[label], let view = views[label] else { return }
    view.removeFromSuperview()
    let screen = NativeScreen(over: label)
    screen.frame = pane.bounds
    screen.autoresizingMask = [.width, .height]
    pane.addSubview(screen)
    pushed[label] = screen
    log("host: pushed a native screen over \(label)")
}
func pop(_ label: String) {
    guard let screen = pushed.removeValue(forKey: label), let pane = panes[label], let view = views[label] else { return }
    screen.removeFromSuperview()
    view.frame = pane.bounds
    pane.addSubview(view)
    log("host: popped the native screen over \(label); \(label) remounted")
}

/// The delegate the host owns: a command is an intention (LLP 1031 D5). Here
/// `setScheme("dark")` from any session pushes a native screen over session
/// a, and `setScheme("light")` pops it — the host decides what a command
/// means, and it happens after the batch that carried it was applied (D2).
final class HostDelegate: ExactSessionDelegate {
    func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
        log("host: command \(name)\(args.isEmpty ? "" : " \(args)") from \(session.label)")
        switch (name, args.first as? String) {
        case ("setScheme", "dark"): push(over: "a")
        case ("setScheme", "light"): pop("a")
        default: break
        }
    }
    func exactSession(_ session: ExactSession, didChange state: ExactSession.State) {
        log("host: \(session.label) is \(state)")
    }
}
let delegate = HostDelegate()
a.delegate = delegate
b.delegate = delegate

/// The control file: the native buttons a real host has, as lines the
/// smoke appends.
var consumed = 0
var control: DispatchSourceTimer?
if let path = ExactEnv.environment["EXACT_HOST_CONTROL"] {
    let t = DispatchSource.makeTimerSource(queue: .main)
    t.schedule(deadline: .now() + 0.1, repeating: 0.1)
    t.setEventHandler {
        guard let text = try? String(contentsOfFile: path, encoding: .utf8) else { return }
        let lines = text.split(separator: "\n").map(String.init)
        guard lines.count > consumed else { return }
        for line in lines[consumed...] {
            let parts = line.split(separator: " ", maxSplits: 1).map(String.init)
            guard let verb = parts.first else { continue }
            let arg = parts.count > 1 ? parts[1] : ""
            switch verb {
            case "destroy":
                if let s = sessions.first(where: { $0.0 == arg })?.1 { s.destroy(); views[arg]?.removeFromSuperview(); log("host: destroyed \(arg)") }
            case "unmount":
                if let v = views[arg] { v.removeFromSuperview(); log("host: unmounted \(arg)") }
            case "mount":
                if let v = views[arg], let pane = panes[arg] { v.frame = pane.bounds; pane.addSubview(v); log("host: mounted \(arg)") }
            case "apply":
                let bytes = FileManager.default.contents(atPath: arg) ?? Data()
                let ok = exact.apply(bytes, label: String(arg.split(separator: "/").last ?? "plan"))
                log("host: apply \(arg): \(ok ? "every session took it" : "refused")")
            default:
                log("host: unknown control \(line)")
            }
        }
        consumed = lines.count
    }
    t.resume()
    control = t
}

final class Delegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
    func windowDidBecomeKey(_ notification: Notification) { agentReady() }
    func windowDidChangeOcclusionState(_ notification: Notification) { for (_, s) in sessions { s.occlusionChanged() } }
}
let appDelegate = Delegate()
app.delegate = appDelegate
window.delegate = appDelegate

// Both boot at their panes' sizes; EXACT_PLAN boots a file plan into both.
for (label, view) in [("a", viewA), ("b", viewB)] {
    let size = view.session.viewportSize
    if let path = ExactEnv.environment["EXACT_PLAN"], let bytes = FileManager.default.contents(atPath: path) {
        view.session.boot(plan: bytes, size: size)
    } else {
        view.session.boot(size: size)
    }
    log("host: \(label) booted in \(String(format: "%.1f", view.session.bootMs)) ms with \(view.session.viewCount) views\(view.session.bootError.map { " — \($0)" } ?? "")")
}
window.makeKeyAndOrderFront(nil)
if ExactEnv.agentMode { window.orderFrontRegardless() } else { app.activate(ignoringOtherApps: true) }

nonisolated(unsafe) var readySent = false
func agentReady() {
    guard ExactEnv.agentMode, !readySent else { return }
    readySent = true
    let error: Any = (a.bootError ?? b.bootError).map { $0 as Any } ?? NSNull()
    Agent.reply(["ready": true, "boot": max(a.bootMs, b.bootMs), "views": a.viewCount + b.viewCount, "sessions": sessions.map(\.0), "error": error])
    Agent.startStdio(sessions: sessions)
}
if ExactEnv.agentMode { DispatchQueue.main.async { agentReady() } }
app.run()
