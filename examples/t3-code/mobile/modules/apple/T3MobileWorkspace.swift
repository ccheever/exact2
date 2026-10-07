// @ref llp/1106.011-responsive-workspace.decision.md#composition-decision
// T3 Code 365aa87982: AdaptiveWorkspaceLayout, sidebar-navigation-shell,
// workspace-pane-animation. Owns only public tab-container child placement.
#if os(iOS)
import UIKit

final class T3MobileWorkspace {
    private weak var homeChrome: T3HomeChrome?
    private weak var holder: T3WorkspaceContainer?
    private var configuration: T3WorkspaceConfiguration?
    private var nextOwner = 0
    private var currentOwner = 0
    private var active = true
    init(homeChrome: T3HomeChrome) { self.homeChrome = homeChrome }

    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard active else { throw ExactNativeRefusal("The workspace session has ended.") }
        nextOwner += 1
        let instance = T3WorkspaceConfigurationView(owner: self, generation: nextOwner, events: events)
        try instance.setProps(props)
        return instance
    }
    func container(_ contents: ExactTabContents) -> UIViewController? {
        guard active, contents.isLive, contents.tabs.count == 2,
              Set(contents.tabs.map(\.name)) == Set(["sidebar-panel", "workspace-panel"]),
              let sidebar = contents.tabs.first(where: { $0.name == "sidebar-panel" }),
              let workspace = contents.tabs.first(where: { $0.name == "workspace-panel" }),
              contents.tabs.indices.contains(contents.selected), contents.tabs[contents.selected].name == "workspace-panel",
              sidebar.controller !== workspace.controller,
              sidebar.controller.parent == nil, workspace.controller.parent == nil else { return nil }
        holder?.retire()
        let container = T3WorkspaceContainer(contents: contents, sidebar: sidebar.controller, workspace: workspace.controller,
            configuration: configuration, hideSidebar: { [weak self] key in self?.homeChrome?.hideSidebar(key: key) },
            showSidebar: { [weak self] key in self?.homeChrome?.showSidebar(key: key) })
        holder = container
        contents.onSelect = { [weak container] index in container?.selectionChanged(index) }
        return container
    }
    fileprivate func configure(_ next: T3WorkspaceConfiguration, generation: Int) {
        guard active, generation >= currentOwner else { return }
        currentOwner = generation; configuration = next
        holder?.configure(next)
    }
    fileprivate func release(generation: Int) {
        guard active, generation == currentOwner else { return }
        configuration = nil; holder?.configure(nil)
    }
    func destroy() {
        guard active else { return }; active = false
        holder?.retire(); holder = nil; configuration = nil
    }
}

private struct T3WorkspaceConfiguration: Decodable {
    let sidebarRouteKey: String
    let viewportWidth: Double
    let viewportHeight: Double
    let usesSplitView: Bool
    let sidebarVisible: Bool
    let sidebarContentWidth: Double
    let sidebarTargetWidth: Double
    let contentSettledWidth: Double
    let inspectorTargetWidth: Double
    let reducedMotion: Bool
    let appearance: String
    let background: String
    static func decode(_ props: [String: String]) throws -> Self {
        guard let json = props["workspace-configuration"], let bytes = json.data(using: .utf8),
              let value = try? JSONDecoder().decode(Self.self, from: bytes), !value.sidebarRouteKey.isEmpty,
              ["light", "dark"].contains(value.appearance),
              [value.viewportWidth, value.viewportHeight, value.sidebarContentWidth, value.sidebarTargetWidth, value.contentSettledWidth, value.inspectorTargetWidth].allSatisfy({ $0.isFinite && $0 >= 0 }),
              value.sidebarTargetWidth <= value.sidebarContentWidth,
              value.usesSplitView || (!value.sidebarVisible && value.sidebarTargetWidth == 0 && value.inspectorTargetWidth == 0),
              value.sidebarVisible == (value.sidebarTargetWidth > 0) else {
            throw ExactNativeRefusal("Workspace layout requires matching finite panel geometry, route key and appearance.")
        }
        return value
    }
}
private final class T3WorkspaceConfigurationView: ExactNativeInstance {
    private weak var owner: T3MobileWorkspace?
    private let generation: Int
    private let root = UIView()
    private var active = true
    override var view: UIView { root }
    init(owner: T3MobileWorkspace, generation: Int, events: ExactNativeEvents) {
        self.owner = owner; self.generation = generation
        super.init(events: events)
        root.isUserInteractionEnabled = false; root.backgroundColor = .clear
        root.accessibilityElementsHidden = true
    }
    override func setProps(_ props: [String: String]) throws {
        guard active else { return }
        owner?.configure(try T3WorkspaceConfiguration.decode(props), generation: generation)
    }
    override func destroy() {
        guard active else { return }; active = false
        owner?.release(generation: generation); owner = nil
    }
}

