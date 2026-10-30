#if os(iOS)
import UIKit

/// UIKit owns sheet width, scrolling, Dynamic Type and row interaction.
final class DevMenuController: UITableViewController {
    struct Command {
        let title: String
        let symbol: String
        let detail: String?
        let action: (() -> Void)?
    }
    private(set) var groups: [[Command]] = []

    init() { super.init(style: .insetGrouped) }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Exact Developer"
        navigationItem.rightBarButtonItem = UIBarButtonItem(systemItem: .done, primaryAction: UIAction { [weak self] _ in
            self?.dismiss(animated: true)
        })
        tableView.rowHeight = UITableView.automaticDimension
        tableView.estimatedRowHeight = 56
        tableView.cellLayoutMarginsFollowReadableWidth = true
        tableView.accessibilityIdentifier = "exact.host.dev-menu"
        rebuild()
        if traitCollection.preferredContentSizeCategory.isAccessibilityCategory {
            navigationController?.sheetPresentationController?.selectedDetentIdentifier = .large
        }
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        rebuild()
        if traitCollection.preferredContentSizeCategory.isAccessibilityCategory {
            navigationController?.sheetPresentationController?.selectedDetentIdentifier = .large
        }
    }

    private func rebuild() {
        var project = [
            Command(title: "Reload", symbol: "arrow.clockwise", detail: nil, action: DevMenu.reload),
            Command(title: "Open Project…", symbol: "folder", detail: nil, action: DevMenu.openProject)]
        #if EXACT_WIDGET_GALLERY
        if WidgetGallery.available { project.append(Command(title: "Widgets…", symbol: "square.grid.2x2", detail: nil, action: WidgetGallery.show)) }
        #endif
        var diagnostics: [Command] = []
        let visible = DevMenu.session?.frameHealth.hudVisible ?? false
        let toggle: (() -> Void)? = DevMenu.frameHealthContainer == nil ? nil : {
            DevMenu.frameHealthOverlay()?.setVisible(!visible)
        }
        diagnostics.append(Command(title: visible ? "Hide Frame Health" : "Show Frame Health", symbol: "waveform.path.ecg",
            detail: toggle == nil ? "This host has no frame-health overlay container" : "Draggable callback timing readout", action: toggle))
        diagnostics.append(Command(title: "Frame Health Details…", symbol: "chart.xyaxis.line", detail: "Timing distributions and coverage", action: DevMenu.showFrameHealthDetails))
        diagnostics.append(Command(title: "Share Frame Health…", symbol: "doc.text", detail: "Recent hitches · at most 4 KB", action: DevMenu.shareFrameHealth))
        if FrameSampler.measured { diagnostics.append(Command(title: "Save Trace", symbol: "square.and.arrow.up", detail: nil, action: DevMenu.saveTrace)) }
        groups = [project, diagnostics, [Command(title: "Build Information", symbol: "info.circle", detail: nil, action: {})]]
        tableView.reloadData()
    }

    override func numberOfSections(in tableView: UITableView) -> Int { groups.count }
    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { groups[section].count }
    override func tableView(_ tableView: UITableView, titleForHeaderInSection section: Int) -> String? {
        ["Project", "Diagnostics", "About"][section]
    }
    override func tableView(_ tableView: UITableView, titleForFooterInSection section: Int) -> String? {
        section == 1 ? "Frame health measures Exact callbacks and applies. Native-scroll deadlines are experimental. Presentation is unobserved." : nil
    }

    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "command") ?? UITableViewCell(style: .default, reuseIdentifier: "command")
        let command = groups[indexPath.section][indexPath.row]
        var content = cell.defaultContentConfiguration()
        content.text = command.title
        content.secondaryText = command.detail
        content.image = UIImage(systemName: command.symbol)
        content.textProperties.numberOfLines = 0
        content.textProperties.color = command.action == nil ? .secondaryLabel : .label
        content.secondaryTextProperties.numberOfLines = 0
        cell.contentConfiguration = content
        cell.isUserInteractionEnabled = command.action != nil
        cell.accessibilityTraits = command.action == nil ? [.button, .notEnabled] : [.button]
        cell.accessoryType = indexPath.section == 2 ? .disclosureIndicator : .none
        cell.accessibilityIdentifier = "exact.host.dev-menu.\(command.symbol)"
        return cell
    }

    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: true)
        if indexPath.section == 2 {
            navigationController?.pushViewController(BuildInformationController(), animated: true)
        } else {
            guard let action = groups[indexPath.section][indexPath.row].action else { return }
            dismiss(animated: true, completion: action)
        }
    }

    static func navigation(contentSizeCategory: UIContentSizeCategory = .large) -> UINavigationController {
        let navigation = UINavigationController(rootViewController: DevMenuController())
        navigation.modalPresentationStyle = UIDevice.current.userInterfaceIdiom == .pad ? .formSheet : .pageSheet
        navigation.preferredContentSize = CGSize(width: 560, height: 640)
        if let sheet = navigation.sheetPresentationController {
            sheet.detents = [.medium(), .large()]
            sheet.selectedDetentIdentifier = contentSizeCategory.isAccessibilityCategory ? .large : .medium
            sheet.prefersGrabberVisible = true
            sheet.prefersEdgeAttachedInCompactHeight = true
            sheet.widthFollowsPreferredContentSizeWhenEdgeAttached = false
        }
        return navigation
    }
}

private final class BuildInformationController: UIViewController {
    override func loadView() {
        let text = UITextView()
        text.isEditable = false
        text.alwaysBounceVertical = true
        text.font = .preferredFont(forTextStyle: .body)
        text.adjustsFontForContentSizeCategory = true
        text.backgroundColor = .systemGroupedBackground
        text.textColor = .label
        text.textContainerInset = UIEdgeInsets(top: 20, left: 20, bottom: 20, right: 20)
        text.text = DevMenu.info()
        view = text
    }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Build Information"
        navigationItem.rightBarButtonItem = UIBarButtonItem(title: "Copy", primaryAction: UIAction { _ in
            UIPasteboard.general.string = DevMenu.info()
        })
    }
}
#endif
