#if os(iOS)
import UIKit

final class GallerySplit: UISplitViewController, UISplitViewControllerDelegate {
    private let contents: ExactTabContents
    private let stacks: [UINavigationController]
    private let sidebar: GallerySidebar
    private var selectedFromSidebar = false

    init(_ contents: ExactTabContents) {
        self.contents = contents
        stacks = contents.tabs.map { $0.controller }
        sidebar = GallerySidebar(contents)
        super.init(style: .doubleColumn)
        delegate = self
        preferredDisplayMode = .oneBesideSecondary
        setViewController(UINavigationController(rootViewController: sidebar), for: .primary)
        setViewController(stacks[contents.selected], for: .secondary)
        sidebar.choose = { [weak self] index in self?.choose(index) }
        contents.onSelect = { [weak self] index in
            guard let self else { return }
            selectedFromSidebar = true
            setViewController(stacks[index], for: .secondary)
            sidebar.refreshSelection()
            show(.secondary)
        }
    }

    required init?(coder: NSCoder) { nil }

    private func choose(_ index: Int) {
        selectedFromSidebar = true
        // Reveal the current section too: its router selection does not change
        if contents.selected == index {
            contents.select(index)
            show(.secondary)
        } else {
            contents.select(index)
        }
    }

    func splitViewController(_ svc: UISplitViewController,
                             topColumnForCollapsingToProposedTopColumn proposedTopColumn: UISplitViewController.Column) -> UISplitViewController.Column {
        selectedFromSidebar ? .secondary : .primary
    }
}

final class GallerySidebar: UITableViewController {
    private let contents: ExactTabContents
    private let groups = [("Text", [0, 1]), ("Color & images", [2, 3, 4, 5]), ("UI & media", [6, 7, 8])]
    var choose: ((Int) -> Void)?

    init(_ contents: ExactTabContents) {
        self.contents = contents
        super.init(style: .insetGrouped)
        title = "UI Gallery"
        navigationItem.largeTitleDisplayMode = .always
    }

    required init?(coder: NSCoder) { nil }

    override func viewDidLoad() {
        super.viewDidLoad()
        navigationController?.navigationBar.prefersLargeTitles = true
        tableView.accessibilityIdentifier = "gallery-sidebar"
        tableView.register(UITableViewCell.self, forCellReuseIdentifier: "section")
        clearsSelectionOnViewWillAppear = false
        refreshSelection()
    }

    func refreshSelection() {
        guard isViewLoaded else { return }
        // Keep the cells so UIKit updates separators with its selection highlight
        for (section, group) in groups.enumerated() {
            if let row = group.1.firstIndex(of: contents.selected) {
                let indexPath = IndexPath(row: row, section: section)
                if tableView.indexPathForSelectedRow != indexPath {
                    tableView.selectRow(at: indexPath, animated: false, scrollPosition: .none)
                }
            }
        }
    }

    override func numberOfSections(in tableView: UITableView) -> Int { groups.count }
    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { groups[section].1.count }
    override func tableView(_ tableView: UITableView, titleForHeaderInSection section: Int) -> String? { groups[section].0 }

    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let index = groups[indexPath.section].1[indexPath.row]
        let tab = contents.tabs[index]
        let cell = tableView.dequeueReusableCell(withIdentifier: "section", for: indexPath)
        var configuration = cell.defaultContentConfiguration()
        configuration.text = tab.item.title ?? tab.name
        configuration.image = tab.item.image
        cell.contentConfiguration = configuration
        cell.accessoryType = .disclosureIndicator
        cell.accessibilityIdentifier = "sidebar-\(tab.name)"
        return cell
    }

    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        choose?(groups[indexPath.section].1[indexPath.row])
    }
}
#endif
