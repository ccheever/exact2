// @ref LLP 1115.002 D8.1; LLP 1075.003 §3.5 — covers on the Mac: how the host
// tells the kernel that a box is the platform's now, as the UIKit host does
// (`reportCovers`, NavigationBarIOS.swift). A box a native presentation
// stands for is covered whole and laid out as `display: none`; nothing
// authored changes, and a host that reports nothing lays out as authored.
//
// A presentation reports what it covers in the step in which it projected
// (the sidebar its tablist), and reports nothing when it stops. The kernel
// hears after the batch being applied, and what it hears is what stands
// then: a cover reported and taken back inside one batch is never told, so
// the kernel is never told of a projection that did not happen.
#if os(macOS)
import AppKit

final class HostCovers {
    private unowned let presenter: Presenter
    /// What each presentation covers, as it last reported, by its name.
    private var claims: [String: [UInt32: HostCover]] = [:]
    /// What the kernel was last told.
    private(set) var delivered: [UInt32: HostCover] = [:]
    private var queued = false
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// `presentation` covers exactly these boxes now; an empty report ends
    /// its covers. Told to the kernel after the batch being applied.
    func report(_ presentation: String, _ covers: [UInt32: HostCover]) {
        guard claims[presentation] ?? [:] != covers else { return }
        claims[presentation] = covers.isEmpty ? nil : covers
        guard !queued else { return }
        queued = true
        // Never inside a batch, whose remaining frames would overwrite the
        // covered ones (as on iOS).
        presenter.afterBatch { [weak self] in self?.deliver() }
    }

    private func deliver() {
        // A callback the runtime waits on can make no runtime call: deliver once it returns
        if Owner.shared.isServingMain {
            DispatchQueue.main.async { [weak self] in self?.deliver() }
            return
        }
        queued = false
        var wanted: [UInt32: HostCover] = [:]
        // A replaced box leaves layout, including any scrollbar gutter.
        // Other competing claims keep their stable order by name.
        for name in claims.keys.sorted() { wanted.merge(claims[name] ?? [:]) { first, next in next == .whole ? next : first } }
        wanted = wanted.filter { presenter.views[$0.key] != nil }
        var changes: [(UInt32, HostCover?)] = []
        for id in wanted.keys.sorted() where delivered[id] != wanted[id] { changes.append((id, wanted[id])) }
        for id in delivered.keys.sorted() where wanted[id] == nil && presenter.views[id] != nil { changes.append((id, nil)) }
        delivered = wanted
        if !changes.isEmpty { presenter.onCovers?(changes, []) }
    }

    /// A restart: the new runner knows no cover, and each presentation
    /// reports again when it projects again.
    func reset() { claims.removeAll(); delivered.removeAll(); queued = false }
}
#endif
