// The right panel's native needs. The Files editor (hook `t3-file-editor`) takes the
// focus when a press in the file's body mounts it. It writes exactly what was typed
// through its `autocorrect="off"`, which exact2 #111 maps to no smart quotes, dashes
// or text replacement (the reference's editor is a browser textarea, which never
// substitutes). An attachment's text and Save file are the data module's
// (attachment-files.ts).
import AppKit
import Foundation

enum T3FileEditor {
    static func install(_ element: ExactElement) {
        guard element.hatch == .t3FileEditor, let view = element.textView else { return }
        watchPresses()
        if element.isNew, element.id == fileEditorId { focusFileEditor(element, view) }
    }

    /// The Files editor's textarea (`files-surface.contract`).
    static let fileEditorId = "file-editor"

    /// A press anywhere in a Files editor's body (`file-lines`: below the last line, the gutter)
    /// begins editing and mounts the editor, but the press left the focus where it was (the
    /// composer, which opening a thread focuses) and Exact's focus on the pressed view, which blocks
    /// the textarea's autofocus: typing then went to the composer. The editor mounts only from that
    /// press, so it takes the focus as the reference's editor does (measured on the
    /// f870c41 oracle with @pierre/diffs 1.3.0-beta.10: a click below the last line focuses the
    /// contenteditable with the caret at the end of the last line, whatever the click's x; a click on
    /// a line puts the caret under it). A real press's point is kept (`watchPresses`): below the
    /// text the caret goes to the end, on a line to the character under it; an agent's plain tap
    /// carries no AppKit event, so its caret goes to the end (`tap … mouse at x y` is a real press
    /// the monitor sees since exact2 #186).
    static func focusFileEditor(_ element: ExactElement, _ view: NSTextView, attempts: Int = 20) {
        DispatchQueue.main.async { [weak element, weak view] in
            guard let element, element.isLive, let view else { return }
            guard let window = view.window else {
                if attempts > 0 { DispatchQueue.main.asyncAfter(deadline: .now() + 0.025) { focusFileEditor(element, view, attempts: attempts - 1) } }
                return
            }
            if window.firstResponder !== view {
                element.focus()
                if window.firstResponder !== view { window.makeFirstResponder(view) }
            }
            guard window.firstResponder === view else { return }
            view.setSelectedRange(NSRange(location: caret(in: view, press: recentPress(in: window)), length: 0))
        }
    }

    /// Where a press at `point` (window coordinates) puts the caret: the end below the text (or
    /// with no press), else the insertion point nearest it.
    static func caret(in view: NSTextView, press point: NSPoint?) -> Int {
        let end = (view.string as NSString).length
        guard let point, let manager = view.layoutManager, let container = view.textContainer else { return end }
        let local = view.convert(point, from: nil)
        guard view.bounds.contains(local) else { return end }
        manager.ensureLayout(for: container)
        let used = manager.usedRect(for: container).offsetBy(dx: view.textContainerOrigin.x, dy: view.textContainerOrigin.y)
        if view.isFlipped ? local.y >= used.maxY : local.y <= used.minY { return end }
        return min(end, view.characterIndexForInsertion(at: local))
    }

    /// The last real left press (an AppKit event; an agent's plain tap has none) in a window.
    private static var press: (window: NSWindow, point: NSPoint, at: TimeInterval)?
    private static var pressMonitor: Any?
    static func watchPresses() {
        guard pressMonitor == nil else { return }
        pressMonitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown]) { event in
            if let window = event.window { press = (window, event.locationInWindow, ProcessInfo.processInfo.systemUptime) }
            return event
        }
    }
    /// A press in `window` within the last second (the one that began editing), consumed once.
    static func recentPress(in window: NSWindow) -> NSPoint? {
        guard let last = press, last.window === window, ProcessInfo.processInfo.systemUptime - last.at < 1 else { return nil }
        press = nil
        return last.point
    }
    /// Tests stand in for a press.
    static func recordPress(_ window: NSWindow, at point: NSPoint) { press = (window, point, ProcessInfo.processInfo.systemUptime) }
}

/// PullRequestUnavailableError.message (contracts/pullRequest.ts PROVIDER_REQUIREMENT): the
/// sentence the reference shows when pull requests cannot be read, never the CLI's own output.
enum T3PullRequestErrors {
    private static let requirement: [String: (missing: String, unauthenticated: String)] = [
        "github": ("GitHub CLI (`gh`) is required to browse change requests on this host. Install it from https://cli.github.com/ and reload.",
                   "GitHub CLI is not authenticated. Run `gh auth login` and retry."),
        "forgejo": ("Install Forgejo CLI (`fj` 0.6 or later) from https://codeberg.org/forgejo-contrib/forgejo-cli or Gitea CLI (`tea` 0.16 or later) from https://gitea.com/gitea/tea to browse Forgejo pull requests.",
                    "Authenticate your Forgejo or Gitea server with `fj --host <server-url> auth add-token` on the T3 Code server. If fj is missing or unconfigured for that server, use `tea login add`. A configured fj account must be repaired with fj."),
        "gitlab": ("GitLab CLI (`glab`) is required to browse change requests on this host. Install it from https://gitlab.com/gitlab-org/cli and reload.",
                   "GitLab CLI is not authenticated. Run `glab auth login` and retry."),
        "azure-devops": ("Azure CLI (`az`) with the Azure DevOps extension is required. Install `az`, then run `az extension add --name azure-devops`.",
                         "Azure CLI is not signed in. Run `az login` and retry."),
        "bitbucket": ("Bitbucket needs API credentials on the server. Add them in Settings → Source Control.",
                      "Bitbucket rejected the configured credentials. Check them in Settings → Source Control."),
    ]
    static func unavailable(reason: String, provider: String) -> String {
        let known = requirement[provider]
        switch reason {
        case "cli-missing": return known?.missing ?? "The tool this host is read through is not installed or set up."
        case "cli-unauthenticated": return known?.unauthenticated ?? "This host has no working credentials."
        case "provider-unsupported": return "Change requests cannot be browsed for this project's host yet."
        default: return ""
        }
    }
}
