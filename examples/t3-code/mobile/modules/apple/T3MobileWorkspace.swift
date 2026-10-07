// @ref llp/1107.011-responsive-workspace.decision.md#composition-decision
// T3 Code 365aa87982: AdaptiveWorkspaceLayout, sidebar-navigation-shell,
// workspace-pane-animation. Owns only public tab-container child placement.
#if os(iOS)
import UIKit

final class T3MobileWorkspace {
    private weak var homeChrome: T3HomeChrome?
    private weak var inspectorChrome: T3MobileInspector?
    private weak var configurationView: T3WorkspaceConfigurationView?
    private weak var holder: T3WorkspaceContainer?
    private var configuration: T3WorkspaceConfiguration?
    private var nextOwner = 0
    private var currentOwner = 0
    private var active = true
    init(homeChrome: T3HomeChrome, inspector: T3MobileInspector) { self.homeChrome = homeChrome; inspectorChrome = inspector }

    func makeView(props: [String: String], events: ExactNativeEvents) throws -> ExactNativeInstance {
        guard active else { throw ExactNativeRefusal("The workspace session has ended.") }
        nextOwner += 1
        let instance = T3WorkspaceConfigurationView(owner: self, generation: nextOwner, events: events)
        try instance.setProps(props)
        return instance
    }
    func container(_ contents: ExactTabContents) -> UIViewController? {
        guard active, contents.isLive, contents.tabs.count == 3,
              Set(contents.tabs.map(\.name)) == Set(["sidebar-panel", "workspace-panel", "inspector-panel"]),
              let sidebar = contents.tabs.first(where: { $0.name == "sidebar-panel" }),
              let workspace = contents.tabs.first(where: { $0.name == "workspace-panel" }),
              let inspector = contents.tabs.first(where: { $0.name == "inspector-panel" }),
              contents.tabs.indices.contains(contents.selected), contents.tabs[contents.selected].name == "workspace-panel",
              Set(contents.tabs.map { ObjectIdentifier($0.controller) }).count == 3,
              contents.tabs.allSatisfy({ $0.controller.parent == nil }) else { return nil }
        holder?.retire()
        let container = T3WorkspaceContainer(contents: contents, sidebar: sidebar.controller, workspace: workspace.controller, inspector: inspector.controller,
            configuration: configuration, hideSidebar: { [weak self] key in self?.homeChrome?.hideSidebar(key: key) },
            showSidebar: { [weak self] key in self?.homeChrome?.showSidebar(key: key) },
            inspectorVisibility: { [weak self] key, visible in self?.inspectorChrome?.setVisible(key: key, visible: visible) },
            emit: { [weak self] event in self?.emit(event) })
        holder = container
        contents.onSelect = { [weak container] index in container?.selectionChanged(index) }
        return container
    }
    fileprivate func configure(_ next: T3WorkspaceConfiguration, generation: Int, view: T3WorkspaceConfigurationView) {
        guard active, generation >= currentOwner else { return }
        currentOwner = generation; configuration = next; configurationView = view
        holder?.configure(next)
    }
    fileprivate func release(generation: Int) {
        guard active, generation == currentOwner else { return }
        configuration = nil; configurationView = nil; holder?.configure(nil)
    }
    private func emit(_ event: [String: Any]) {
        guard active, let configuration, event["owner"] as? String == configuration.inspectorOwner else { return }
        if event["kind"] as? String == "inspector-closed" {
            guard !configuration.inspectorVisible, event["exitToken"] as? String == configuration.inspectorExitToken else { return }
        } else { guard configuration.inspectorVisible || event["phase"] as? String == "end" else { return } }
        configurationView?.emit(event)
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
    let inspectorRouteKey: String
    let inspectorOwner: String
    let inspectorExitToken: String
    let inspectorVisible: Bool
    let inspectorContentWidth: Double
    let inspectorResizing: Bool
    let dividerColor: String
    let dividerActiveColor: String
    let reducedMotion: Bool
    let appearance: String
    let background: String
    static func decode(_ props: [String: String]) throws -> Self {
        guard let json = props["workspace-configuration"], let bytes = json.data(using: .utf8),
              let value = try? JSONDecoder().decode(Self.self, from: bytes), !value.sidebarRouteKey.isEmpty,
              ["light", "dark"].contains(value.appearance),
              [value.viewportWidth, value.viewportHeight, value.sidebarContentWidth, value.sidebarTargetWidth, value.contentSettledWidth, value.inspectorTargetWidth, value.inspectorContentWidth].allSatisfy({ $0.isFinite && $0 >= 0 }),
              value.sidebarTargetWidth <= value.sidebarContentWidth,
              value.usesSplitView || (!value.sidebarVisible && value.sidebarTargetWidth == 0 && value.inspectorTargetWidth == 0),
              value.sidebarVisible == (value.sidebarTargetWidth > 0),
              value.inspectorRouteKey == "t3-workspace-inspector",
              value.inspectorVisible == (value.inspectorTargetWidth > 0),
              !value.inspectorVisible || (!value.inspectorOwner.isEmpty && value.inspectorContentWidth > 0) else {
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
        owner?.configure(try T3WorkspaceConfiguration.decode(props), generation: generation, view: self)
    }
    fileprivate func emit(_ event: [String: Any]) {
        guard active, let bytes = try? JSONSerialization.data(withJSONObject: event) else { return }
        events.change(String(decoding: bytes, as: UTF8.self))
    }
    override func destroy() {
        guard active else { return }; active = false
        owner?.release(generation: generation); owner = nil
    }
}

/// Empty space passes through; the three pane wrappers hold only public vended
/// navigation controllers. Their Exact route content remains framework-owned.
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
    private let inspector: UINavigationController
    private let sidebarClip = UIView()
    private let workspaceClip = UIView()
    private let inspectorClip = UIView()
    private let divider = T3WorkspaceDivider()
    private let hideSidebar: (String) -> Void
    private let showSidebar: (String) -> Void
    private let inspectorVisibility: (String, Bool) -> Void
    private let emit: ([String: Any]) -> Void
    private var configuration: T3WorkspaceConfiguration?
    private var pendingConfiguration: T3WorkspaceConfiguration?
    private var hasPendingConfiguration = true
    private var live = true
    private var renderedSidebar: CGFloat = 0
    private var renderedInspector: CGFloat = 0
    private var startSidebar: CGFloat = 0
    private var startInspector: CGFloat = 0
    private var renderedInspectorContent: CGFloat = 0
    private var startInspectorContent: CGFloat = 0
    private var contentStarted: CFTimeInterval = 0
    private var inspectorProgress: CGFloat = 0
    private var startProgress: CGFloat = 0
    private var progressStarted: CFTimeInterval = 0
    private var inspectorTransition: Bool?
    private var inspectorAppeared = false
    private var lastClosed: [String] = []
    private var drag: (owner: String, startWidth: Double, viewportWidth: Double, viewportHeight: Double)?
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

    private var inspectorWanted: Bool {
        live && contents.isLive && configuration?.inspectorVisible == true && (!hasPendingConfiguration ||
            pendingConfiguration?.inspectorVisible == true && pendingConfiguration?.inspectorOwner == configuration?.inspectorOwner)
    }
    init(contents: ExactTabContents, sidebar: UINavigationController, workspace: UINavigationController, inspector: UINavigationController,
         configuration: T3WorkspaceConfiguration?, hideSidebar: @escaping (String) -> Void, showSidebar: @escaping (String) -> Void, inspectorVisibility: @escaping (String, Bool) -> Void, emit: @escaping ([String: Any]) -> Void) {
        self.contents = contents; self.sidebar = sidebar; self.workspace = workspace; self.inspector = inspector
        pendingConfiguration = configuration; self.hideSidebar = hideSidebar; self.showSidebar = showSidebar; self.inspectorVisibility = inspectorVisibility; self.emit = emit
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
        sidebarClip.clipsToBounds = true; workspaceClip.clipsToBounds = true; inspectorClip.clipsToBounds = true
        view.addSubview(sidebarClip); view.addSubview(workspaceClip); view.addSubview(inspectorClip); view.addSubview(divider)
        divider.resize = { [weak self] phase, translation in self?.resize(phase: phase, translation: translation) }
        // Exact vends these controllers detached; only this public container owns placement.
        for (controller, clip) in [(sidebar, sidebarClip), (workspace, workspaceClip), (inspector, inspectorClip)] {
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
        layoutChildren()
    }
    override func viewSafeAreaInsetsDidChange() {
        super.viewSafeAreaInsetsDidChange()
        // UIKit derives each child's safe area from its actual wrapper frame.
        // Never overwrite Exact route covers or introduce duplicate inset padding.
        view.setNeedsLayout()
    }
    private func synchronizeSidebarChrome() {
        if let key = configuration?.inspectorRouteKey {
            inspectorVisibility(key, inspectorWanted && (parentTransition ?? parentVisible))
        }
        guard let key = configuration?.sidebarRouteKey else { return }
        if sidebarWanted && (parentTransition ?? parentVisible) { showSidebar(key) }
        else { hideSidebar(key) }
    }
    private func applyAppearance() {
        guard isViewLoaded else { return }
        overrideUserInterfaceStyle = configuration?.appearance == "dark" ? .dark : configuration == nil ? .unspecified : .light
        let background = T3WorkspaceColor.parse(configuration?.background)
        sidebarClip.backgroundColor = background; workspaceClip.backgroundColor = background; inspectorClip.backgroundColor = background
    }
    func configure(_ next: T3WorkspaceConfiguration?) {
        guard live, contents.isLive else { return }
        pendingConfiguration = next; hasPendingConfiguration = true
        if let drag, next?.inspectorOwner != drag.owner || next?.inspectorVisible != true || next?.viewportWidth != drag.viewportWidth || next?.viewportHeight != drag.viewportHeight { cancelResize() }
        if let key = configuration?.inspectorRouteKey, !inspectorWanted { inspectorVisibility(key, false) }
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
            // matched configuration clipped to real bounds and its clocks alive;
            // do not derive a second layout or start stale-viewport targets.
            layoutChildren(); return
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
        let contentChanged = previous?.inspectorContentWidth != next?.inspectorContentWidth
        let resizingChanged = previous?.inspectorResizing != next?.inspectorResizing
        let progressChanged = previous?.inspectorVisible != next?.inspectorVisible || contentChanged || resizingChanged
        // The source effect retargets width AND progress on width/resizing changes,
        // even when the destination is still zero (unsupported -> supported exit).
        let inspectorChanged = previous?.inspectorTargetWidth != next?.inspectorTargetWidth || progressChanged
        let changed = sidebarChanged || inspectorChanged || contentChanged || progressChanged
        let now = CACurrentMediaTime()
        if changed && displayLink != nil { step(now) }
        configuration = next
        synchronizeSidebarChrome()
        guard isViewLoaded else {
            renderedSidebar = CGFloat(next?.sidebarTargetWidth ?? 0); renderedInspector = CGFloat(next?.inspectorTargetWidth ?? 0); return
        }
        applyAppearance()
        let animated = changed && next?.reducedMotion != true
            && !UIAccessibility.isReduceMotionEnabled && view.window != nil && parentVisible
        updateSidebarAppearance(animated: animated)
        if previous?.inspectorOwner.isEmpty != false, next?.inspectorOwner.isEmpty == false,
           next?.inspectorVisible != true, inspectorProgress > 0, parentVisible, parentTransition == nil {
            // A supported column can remount its still-exiting renderer. Its
            // native child appears before resuming that disappearance transition.
            finishInspectorAppearance()
            inspector.beginAppearanceTransition(true, animated: false); inspector.endAppearanceTransition()
            inspectorAppeared = true
        }
        updateInspectorAppearance(animated: animated)
        if next?.inspectorOwner.isEmpty != false { finishInspectorAppearance() }
        if !changed && displayLink != nil {
            if next?.reducedMotion == true || UIAccessibility.isReduceMotionEnabled { finishMotion() }
            else { layoutChildren() }
            return
        }
        stopMotion()
        if animated {
            if sidebarChanged { startSidebar = renderedSidebar; sidebarStarted = now }
            if inspectorChanged { startInspector = renderedInspector; inspectorStarted = now }
            if contentChanged { startInspectorContent = renderedInspectorContent; contentStarted = now }
            if progressChanged { startProgress = inspectorProgress; progressStarted = now }
            if next?.inspectorVisible != true || next?.inspectorResizing == true || drag != nil {
                renderedInspectorContent = CGFloat(next?.inspectorContentWidth ?? 0)
                startInspectorContent = renderedInspectorContent; contentStarted = 0
            }
            if next?.usesSplitView != true {
                // Sidebar actually unmounts in compact mode. Inspector clocks
                // still run behind its unsupported, visually unmounted wrapper.
                renderedSidebar = 0; startSidebar = 0; sidebarStarted = 0
                finishSidebarAppearance()
            }
            if next?.inspectorResizing == true || drag != nil {
                renderedInspector = CGFloat(next?.inspectorTargetWidth ?? 0); startInspector = renderedInspector; inspectorStarted = 0
            }
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
        let contentFraction = min(1, max(0, (timestamp - contentStarted) / 0.260))
        let progressFraction = min(1, max(0, (timestamp - progressStarted) / 0.260))
        // Exact counterpart of Reanimated Easing.inOut(Easing.cubic), not UIKit's approximate easeInOut.
        func cubic(_ fraction: Double) -> CGFloat {
            CGFloat(fraction < 0.5 ? 4 * fraction * fraction * fraction : 1 - pow(-2 * fraction + 2, 3) / 2)
        }
        renderedSidebar = startSidebar + (CGFloat(configuration?.sidebarTargetWidth ?? 0) - startSidebar) * cubic(sidebarFraction)
        renderedInspector = startInspector + (CGFloat(configuration?.inspectorTargetWidth ?? 0) - startInspector) * cubic(inspectorFraction)
        renderedInspectorContent = startInspectorContent + (CGFloat(configuration?.inspectorContentWidth ?? 0) - startInspectorContent) * cubic(contentFraction)
        inspectorProgress = startProgress + ((configuration?.inspectorVisible == true ? 1 : 0) - startProgress) * cubic(progressFraction)
        layoutChildren()
        if progressFraction >= 1 { finishInspectorAppearance(); notifyClosed() }
        if sidebarFraction >= 1 { finishSidebarAppearance() }
        if sidebarFraction >= 1 && inspectorFraction >= 1 && contentFraction >= 1 && progressFraction >= 1 { finishMotion() }
    }
    private func layoutChildren() {
        guard isViewLoaded else { return }
        let bounds = view.bounds, split = configuration?.usesSplitView == true
        let leading = min(bounds.width, max(0, renderedSidebar))
        let inspectorMounted = configuration?.inspectorOwner.isEmpty == false
        let trailing = inspectorMounted ? min(max(0, bounds.width - leading), max(0, renderedInspector)) : 0
        sidebarClip.frame = CGRect(x: 0, y: 0, width: leading, height: bounds.height)
        sidebarClip.alpha = min(1, leading / 80)
        sidebarClip.isHidden = leading == 0
        sidebarClip.isUserInteractionEnabled = sidebarWanted
        sidebarClip.accessibilityElementsHidden = !sidebarWanted
        workspaceClip.frame = CGRect(x: leading, y: 0, width: max(0, bounds.width - leading - trailing), height: bounds.height)
        inspectorClip.frame = CGRect(x: bounds.width - trailing + (1 - inspectorProgress) * 24, y: 0, width: trailing, height: bounds.height)
        inspectorClip.alpha = inspectorProgress; inspectorClip.isHidden = trailing == 0
        inspectorClip.isUserInteractionEnabled = inspectorWanted; inspectorClip.accessibilityElementsHidden = !inspectorWanted
        divider.frame = CGRect(x: bounds.width - trailing - 22, y: 0, width: 44, height: bounds.height)
        divider.isHidden = !inspectorWanted; divider.isUserInteractionEnabled = inspectorWanted
        divider.accessibilityElementsHidden = !inspectorWanted
        divider.configure(width: configuration?.inspectorContentWidth ?? 0, dragging: drag != nil,
            normal: T3WorkspaceColor.parse(configuration?.dividerColor), active: T3WorkspaceColor.parse(configuration?.dividerActiveColor))
        // Workspace/sidebar settle once. Source inspector content width animates
        // independently while open; only its public navigation frame is assigned.
        // Contract still owns the inspector route/node's matching layout width.
        let side = CGRect(x: 0, y: 0, width: CGFloat(configuration?.sidebarContentWidth ?? 0), height: bounds.height)
        let work = CGRect(x: 0, y: 0, width: split ? CGFloat(configuration?.contentSettledWidth ?? 0) : bounds.width, height: bounds.height)
        UIView.performWithoutAnimation {
            if sidebar.view.frame != side { sidebar.view.frame = side }
            if workspace.view.frame != work { workspace.view.frame = work }
            let detail = CGRect(x: 0, y: 0, width: renderedInspectorContent, height: bounds.height)
            if inspector.view.frame != detail { inspector.view.frame = detail }
        }
    }
    private func stopMotion() { displayLink?.invalidate(); displayLink = nil }
    private func finishMotion() {
        stopMotion()
        renderedSidebar = CGFloat(configuration?.sidebarTargetWidth ?? 0)
        renderedInspector = CGFloat(configuration?.inspectorTargetWidth ?? 0)
        renderedInspectorContent = CGFloat(configuration?.inspectorContentWidth ?? 0)
        inspectorProgress = configuration?.inspectorVisible == true ? 1 : 0
        startInspectorContent = renderedInspectorContent; startProgress = inspectorProgress
        contentStarted = 0; progressStarted = 0
        startSidebar = renderedSidebar; startInspector = renderedInspector
        sidebarStarted = 0; inspectorStarted = 0
        layoutChildren(); finishSidebarAppearance(); finishInspectorAppearance(); notifyClosed()
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
    private func finishInspectorAppearance() {
        if let target = inspectorTransition { inspector.endAppearanceTransition(); inspectorAppeared = target; inspectorTransition = nil }
    }
    private func updateInspectorAppearance(animated: Bool) {
        guard parentVisible, parentTransition == nil else { return }
        if inspectorTransition == inspectorWanted { return }
        finishInspectorAppearance()
        guard inspectorAppeared != inspectorWanted else { return }
        inspector.beginAppearanceTransition(inspectorWanted, animated: animated); inspectorTransition = inspectorWanted
        if !animated { finishInspectorAppearance() }
    }
    private func notifyClosed() {
        guard live, let config = configuration, !config.inspectorOwner.isEmpty, !config.inspectorVisible, !config.inspectorExitToken.isEmpty,
              inspectorProgress == 0, renderedInspector == 0 else { return }
        let token = [config.inspectorOwner, config.inspectorExitToken]
        guard token != lastClosed else { return }; lastClosed = token
        emit(["kind": "inspector-closed", "owner": config.inspectorOwner, "exitToken": config.inspectorExitToken])
    }
    private func resize(phase: String, translation: Double) {
        guard live, inspectorWanted, let config = configuration, matchesViewport(config), !hasPendingConfiguration else { cancelResize(); return }
        if phase == "start" || phase == "step" {
            drag = (config.inspectorOwner, config.inspectorContentWidth, config.viewportWidth, config.viewportHeight)
        }
        guard let drag, drag.owner == config.inspectorOwner else { return }
        emit(["kind": "resize", "owner": drag.owner, "phase": phase, "startWidth": drag.startWidth, "translationX": translation])
        if phase == "end" || phase == "step" { self.drag = nil }
        layoutChildren()
    }
    private func cancelResize() {
        guard let old = drag else { return }; drag = nil
        emit(["kind": "resize", "owner": old.owner, "phase": "end", "startWidth": old.startWidth, "translationX": 0])
        divider.cancel()
    }
    private func finishWorkspaceAppearance() {
        if workspaceTransition { workspace.endAppearanceTransition(); workspaceTransition = false }
    }
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        finishSidebarAppearance(); finishInspectorAppearance(); finishWorkspaceAppearance(); parentTransition = true
        synchronizeSidebarChrome()
        workspace.beginAppearanceTransition(true, animated: animated); workspaceTransition = true
        if sidebarWanted { sidebar.beginAppearanceTransition(true, animated: animated); sidebarTransition = true }
        if inspectorWanted { inspector.beginAppearanceTransition(true, animated: animated); inspectorTransition = true }
    }
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        finishWorkspaceAppearance()
        finishSidebarAppearance(); finishInspectorAppearance(); parentTransition = nil; parentVisible = true
        synchronizeSidebarChrome()
        updateSidebarAppearance(animated: false); updateInspectorAppearance(animated: false)
    }
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        if let key = configuration?.sidebarRouteKey { hideSidebar(key) }
        cancelResize(); finishMotion(); finishWorkspaceAppearance(); parentTransition = false
        if let key = configuration?.inspectorRouteKey { inspectorVisibility(key, false) }
        workspace.beginAppearanceTransition(false, animated: animated); workspaceTransition = true
        if sidebarAppeared { sidebar.beginAppearanceTransition(false, animated: animated); sidebarTransition = false }
        if inspectorAppeared { inspector.beginAppearanceTransition(false, animated: animated); inspectorTransition = false }
    }
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        finishWorkspaceAppearance()
        finishSidebarAppearance(); finishInspectorAppearance(); parentTransition = nil; parentVisible = false
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
        cancelResize(); stopMotion(); finishSidebarAppearance(); finishInspectorAppearance(); finishWorkspaceAppearance()
        if let key = configuration?.inspectorRouteKey { inspectorVisibility(key, false) }
        divider.resize = nil
        if let key = configuration?.sidebarRouteKey { hideSidebar(key) }
        contents.onSelect = nil
        if let motionObserver { NotificationCenter.default.removeObserver(motionObserver); self.motionObserver = nil }
    }
    deinit { displayLink?.invalidate(); if let motionObserver { NotificationCenter.default.removeObserver(motionObserver) } }
}
/// The source divider occupies 44pt centered on its boundary. It owns input
/// recognition only; root constrains and persists width from the captured start.
private final class T3WorkspaceDivider: UIView {
    var resize: ((String, Double) -> Void)?
    private let line = UIView()
    private lazy var pan = T3WorkspacePan(target: self, action: #selector(panned))
    override init(frame: CGRect) {
        super.init(frame: frame); addSubview(line); addGestureRecognizer(pan)
        isAccessibilityElement = true; accessibilityLabel = "Resize detail pane"
        accessibilityTraits = .adjustable
        accessibilityCustomActions = [
            UIAccessibilityCustomAction(name: "Make pane wider", target: self, selector: #selector(widen)),
            UIAccessibilityCustomAction(name: "Make pane narrower", target: self, selector: #selector(narrow)),
        ]
    }
    required init?(coder: NSCoder) { nil }
    func configure(width: Double, dragging: Bool, normal: UIColor, active: UIColor) {
        accessibilityValue = "\(Int(width.rounded())) points wide"
        let thickness: CGFloat = dragging ? 2 : 1 / max(1, traitCollection.displayScale)
        line.frame = CGRect(x: bounds.midX - thickness / 2, y: 0, width: thickness, height: bounds.height)
        line.backgroundColor = dragging ? active : normal.withAlphaComponent(normal.cgColor.alpha * 0.7)
    }
    @objc private func panned() {
        switch pan.state {
        case .began: resize?("start", 0); resize?("update", pan.translationX)
        case .changed: resize?("update", pan.translationX)
        case .ended, .cancelled: resize?("end", pan.translationX)
        default: break
        }
    }
    func cancel() { pan.isEnabled = false; pan.isEnabled = true }
    override func accessibilityIncrement() { if !isHidden && isUserInteractionEnabled { resize?("step", -24) } }
    override func accessibilityDecrement() { if !isHidden && isUserInteractionEnabled { resize?("step", 24) } }
    @objc private func widen() -> Bool { accessibilityIncrement(); return true }
    @objc private func narrow() -> Bool { accessibilityDecrement(); return true }
}
private final class T3WorkspacePan: UIGestureRecognizer {
    private var origin: CGPoint?
    private(set) var translationX: Double = 0
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        guard origin == nil, touches.count == 1, let touch = touches.first else {
            state = state == .possible ? .failed : .cancelled; return
        }
        origin = touch.location(in: view?.window)
    }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let origin, let touch = touches.first else { return }
        let current = touch.location(in: view?.window)
        translationX = Double(current.x - origin.x)
        if state == .possible {
            if abs(current.y - origin.y) > 24 { state = .failed }
            else if abs(translationX) > 4 { state = .began }
        } else if state == .began || state == .changed { state = .changed }
    }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
        state = state == .possible ? .failed : .ended
    }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) { state = .cancelled }
    override func reset() { super.reset(); origin = nil; translationX = 0 }
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
