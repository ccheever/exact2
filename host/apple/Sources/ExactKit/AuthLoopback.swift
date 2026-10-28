// A loopback redirect for `openAuthSession` on macOS (LLP 1133.005 D9, RFC
// 8252 §7.3): the listener library's receiver binds the callback's port
// before the default browser opens the authorization URL, answers the one
// request whose path and `state` match, and hands back the callback URL;
// the window then comes forward. `ASWebAuthenticationSession` is not used:
// its callback matcher takes a private-use scheme or `https` only. The
// receiver is the listener's separate artifact (`libexact_listen.dylib`,
// LLP 1133.005 D13), carried by a build whose grants listen or name a
// loopback callback.
#if os(macOS)
import AppKit
import Foundation

private typealias LoopbackDone = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void

/// The listener library's loopback entry points.
private final class LoopbackLibrary {
    typealias Open = @convention(c) (UnsafePointer<UInt8>?, Int, LoopbackDone?, UnsafeMutableRawPointer?, UnsafeMutablePointer<Int32>?) -> UnsafeMutableRawPointer?
    typealias Close = @convention(c) (UnsafeMutableRawPointer?) -> Void
    let open: Open, close: Close
    private init(open: Open, close: Close) { self.open = open; self.close = close }
    static let shared: LoopbackLibrary? = {
        let standard = Bundle.main.executableURL!.deletingLastPathComponent().path + "/libexact_listen.dylib"
        let path = ProcessInfo.processInfo.environment["EXACT_LISTEN_DYLIB"] ?? standard
        guard let library = dlopen(path, RTLD_NOW | RTLD_LOCAL),
              let open = dlsym(library, "exact_listen_loopback_open"), let close = dlsym(library, "exact_listen_loopback_close")
        else { return nil }
        return LoopbackLibrary(open: unsafeBitCast(open, to: Open.self), close: unsafeBitCast(close, to: Close.self))
    }()
}

/// One loopback sign-in: its receiver, until the callback or a cancel.
final class LoopbackSheet: AuthSheet {
    private var receiver: UnsafeMutableRawPointer?
    fileprivate var answered: ((String) -> Void)?

    fileprivate init() {}

    func cancel() {
        answered = nil
        if let receiver { LoopbackLibrary.shared?.close(receiver) }
        receiver = nil
    }

    /// Bind `loopback`'s port for `state`; a failure is the request's answer.
    fileprivate func bind(_ loopback: [String: Any], state: String) -> AuthFailure? {
        guard let library = LoopbackLibrary.shared else {
            return AuthFailure(status: 501, message: "this build carries no listener for a loopback callback: rebuild with its auth.callback grant")
        }
        let host = loopback["host"] as? String ?? "", port = (loopback["port"] as? NSNumber)?.intValue ?? 0
        let config = Array([host, String(port), loopback["path"] as? String ?? "", state, ExactEnv.appName].joined(separator: "\0").utf8)
        var error: Int32 = 0
        let context = Unmanaged.passUnretained(self).toOpaque()
        receiver = config.withUnsafeBufferPointer { library.open($0.baseAddress, $0.count, { ctx, bytes, count in
            guard let ctx, let bytes else { return }
            let url = String(decoding: UnsafeBufferPointer(start: bytes, count: count), as: UTF8.self)
            let sheet = Unmanaged<LoopbackSheet>.fromOpaque(ctx).takeUnretainedValue()
            DispatchQueue.main.async { sheet.answered?(url) }
        }, context, &error) }
        if receiver != nil { return nil }
        return error == 1 ? AuthFailure(status: 502, message: "port in use")
            : AuthFailure(status: 502, message: "the loopback callback's port could not be bound")
    }

    deinit { cancel() }
}

extension ExactSession {
    /// Open a loopback sign-in for `ticket` (D9): bind, then the default
    /// browser; the callback brings the window forward.
    func authLoopback(_ ticket: UInt64, url: URL, state: String, ephemeral: Bool, loopback: [String: Any], done: @escaping (Result<URL, AuthFailure>) -> Void) -> Result<AuthSheet, AuthFailure> {
        let sheet = LoopbackSheet()
        if let failure = sheet.bind(loopback, state: state) { return .failure(failure) }
        // Opening a URL carries no "no shared cookies" request (LLP 1069.006 D3).
        if ephemeral { log("auth \(ticket): ephemeral is not asked of the default browser") }
        sheet.answered = { [weak self, weak sheet] text in
            sheet?.cancel()
            self?.bringForward()
            guard let callback = URL(string: text) else { return done(.failure(AuthFailure(status: 502, message: "the callback URL is unreadable"))) }
            done(.success(callback))
        }
        guard NSWorkspace.shared.open(url) else {
            sheet.cancel()
            return .failure(AuthFailure(status: 502, message: "the default browser did not open"))
        }
        // Bounded at ten minutes, as the web's popup is (LLP 1069.006 D4).
        DispatchQueue.main.asyncAfter(deadline: .now() + 600) { [weak sheet] in
            guard let sheet, sheet.answered != nil else { return }
            sheet.cancel()
            done(.failure(AuthFailure(status: 499, message: "cancelled: no callback within ten minutes")))
        }
        return .success(sheet)
    }

    /// The window that asked comes forward (LLP 1133.005 D10), best effort
    /// under macOS 14's cooperative activation.
    private func bringForward() {
        guard let window = presenter.shareAnchor(nil).0?.window else { return }
        if NSApp.isHidden { NSApp.unhide(nil) }
        if window.isMiniaturized { window.deminiaturize(nil) }
        window.makeKeyAndOrderFront(nil)
        if #available(macOS 14.0, *) { NSApp.activate() } else { NSApp.activate(ignoringOtherApps: true) }
    }
}
#endif