/// Empty trailing space stays transparent and passes hits through to any real
/// authored inspector. This holder never manufactures or reparents inspector content.
private final class T3WorkspaceSurface: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        let hit = super.hitTest(point, with: event)
        return hit === self ? nil : hit
    }
}
private final class T3WorkspaceTick: NSObject {
    weak var owner: T3WorkspaceContainer?
    @objc func step(_ link: CADisplayLink) { owner?.step(link.timestamp) }
}
private final class T3WorkspaceContainer: UIViewController {
    private let contents: ExactTabContents
    private let sidebar: UINavigationController
    private let workspace: UINavigationController
    private let sidebarClip = UIView()
    private let workspaceClip = UIView()
    private let hideSidebar: (String) -> Void
    private let showSidebar: (String) -> Void
    private var configuration: T3WorkspaceConfiguration?
    private var pendingConfiguration: T3WorkspaceConfiguration?
    private var hasPendingConfiguration = true
    private var live = true
    private var renderedSidebar: CGFloat = 0
    private var renderedInspector: CGFloat = 0
    private var startSidebar: CGFloat = 0
    private var startInspector: CGFloat = 0
    private var sidebarStarted: CFTimeInterval = 0
    private var inspectorStarted: CFTimeInterval = 0
    private var displayLink: CADisplayLink?
    private var motionObserver: NSObjectProtocol?
    private var parentVisible = false
    private var parentTransition: Bool?
    private var sidebarTransition: Bool?
    private var sidebarAppeared = false
    private var workspaceTransition = false
    private var sidebarWanted: Bool {
        live && contents.isLive && configuration?.sidebarVisible == true && (!hasPendingConfiguration ||
            pendingConfiguration?.sidebarVisible == true && pendingConfiguration?.sidebarRouteKey == configuration?.sidebarRouteKey)
    }

    init(contents: ExactTabContents, sidebar: UINavigationController, workspace: UINavigationController,
         configuration: T3WorkspaceConfiguration?, hideSidebar: @escaping (String) -> Void, showSidebar: @escaping (String) -> Void) {
        self.contents = contents; self.sidebar = sidebar; self.workspace = workspace
        pendingConfiguration = configuration; self.hideSidebar = hideSidebar; self.showSidebar = showSidebar
        super.init(nibName: nil, bundle: nil)
    }
    required init?(coder: NSCoder) { nil }
    override var shouldAutomaticallyForwardAppearanceMethods: Bool { false }
    override var childForStatusBarStyle: UIViewController? { workspace }
    override var childForStatusBarHidden: UIViewController? { workspace }
    override var childForHomeIndicatorAutoHidden: UIViewController? { workspace }
    override var childForScreenEdgesDeferringSystemGestures: UIViewController? { workspace }
    override var supportedInterfaceOrientations: UIInterfaceOrientationMask { workspace.supportedInterfaceOrientations }
    override var preferredInterfaceOrientationForPresentation: UIInterfaceOrientation { workspace.preferredInterfaceOrientationForPresentation }

