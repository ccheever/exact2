#if os(iOS)
import UIKit

/// UIKit owns sheet width, scrolling, Dynamic Type and row interaction.
final class DevMenuController: UITableViewController {
    struct Command {
        let title: String
        let symbol: String
        let action: (() -> Void)?
        /// Why the command is unavailable and how to make it available: the row is dimmed, and a tap explains
        var unavailable: (title: String, message: String)? = nil
        /// Whether what the command shows is on screen: the row has a checkmark, and a tap hides it
        var checked = false
    }
    private(set) var groups: [[Command]] = []

    init() { super.init(style: .insetGrouped) }
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Exact Developer"
        navigationItem.rightBarButtonItem = UIBarButtonItem(systemItem: .close, primaryAction: UIAction { [weak self] _ in
            self?.dismiss(animated: true)
        })
        tableView.rowHeight = UITableView.automaticDimension
        tableView.estimatedRowHeight = 56
        tableView.cellLayoutMarginsFollowReadableWidth = true
        tableView.accessibilityIdentifier = "exact.host.dev-menu"
        rebuild()
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        rebuild()
    }

    private func rebuild() {
        var project = [
            Command(title: "Reload", symbol: "arrow.clockwise", action: DevMenu.reload),
            Command(title: "Open Project…", symbol: "folder", action: DevMenu.openProject)]
        #if EXACT_WIDGET_GALLERY
        if WidgetGallery.available { project.append(Command(title: "Widgets…", symbol: "square.grid.2x2", action: WidgetGallery.show)) }
        #endif
        var diagnostics = [Self.frameHealth(DevMenu.session?.frameHealth, container: DevMenu.frameHealthContainer != nil)]
        if FrameSampler.measured { diagnostics.append(Command(title: "Save Trace", symbol: "square.and.arrow.up", action: DevMenu.saveTrace)) }
        groups = [project, diagnostics, [Command(title: "Build Information", symbol: "info.circle", action: {})]]
        tableView.reloadData()
    }

    /// The one frame-health command: checked while the card shows, dimmed where the card cannot read, with a tap that says why
    static func frameHealth(_ health: FrameHealth?, container: Bool) -> Command {
        let visible = health?.hudVisible ?? false
        let unavailable = !container ? (title: "Frame Health Is Unavailable",
            message: "This app's host gives the developer menu no place for the frame-health card. A host that embeds Exact passes a container view as frameHealthContainer to DevMenu.install.")
            : health?.unavailableExplanation
        return Command(title: unavailable == nil ? "Frame Health" : "Frame Health Disabled", symbol: "waveform.path.ecg",
            action: { DevMenu.frameHealthOverlay()?.setVisible(!visible) }, unavailable: unavailable, checked: unavailable == nil && visible)
    }

    override func numberOfSections(in tableView: UITableView) -> Int { groups.count }
    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { groups[section].count }
    override func tableView(_ tableView: UITableView, titleForHeaderInSection section: Int) -> String? {
        ["Project", "Diagnostics", "About"][section]
    }

    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "command") ?? UITableViewCell(style: .default, reuseIdentifier: "command")
        let command = groups[indexPath.section][indexPath.row]
        var content = cell.defaultContentConfiguration()
        content.text = command.title
        content.image = UIImage(systemName: command.symbol)
        content.textProperties.numberOfLines = 0
        let enabled = command.action != nil && command.unavailable == nil
        content.textProperties.color = enabled ? .label : .secondaryLabel
        cell.contentConfiguration = content
        cell.isUserInteractionEnabled = command.action != nil
        cell.accessibilityTraits = enabled ? (command.checked ? [.button, .selected] : [.button]) : [.button, .notEnabled]
        cell.accessibilityHint = command.unavailable == nil ? nil : "Explains why it is unavailable."
        cell.accessoryType = indexPath.section == 2 ? .disclosureIndicator : command.checked ? .checkmark : .none
        cell.accessibilityIdentifier = "exact.host.dev-menu.\(command.symbol)"
        return cell
    }

    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: true)
        if indexPath.section == 2 {
            navigationController?.pushViewController(BuildInformationController(), animated: true)
        } else if let unavailable = groups[indexPath.section][indexPath.row].unavailable {
            let alert = UIAlertController(title: unavailable.title, message: unavailable.message, preferredStyle: .alert)
            alert.addAction(UIAlertAction(title: "OK", style: .default))
            present(alert, animated: true)
        } else {
            guard let action = groups[indexPath.section][indexPath.row].action else { return }
            dismiss(animated: true, completion: action)
        }
    }

    static func navigation() -> UINavigationController {
        let navigation = UINavigationController(rootViewController: DevMenuController())
        navigation.modalPresentationStyle = UIDevice.current.userInterfaceIdiom == .pad ? .formSheet : .pageSheet
        navigation.preferredContentSize = CGSize(width: 560, height: 640)
        if let sheet = navigation.sheetPresentationController {
            // Opens at full height, and a drag rests it at half to show the app beneath
            sheet.detents = [.medium(), .large()]
            sheet.selectedDetentIdentifier = .large
            sheet.prefersGrabberVisible = true
            sheet.prefersEdgeAttachedInCompactHeight = true
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

    // Keeps the text clear of the side insets too, such as the iPhone Duo's strip of bar items: a scroll view adjusts
    // its content only for the insets on the axis it scrolls
    override func viewSafeAreaInsetsDidChange() {
        super.viewSafeAreaInsetsDidChange()
        guard let text = view as? UITextView else { return }
        let safe = view.safeAreaInsets
        text.textContainerInset = UIEdgeInsets(top: 20, left: 20 + safe.left, bottom: 20, right: 20 + safe.right)
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
