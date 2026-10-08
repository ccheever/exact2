// The screen: top bar + one UICollectionView (compositional layout, estimated heights, diffable data
// source, prefetching), the 1 Hz live clock and BENCH_START_INDEX.
import UIKit

/// The content column C and its x origin, from the feed width (SPEC "Screen").
enum Geo {
    static var W: CGFloat = 0
    static var C: CGFloat = 0
    static var x0: CGFloat = 0
    static func set(width: CGFloat) {
        W = width
        C = min(width - 32, 600)
        x0 = ((width - C) / 2).rounded()
    }
}

let cellClasses: [String: FeedCell.Type] = [
    "photo": PhotoCell.self, "thumbs": ThumbsCell.self, "shader": ShaderCell.self, "canvas": CanvasCell.self,
    "svg": SvgCell.self, "video": VideoCell.self, "map": MapCell.self, "markdown": MarkdownCell.self,
    "code": CodeCell.self, "intl": IntlCell.self, "typeface": TypefaceCell.self, "carousel": CarouselCell.self,
    "motion": MotionCell.self, "glass": GlassCell.self, "live": LiveCell.self, "thread": ThreadCell.self,
    "webview": WebCell.self, "filmstrip": FilmstripCell.self, "inbox": InboxCell.self,
]

final class FeedViewController: UIViewController, UICollectionViewDelegate, UICollectionViewDataSourcePrefetching {
    private let rows = benchRows
    private var collectionView: UICollectionView!
    private var dataSource: UICollectionViewDiffableDataSource<Int, Int>!
    private let topBar = UIView()
    private let titleLabel = label(sys(17, .semibold))
    private let liveLabel = label(sys(13), .secondaryGray)
    private let topLine = UIView()
    private var timer: Timer?
    private var didStart = false

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .white
        titleLabel.text = "Extra Heavy"
        liveLabel.text = liveMode ? "Live: on" : "Live: off"
        topLine.backgroundColor = .hairline
        topBar.backgroundColor = .white
        topBar.addSubview(titleLabel)
        topBar.addSubview(liveLabel)
        topBar.addSubview(topLine)

        let size = NSCollectionLayoutSize(widthDimension: .fractionalWidth(1), heightDimension: .estimated(470))
        let item = NSCollectionLayoutItem(layoutSize: size)
        let group = NSCollectionLayoutGroup.vertical(layoutSize: size, subitems: [item])
        let layout = UICollectionViewCompositionalLayout(section: NSCollectionLayoutSection(group: group))
        collectionView = FeedCollectionView(frame: .zero, collectionViewLayout: layout)
        collectionView.allowsFocus = false
        collectionView.backgroundColor = .white
        collectionView.delegate = self
        collectionView.prefetchDataSource = self
        collectionView.isPrefetchingEnabled = true
        view.addSubview(collectionView)
        view.addSubview(topBar)

        // One reuse identifier per kind, each its own class (reuse pools stay per kind).
        for (kind, cls) in cellClasses { collectionView.register(cls, forCellWithReuseIdentifier: kind) }
        dataSource = UICollectionViewDiffableDataSource<Int, Int>(collectionView: collectionView) { [unowned self] cv, ip, i in
            let r = self.rows[i]
            let cell = cv.dequeueReusableCell(withReuseIdentifier: r.kind, for: ip) as! FeedCell
            cell.configure(r)
            return cell
        }
        var snap = NSDiffableDataSourceSnapshot<Int, Int>()
        // Header-less sections of 500 rows: with one section, each self-sized cell makes the compositional
        // layout re-solve every estimated row after it (the heavybench UIKit app's trace); in sections it
        // re-solves one section and shifts the rest. Invisible on screen. BENCH_CHUNK=0: one section.
        let chunk = ProcessInfo.processInfo.environment["BENCH_CHUNK"].flatMap(Int.init) ?? 500
        if chunk <= 0 { snap.appendSections([0]); snap.appendItems(Array(rows.indices), toSection: 0) } else {
            for (si, start) in stride(from: 0, to: rows.count, by: chunk).enumerated() {
                snap.appendSections([si]); snap.appendItems(Array(start..<min(start + chunk, rows.count)), toSection: si)
            }
        }
        dataSource.applySnapshotUsingReloadData(snap)

