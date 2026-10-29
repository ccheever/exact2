import Foundation
#if os(iOS)
import UIKit

/// Native detail view over the same bounded metrics as the agent's health request.
final class FrameHealthInspectorController: UITableViewController {
    weak var health: FrameHealth?
    private let source = UISegmentedControl(items: ["App", "Scroll"])
    private var lines: [String] = []
    init(health: FrameHealth) { self.health = health; super.init(style: .insetGrouped) }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Frame Health"
        view.accessibilityIdentifier = "exact.host.frame-health-inspector"
        navigationItem.rightBarButtonItem = UIBarButtonItem(systemItem: .done, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) })
        source.selectedSegmentIndex = 0
        source.addAction(UIAction { [weak self] _ in self?.refresh() }, for: .valueChanged)
        let header = UIView(frame: CGRect(x: 0, y: 0, width: 320, height: 56))
        source.frame = header.bounds.insetBy(dx: 20, dy: 12)
        source.autoresizingMask = [.flexibleWidth]
        header.addSubview(source); tableView.tableHeaderView = header
        tableView.rowHeight = UITableView.automaticDimension
        tableView.estimatedRowHeight = 44
        refresh()
    }
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        health?.setInspectorVisible(true)
        health?.inspected = { [weak self] in self?.refresh() }
        health?.inspectionEnded = { [weak self] in self?.dismiss(animated: false) }
    }
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        health?.inspected = nil; health?.inspectionEnded = nil; health?.setInspectorVisible(false)
    }
    private func refresh() {
        guard let health else { return }
        source.setEnabled(health.nativeHealth != nil, forSegmentAt: 1)
        let chosen = source.selectedSegmentIndex == 1 ? health.nativeHealth ?? health : health
        lines = FrameHealthReadout.detail(chosen).components(separatedBy: "\n")
        tableView.reloadData()
    }
    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { lines.count }
    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "metric") ?? UITableViewCell(style: .default, reuseIdentifier: "metric")
        var content = cell.defaultContentConfiguration()
        content.text = lines[indexPath.row]
        content.textProperties.font = UIFontMetrics(forTextStyle: .body).scaledFont(for: .monospacedDigitSystemFont(ofSize: 15, weight: .regular))
        content.textProperties.numberOfLines = 0
        cell.contentConfiguration = content
        cell.selectionStyle = .none
        return cell
    }
}

extension DevMenu {
    static func showFrameHealthDetails() {
        guard let health = session?.frameHealth else { return }
        let navigation = UINavigationController(rootViewController: FrameHealthInspectorController(health: health))
        navigation.modalPresentationStyle = .pageSheet
        navigation.sheetPresentationController?.detents = [.large()]
        navigation.sheetPresentationController?.prefersGrabberVisible = true
        present(navigation)
    }
}
#elseif os(macOS)
import AppKit

final class FrameHealthInspector: NSViewController, NSWindowDelegate {
    weak var health: FrameHealth?
    private let output = NSTextView()
    private let source = NSSegmentedControl(labels: ["App", "Scroll"], trackingMode: .selectOne, target: nil, action: nil)
    init(health: FrameHealth) { self.health = health; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }
    override func loadView() {
        view = NSView(frame: CGRect(x: 0, y: 0, width: 560, height: 460))
        view.setAccessibilityIdentifier("exact.host.frame-health-inspector")
        let scroll = NSScrollView(); scroll.hasVerticalScroller = true
        output.isEditable = false; output.isSelectable = true
        output.font = .monospacedDigitSystemFont(ofSize: 12, weight: .regular)
        output.autoresizingMask = [.width]
        output.isVerticallyResizable = true
        output.textContainer?.widthTracksTextView = true
        scroll.documentView = output
        source.selectedSegment = 0; source.target = self; source.action = #selector(refresh)
        let done = NSButton(title: "Done", target: self, action: #selector(close))
        done.keyEquivalent = "\r"
        for child in [scroll, source, done] { child.translatesAutoresizingMaskIntoConstraints = false; view.addSubview(child) }
        NSLayoutConstraint.activate([
            source.topAnchor.constraint(equalTo: view.topAnchor, constant: 16), source.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 16),
            scroll.topAnchor.constraint(equalTo: source.bottomAnchor, constant: 12), scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor, constant: 16),
            scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16), scroll.bottomAnchor.constraint(equalTo: done.topAnchor, constant: -12),
            done.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -16), done.bottomAnchor.constraint(equalTo: view.bottomAnchor, constant: -16)])
        health?.inspected = { [weak self] in self?.refresh() }
        health?.inspectionEnded = { [weak self] in self?.close() }
        health?.setInspectorVisible(true)
        refresh()
    }
    @objc private func refresh() {
        guard let health else { return }
        source.setEnabled(health.nativeHealth != nil, forSegment: 1)
        output.string = FrameHealthReadout.detail(source.selectedSegment == 1 ? health.nativeHealth ?? health : health)
    }
    @objc private func close() {
        health?.inspected = nil; health?.inspectionEnded = nil; health?.setInspectorVisible(false)
        if let window = view.window {
            window.sheetParent?.endSheet(window)
            window.close()
        }
        health?.inspectorController = nil
    }
    func windowWillClose(_ notification: Notification) { health?.inspected = nil; health?.setInspectorVisible(false); health?.inspectorController = nil }
    static func show(health: FrameHealth, window: NSWindow?) {
        guard let window, health.inspectorController == nil else { return }
        let controller = FrameHealthInspector(health: health)
        let panel = NSPanel(contentViewController: controller)
        panel.title = "Frame Health"; panel.delegate = controller
        panel.styleMask.insert(.resizable)
        let owner = NSWindowController(window: panel)
        health.inspectorController = owner
        window.beginSheet(panel)
    }
}
#endif