    override func loadView() {
        view = T3WorkspaceSurface(); view.backgroundColor = .clear
        sidebarClip.clipsToBounds = true; workspaceClip.clipsToBounds = true
        view.addSubview(sidebarClip); view.addSubview(workspaceClip)
        // Exact vends these controllers detached; only this public container owns placement.
        for (controller, clip) in [(sidebar, sidebarClip), (workspace, workspaceClip)] {
            addChild(controller); clip.addSubview(controller.view); controller.didMove(toParent: self)
        }
        if let key = pendingConfiguration?.sidebarRouteKey { hideSidebar(key) }
        applyAppearance(); synchronizeSidebarChrome()
        motionObserver = NotificationCenter.default.addObserver(forName: UIAccessibility.reduceMotionStatusDidChangeNotification,
            object: nil, queue: .main) { [weak self] _ in
                guard let self, UIAccessibility.isReduceMotionEnabled else { return }
                self.finishMotion()
            }
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        guard live, contents.isLive else { retire(); return }
        applyPendingIfMatching()
        if let configuration, !matchesViewport(configuration) { finishMotion() }
        layoutChildren()
    }
    override func viewSafeAreaInsetsDidChange() {
        super.viewSafeAreaInsetsDidChange()
        // UIKit derives each child's safe area from its actual wrapper frame.
        // Never overwrite Exact route covers or introduce duplicate inset padding.
        view.setNeedsLayout()
    }
    private func synchronizeSidebarChrome() {
        guard let key = configuration?.sidebarRouteKey else { return }
        if sidebarWanted && (parentTransition ?? parentVisible) { showSidebar(key) }
        else { hideSidebar(key) }
    }
    private func applyAppearance() {
        guard isViewLoaded else { return }
        overrideUserInterfaceStyle = configuration?.appearance == "dark" ? .dark : configuration == nil ? .unspecified : .light
        let background = T3WorkspaceColor.parse(configuration?.background)
        sidebarClip.backgroundColor = background; workspaceClip.backgroundColor = background
    }
    func configure(_ next: T3WorkspaceConfiguration?) {
        guard live, contents.isLive else { return }
        pendingConfiguration = next; hasPendingConfiguration = true
        if let key = configuration?.sidebarRouteKey, !sidebarWanted { hideSidebar(key) }
        guard isViewLoaded else { return }
        applyPendingIfMatching()
    }
    private func matchesViewport(_ value: T3WorkspaceConfiguration) -> Bool {
        abs(view.bounds.width - value.viewportWidth) <= 0.5 && abs(view.bounds.height - value.viewportHeight) <= 0.5
    }
    private func applyPendingIfMatching() {
        guard hasPendingConfiguration else { return }
        if let pendingConfiguration, !matchesViewport(pendingConfiguration) {
            // Root producers may settle after UIKit's new bounds. Keep the last
            // matched child widths clipped to real bounds; do not derive a second
            // adaptive layout or restart animation against a stale viewport.
            finishMotion(); return
        }
        let next = pendingConfiguration
        hasPendingConfiguration = false; pendingConfiguration = nil
        applyConfiguration(next)
    }
    private func applyConfiguration(_ next: T3WorkspaceConfiguration?) {
        let previous = configuration
        if let previous, previous.sidebarVisible && (next?.sidebarVisible != true || next?.sidebarRouteKey != previous.sidebarRouteKey) {
            hideSidebar(previous.sidebarRouteKey)
        }
        let sidebarChanged = previous?.sidebarTargetWidth != next?.sidebarTargetWidth
        let inspectorChanged = previous?.inspectorTargetWidth != next?.inspectorTargetWidth
        let changed = sidebarChanged || inspectorChanged
        let now = CACurrentMediaTime()
        if changed && displayLink != nil { step(now) }
        configuration = next
        synchronizeSidebarChrome()
        guard isViewLoaded else {
            renderedSidebar = CGFloat(next?.sidebarTargetWidth ?? 0); renderedInspector = CGFloat(next?.inspectorTargetWidth ?? 0); return
        }
        applyAppearance()
        let animated = changed && next?.usesSplitView == true && next?.reducedMotion != true
            && !UIAccessibility.isReduceMotionEnabled && view.window != nil && parentVisible
        updateSidebarAppearance(animated: animated)
        if !changed && displayLink != nil {
            if next?.reducedMotion == true || UIAccessibility.isReduceMotionEnabled { finishMotion() }
            else { layoutChildren() }
            return
        }
        stopMotion()
        if animated {
            if sidebarChanged { startSidebar = renderedSidebar; sidebarStarted = now }
            if inspectorChanged { startInspector = renderedInspector; inspectorStarted = now }
            let tick = T3WorkspaceTick(); tick.owner = self
            let link = CADisplayLink(target: tick, selector: #selector(T3WorkspaceTick.step(_:)))
            displayLink = link; link.add(to: .main, forMode: .common)
            layoutChildren()
        } else { finishMotion() }
    }
    fileprivate func step(_ timestamp: CFTimeInterval) {
        guard live, contents.isLive else { retire(); return }
        let sidebarFraction = min(1, max(0, (timestamp - sidebarStarted) / 0.260))
        let inspectorFraction = min(1, max(0, (timestamp - inspectorStarted) / 0.260))
        // Exact counterpart of Reanimated Easing.inOut(Easing.cubic), not UIKit's approximate easeInOut.
        func cubic(_ fraction: Double) -> CGFloat {
            CGFloat(fraction < 0.5 ? 4 * fraction * fraction * fraction : 1 - pow(-2 * fraction + 2, 3) / 2)
        }
        renderedSidebar = startSidebar + (CGFloat(configuration?.sidebarTargetWidth ?? 0) - startSidebar) * cubic(sidebarFraction)
        renderedInspector = startInspector + (CGFloat(configuration?.inspectorTargetWidth ?? 0) - startInspector) * cubic(inspectorFraction)
        layoutChildren()
        if sidebarFraction >= 1 { finishSidebarAppearance() }
        if sidebarFraction >= 1 && inspectorFraction >= 1 { finishMotion() }
    }
    private func layoutChildren() {
        guard isViewLoaded else { return }
        let bounds = view.bounds, split = configuration?.usesSplitView == true
        let leading = min(bounds.width, max(0, renderedSidebar))
        let trailing = min(max(0, bounds.width - leading), max(0, renderedInspector))
        sidebarClip.frame = CGRect(x: 0, y: 0, width: leading, height: bounds.height)
        sidebarClip.alpha = min(1, leading / 80)
        sidebarClip.isHidden = leading == 0
        sidebarClip.isUserInteractionEnabled = sidebarWanted
        sidebarClip.accessibilityElementsHidden = !sidebarWanted
        workspaceClip.frame = CGRect(x: leading, y: 0, width: max(0, bounds.width - leading - trailing), height: bounds.height)
        // Only the clipping wrappers change every frame. Each native navigator
        // receives its settled width once per configuration/viewport change.
        let side = CGRect(x: 0, y: 0, width: CGFloat(configuration?.sidebarContentWidth ?? 0), height: bounds.height)
        let work = CGRect(x: 0, y: 0, width: split ? CGFloat(configuration?.contentSettledWidth ?? 0) : bounds.width, height: bounds.height)
        UIView.performWithoutAnimation {
            if sidebar.view.frame != side { sidebar.view.frame = side }
            if workspace.view.frame != work { workspace.view.frame = work }
        }
    }
    private func stopMotion() { displayLink?.invalidate(); displayLink = nil }
    private func finishMotion() {
        stopMotion()
        renderedSidebar = CGFloat(configuration?.sidebarTargetWidth ?? 0)
        renderedInspector = CGFloat(configuration?.inspectorTargetWidth ?? 0)
        startSidebar = renderedSidebar; startInspector = renderedInspector
        sidebarStarted = 0; inspectorStarted = 0
        layoutChildren(); finishSidebarAppearance()
    }
    private func finishSidebarAppearance() {
        if let target = sidebarTransition { sidebar.endAppearanceTransition(); sidebarAppeared = target; sidebarTransition = nil }
    }
    private func updateSidebarAppearance(animated: Bool) {
        guard parentVisible, parentTransition == nil else { return }
        if sidebarTransition == sidebarWanted { return }
        finishSidebarAppearance()
        guard sidebarAppeared != sidebarWanted else { return }
        sidebar.beginAppearanceTransition(sidebarWanted, animated: animated); sidebarTransition = sidebarWanted
        if !animated { finishSidebarAppearance() }
    }
    private func finishWorkspaceAppearance() {
        if workspaceTransition { workspace.endAppearanceTransition(); workspaceTransition = false }
    }
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        finishSidebarAppearance(); finishWorkspaceAppearance(); parentTransition = true
        synchronizeSidebarChrome()
        workspace.beginAppearanceTransition(true, animated: animated); workspaceTransition = true
        if sidebarWanted { sidebar.beginAppearanceTransition(true, animated: animated); sidebarTransition = true }
    }
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        finishWorkspaceAppearance()
        finishSidebarAppearance(); parentTransition = nil; parentVisible = true
        synchronizeSidebarChrome()
        updateSidebarAppearance(animated: false)
    }
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        if let key = configuration?.sidebarRouteKey { hideSidebar(key) }
        finishMotion(); finishWorkspaceAppearance(); parentTransition = false
        workspace.beginAppearanceTransition(false, animated: animated); workspaceTransition = true
        if sidebarAppeared { sidebar.beginAppearanceTransition(false, animated: animated); sidebarTransition = false }
    }
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        finishWorkspaceAppearance()
        finishSidebarAppearance(); parentTransition = nil; parentVisible = false
        synchronizeSidebarChrome()
    }
    func selectionChanged(_ index: Int) {
        guard live, contents.isLive, contents.tabs.indices.contains(index) else { return }
        // The authored router always selects workspace-panel. Never press/select
        // a tab here or become a second selected-state owner.
        if contents.tabs[index].name != "workspace-panel" { return }
        setNeedsStatusBarAppearanceUpdate(); setNeedsUpdateOfHomeIndicatorAutoHidden()
    }
    override func willMove(toParent parent: UIViewController?) {
        if parent == nil && self.parent != nil { retire() }
        super.willMove(toParent: parent)
    }
    func retire() {
        guard live else { return }; live = false
        if let key = pendingConfiguration?.sidebarRouteKey { hideSidebar(key) }
        pendingConfiguration = nil; hasPendingConfiguration = false
        stopMotion(); finishSidebarAppearance(); finishWorkspaceAppearance()
        if let key = configuration?.sidebarRouteKey { hideSidebar(key) }
        contents.onSelect = nil
        if let motionObserver { NotificationCenter.default.removeObserver(motionObserver); self.motionObserver = nil }
    }
    deinit { displayLink?.invalidate(); if let motionObserver { NotificationCenter.default.removeObserver(motionObserver) } }
}
private enum T3WorkspaceColor {
    static func parse(_ value: String?) -> UIColor {
        let text = (value ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        if text.hasPrefix("#"), let hex = UInt64(text.dropFirst(), radix: 16) {
            let rgb = text.count == 9 ? hex >> 8 : hex
            return UIColor(red: CGFloat((rgb >> 16) & 255) / 255, green: CGFloat((rgb >> 8) & 255) / 255,
                blue: CGFloat(rgb & 255) / 255, alpha: text.count == 9 ? CGFloat(hex & 255) / 255 : 1)
        }
        if text.hasPrefix("rgb"), let start = text.firstIndex(of: "("), let end = text.lastIndex(of: ")") {
            let parts = text[text.index(after: start)..<end].split(separator: ",").compactMap { Double($0.trimmingCharacters(in: .whitespaces)) }
            if parts.count >= 3 { return UIColor(red: parts[0] / 255, green: parts[1] / 255, blue: parts[2] / 255, alpha: parts.count > 3 ? parts[3] : 1) }
        }
        return .systemBackground
    }
}
#endif