        if liveMode {
            let t = Timer(timeInterval: 1, repeats: true) { [weak self] _ in self?.tick() }
            RunLoop.main.add(t, forMode: .common)
            timer = t
        }
    }

    override func viewWillLayoutSubviews() {
        super.viewWillLayoutSubviews()
        let top = view.safeAreaInsets.top
        let w = view.bounds.width
        let th = ceil(titleLabel.font.lineHeight)
        let barH = 10 + th + 10
        topBar.frame = CGRect(x: 0, y: 0, width: w, height: top + barH + 0.5)
        let lx = view.safeAreaInsets.left + 16
        titleLabel.frame = CGRect(x: lx, y: top + 10, width: w / 2, height: th)
        let lw = ceil(liveLabel.sizeThatFits(.zero).width)
        liveLabel.frame = CGRect(x: w - view.safeAreaInsets.right - 16 - lw, y: top + 10 + (th - 16) / 2, width: lw, height: 16)
        topLine.frame = CGRect(x: 0, y: top + barH, width: w, height: 0.5)
        let y = top + barH + 0.5
        let newFrame = CGRect(x: 0, y: y, width: w, height: view.bounds.height - y)
        if collectionView.frame != newFrame {
            Geo.set(width: w)
            InboxMetrics.shared.warm(Geo.C)
            collectionView.frame = newFrame
            collectionView.collectionViewLayout.invalidateLayout()
        }
    }

    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        guard !didStart else { return }
        didStart = true
        // Like the SwiftUI app: estimated heights land one scrollTo near, not on, the row; repeat it.
        guard let s = startIndex, rows.indices.contains(s) else { return }
        for k in 0..<3 {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.25 * Double(k)) { [weak self] in
                guard let self, let ip = self.dataSource.indexPath(for: s) else { return }
                self.collectionView.scrollToItem(at: ip, at: .top, animated: false)
            }
        }
    }

    private func tick() {
        AppState.seconds += 1
        for case let cell as FeedCell in collectionView.visibleCells { cell.tick(in: collectionView) }
    }

    // Visibility drives the media (video, animated images, Lottie, shader, shimmer).
    func collectionView(_ cv: UICollectionView, willDisplay cell: UICollectionViewCell, forItemAt ip: IndexPath) {
        (cell as? FeedCell)?.setVisible(true)
    }
    func collectionView(_ cv: UICollectionView, didEndDisplaying cell: UICollectionViewCell, forItemAt ip: IndexPath) {
        (cell as? FeedCell)?.setVisible(false)
    }
    func collectionView(_ cv: UICollectionView, shouldSelectItemAt ip: IndexPath) -> Bool { false }
    func collectionView(_ cv: UICollectionView, shouldHighlightItemAt ip: IndexPath) -> Bool { false }

    // Prefetching: start the off-main decodes of a row's images before its cell is configured.
    func collectionView(_ cv: UICollectionView, prefetchItemsAt ips: [IndexPath]) {
        for ip in ips { if let i = dataSource.itemIdentifier(for: ip), rows.indices.contains(i) { prefetchImages(rows[i]) } }
    }
}

/// The images a row will ask for, at the pixel sizes its cell asks for them (same keys, so a hit).
func prefetchImages(_ r: Row) {
    let L = ImageLoader.shared
    let C = Geo.C
    L.prefetch(r.avatar, CGSize(width: 36, height: 36))
    switch r.kind {
    case "photo":
        if let p = r.photo { L.prefetch(p.src, CGSize(width: C, height: PhotoCell.photoHeight(p, C))) }
    case "glass":
        if let p = r.photo { L.prefetch(p.src, CGSize(width: C, height: (C * 3 / 4).rounded())) }
    case "shader":
        if let p = r.photo { L.prefetch(p.src, CGSize(width: C, height: (C * 9 / 16).rounded())) }
    case "thumbs":
        let side = (C - 12) / 4
        for t in r.thumbs ?? [] { L.prefetch(t, CGSize(width: side, height: side)) }
    case "canvas":
        if let i = r.image { L.prefetch(i, CGSize(width: 64, height: 64)) }
    case "carousel":
        for c in (r.cards ?? []).prefix(5) { L.prefetch(c.image, CGSize(width: 140, height: 100)) }
    default: break
    }
}

/// The list gives UIKit's focus system nothing to search: its cells are not focus items (`allowsFocus = false`)
/// and it answers focus searches with no items, so an iPad with a hardware keyboard does not walk every visible
/// cell's subtree on each focus update while scrolling (the cryptobench UIKit app traced it at 79 % of its late
/// frames; exact2's host does the same, `FocusSearchIOS.swift`). Taps, selection and first responders are unaffected.
final class FeedCollectionView: UICollectionView {
    override func focusItems(in rect: CGRect) -> [any UIFocusItem] { [] }
}
