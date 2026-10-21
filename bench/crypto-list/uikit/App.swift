// Crypto list benchmark, UIKit (see ../SPEC.md, "Decisions" → "UIKit").
// Written as a performance-minded iOS engineer would: a UICollectionView with a compositional
// layout (fixed 64 pt items) and a diffable data source of coin indices; cells laid out by hand;
// the sparkline, pulse and price flash are Core Animation (render-server) animations; a tick
// updates the model for all 40 coins and pushes the change straight into the visible cells.
import UIKit

// MARK: - Data

struct Coin: Decodable {
    let id: String
    let name: String
    let ticker: String
    let color: String
    var price: Double
    var change24h: Double
    var series: [Double]
    /// Bumped on every live update (a cell shows version v; re-applied on display if stale).
    var version: Int = 0
    enum CodingKeys: String, CodingKey { case id, name, ticker, color, price, change24h, series }
}

private struct CoinFile: Decodable { let version: Int; let coins: [Coin] }

private func loadCoins() -> [Coin] {
    guard let url = Bundle.main.url(forResource: "coins", withExtension: "json"),
          let data = try? Data(contentsOf: url),
          let file = try? JSONDecoder().decode(CoinFile.self, from: data)
    else { return [] }
    return file.coins
}

// MARK: - Colors, fonts, formats

extension UIColor {
    convenience init(hex: UInt32) {
        self.init(red: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
                  blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
    }
    convenience init(hexString s: String) { self.init(hex: UInt32(s.dropFirst(), radix: 16) ?? 0) }
    static let hairline = UIColor(hex: 0xE5E5EA)
    static let secondaryGray = UIColor(hex: 0x8E8E93)
    static let up = UIColor(hex: 0x16A34A)
    static let down = UIColor(hex: 0xDC2626)
}

private enum F {
    static let name = UIFont.systemFont(ofSize: 16, weight: .semibold)
    static let ticker = UIFont.systemFont(ofSize: 13)
    static let price = UIFont.monospacedDigitSystemFont(ofSize: 16, weight: .semibold)
    static let pill = UIFont.systemFont(ofSize: 12, weight: .semibold)
    static let letter = UIFont.systemFont(ofSize: 15, weight: .semibold)
}

private let grouped: NumberFormatter = {
    let f = NumberFormatter()
    f.locale = Locale(identifier: "en_US")
    f.numberStyle = .decimal
    f.minimumFractionDigits = 2
    f.maximumFractionDigits = 2
    return f
}()

/// `$61,234.56` at or above $1; four significant digits below (`$0.0009697`).
func formatPrice(_ p: Double) -> String {
    if p >= 1 { return "$" + (grouped.string(from: NSNumber(value: p)) ?? "") }
    let decimals = max(0, 3 - Int(floor(log10(p))))
    return "$" + String(format: "%.*f", decimals, p)
}

func formatChange(_ c: Double) -> String { (c >= 0 ? "+" : "-") + String(format: "%.2f%%", abs(c)) }

private let env = ProcessInfo.processInfo.environment
private let freeze = env["BENCH_FREEZE"] == "1"
private let liveMode = !freeze && (env["BENCH_LIVE"] == "1" || env["BENCH_SCENARIO"] == "rest")
private let startIndex = env["BENCH_START_INDEX"].flatMap(Int.init)

/// CSS `ease-out` = cubic-bezier(0, 0, 0.58, 1) = Core Animation's easeOut; CSS `ease`.
private let easeOut = CAMediaTimingFunction(name: .easeOut)
private let cssEase = CAMediaTimingFunction(controlPoints: 0.25, 0.1, 0.25, 1)

// MARK: - App

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication,
                     didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool { true }
}

/// Scene lifecycle (required by iOS 27; named in Info.plist's UIApplicationSceneManifest).
@objc(SceneDelegate)
final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let ws = scene as? UIWindowScene else { return }
        let w = UIWindow(windowScene: ws)
        w.overrideUserInterfaceStyle = .light
        w.rootViewController = ListController()
        w.makeKeyAndVisible()
        window = w
    }
}

final class ListController: UIViewController, UICollectionViewDelegate {
    private var coins = loadCoins()
    private var tick = 0
    private var collectionView: UICollectionView!
    private var dataSource: UICollectionViewDiffableDataSource<Int, Int>!
    private var timer: Timer?
    private var didStartScroll = false

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .white

        // Top bar (not in the list).
        let bar = UIView()
        let title = UILabel()
        title.text = "Markets"
        title.font = .systemFont(ofSize: 17, weight: .semibold)
        title.textColor = .black
        let live = UILabel()
        live.text = liveMode ? "Live: on" : "Live: off"
        live.font = .systemFont(ofSize: 13)
        live.textColor = .secondaryGray
        let rule = UIView()
        rule.backgroundColor = .hairline
        for v in [bar, title, live, rule] { v.translatesAutoresizingMaskIntoConstraints = false }
        bar.addSubview(title); bar.addSubview(live)
        view.addSubview(bar); view.addSubview(rule)

        // The list: one section of fixed 64 pt items.
        let size = NSCollectionLayoutSize(widthDimension: .fractionalWidth(1), heightDimension: .absolute(64))
        let group = NSCollectionLayoutGroup.vertical(layoutSize: size, subitems: [NSCollectionLayoutItem(layoutSize: size)])
        let layout = UICollectionViewCompositionalLayout(section: NSCollectionLayoutSection(group: group))
        collectionView = FeedCollectionView(frame: .zero, collectionViewLayout: layout)
        collectionView.allowsFocus = false
        collectionView.translatesAutoresizingMaskIntoConstraints = false
        collectionView.backgroundColor = .white
        collectionView.delegate = self
        view.addSubview(collectionView)

        NSLayoutConstraint.activate([
            bar.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            bar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            bar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            title.topAnchor.constraint(equalTo: bar.topAnchor, constant: 10),
            title.bottomAnchor.constraint(equalTo: bar.bottomAnchor, constant: -10),
            title.leadingAnchor.constraint(equalTo: bar.leadingAnchor, constant: 16),
            live.trailingAnchor.constraint(equalTo: bar.trailingAnchor, constant: -16),
            live.centerYAnchor.constraint(equalTo: title.centerYAnchor),
            rule.topAnchor.constraint(equalTo: bar.bottomAnchor),
            rule.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            rule.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            rule.heightAnchor.constraint(equalToConstant: 0.5),
            collectionView.topAnchor.constraint(equalTo: rule.bottomAnchor),
            collectionView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            collectionView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            collectionView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])

        let reg = UICollectionView.CellRegistration<CoinCell, Int> { [unowned self] cell, _, i in
            cell.apply(self.coins[i])
        }
        dataSource = UICollectionViewDiffableDataSource<Int, Int>(collectionView: collectionView) { cv, ip, i in
            cv.dequeueConfiguredReusableCell(using: reg, for: ip, item: i)
        }
        var snap = NSDiffableDataSourceSnapshot<Int, Int>()
        snap.appendSections([0])
        snap.appendItems(Array(coins.indices))
        dataSource.applySnapshotUsingReloadData(snap)

        if liveMode {
            let t = Timer(timeInterval: 0.1, repeats: true) { [weak self] _ in
                guard let self else { return }
                self.tick += 1
                self.liveTick(self.tick)
            }
            RunLoop.main.add(t, forMode: .common)
            timer = t
        }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        if !didStartScroll, let startIndex, coins.indices.contains(startIndex), collectionView.bounds.height > 0 {
            didStartScroll = true
            collectionView.layoutIfNeeded()
            collectionView.scrollToItem(at: IndexPath(item: startIndex, section: 0), at: .top, animated: false)
        }
    }

    /// Tick k updates 40 coins (SPEC "Live ticks"). The model changes for all 40; a coin whose cell
    /// is on screen is pushed into that cell directly (no snapshot apply: identities never change).
    private func liveTick(_ k: Int) {
        let n = coins.count
        guard n > 0 else { return }
        for j in 0..<40 {
            let i = (k * 7919 + j * 104_729) % n
            let delta = Double((k * 31 + j * 17) % 201 - 100) / 10_000
            coins[i].price *= 1 + delta
            coins[i].series.removeFirst()
            coins[i].series.append(coins[i].price)
            coins[i].change24h += delta * 100
            coins[i].version += 1
            if let cell = collectionView.cellForItem(at: IndexPath(item: i, section: 0)) as? CoinCell {
                cell.update(coins[i], flashUp: delta >= 0)
            }
        }
    }

    func collectionView(_ cv: UICollectionView, willDisplay cell: UICollectionViewCell, forItemAt ip: IndexPath) {
        guard let cell = cell as? CoinCell else { return }
        // A prefetched cell configured before a tick shows the new values without a flash.
        if cell.shownVersion != coins[ip.item].version { cell.apply(coins[ip.item]) }
        cell.appear()
    }

    func collectionView(_ cv: UICollectionView, didEndDisplaying cell: UICollectionViewCell, forItemAt ip: IndexPath) {
        (cell as? CoinCell)?.disappear()
    }
}

/// The list holds nothing UIKit can focus (no selection, no controls): its cells are not focus items
/// (`allowsFocus = false`) and it answers UIKit's focus search with no items, so an iPad with a hardware
/// keyboard does not map every visible cell's subtree on each focus update while scrolling (exact2's
/// host does the same, `FocusSearchIOS.swift`).
final class FeedCollectionView: UICollectionView {
    override func focusItems(in rect: CGRect) -> [any UIFocusItem] { [] }
}

// MARK: - Cell

final class CoinCell: UICollectionViewCell {
    private let icon = UIView()
    private let letter = UILabel()
    private let name = UILabel()
    private let ticker = UILabel()
    private let price = UILabel()
    /// The flash: the same text in the flash colour over the black price, faded out (render server).
    private let priceFlash = UILabel()
    private let pill = UIView()
    private let pillText = UILabel()
    private let chart = UIView()
    private let line = CAShapeLayer()
    private let ring = CAShapeLayer()
    private let dot = CAShapeLayer()
    private let separator = CALayer()
    private var lastPoint = CGPoint.zero
    private(set) var shownVersion = -1
    private var up = true

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .white
        contentView.backgroundColor = .white
        icon.layer.cornerRadius = 16
        letter.font = F.letter; letter.textColor = .white; letter.textAlignment = .center
        name.font = F.name; name.textColor = .black; name.lineBreakMode = .byTruncatingTail
        ticker.font = F.ticker; ticker.textColor = .secondaryGray; ticker.lineBreakMode = .byTruncatingTail
        for l in [price, priceFlash] { l.font = F.price; l.textAlignment = .right; l.textColor = .black }
        priceFlash.alpha = 0
        pill.layer.cornerRadius = 4
        pillText.font = F.pill; pillText.textColor = .white
        line.fillColor = nil; line.lineWidth = 1.5; line.lineJoin = .round; line.lineCap = .round
        ring.path = UIBezierPath(ovalIn: CGRect(x: -9, y: -9, width: 18, height: 18)).cgPath
        dot.path = UIBezierPath(ovalIn: CGRect(x: -3, y: -3, width: 6, height: 6)).cgPath
        for l in [line, ring, dot] { chart.layer.addSublayer(l) }
        // Chart layers never implicitly animate (a tick moves the path and the pulse at once).
        let none: [String: CAAction] = ["path": NSNull(), "position": NSNull(), "fillColor": NSNull(),
                                        "strokeColor": NSNull(), "strokeEnd": NSNull(), "opacity": NSNull(),
                                        "transform": NSNull(), "bounds": NSNull()]
        for l in [line, ring, dot] { l.actions = none }
        separator.backgroundColor = UIColor.hairline.cgColor
        separator.actions = ["position": NSNull(), "bounds": NSNull()]
        for v in [icon, name, ticker, chart, price, priceFlash, pill] { contentView.addSubview(v) }
        icon.addSubview(letter); pill.addSubview(pillText)
        contentView.layer.addSublayer(separator)
        if freeze {
            ring.transform = CATransform3DMakeScale(2.0 / 3, 2.0 / 3, 1) // radius 6
            ring.opacity = 0.25
        } else {
            ring.opacity = 0 // shown by the pulse animation only
        }
    }

    required init?(coder: NSCoder) { fatalError() }

    /// Full configuration (dequeue, or a stale prefetched cell).
    func apply(_ c: Coin) {
        shownVersion = c.version
        icon.backgroundColor = UIColor(hexString: c.color)
        letter.text = String(c.ticker.prefix(1))
        name.text = c.name
        ticker.text = c.ticker
        priceFlash.layer.removeAllAnimations()
        priceFlash.alpha = 0
        setValues(c)
    }

    /// A live tick on an on-screen coin: new values, chart at full length, flash restarted.
    func update(_ c: Coin, flashUp: Bool) {
        shownVersion = c.version
        setValues(c)
        priceFlash.textColor = flashUp ? .up : .down
        let a = CABasicAnimation(keyPath: "opacity")
        a.fromValue = 1; a.toValue = 0; a.duration = 0.4; a.timingFunction = cssEase
        priceFlash.layer.add(a, forKey: "flash") // replaces a running flash: restarts it
    }

    private func setValues(_ c: Coin) {
        let text = formatPrice(c.price)
        price.text = text
        priceFlash.text = text
        up = c.change24h >= 0
        let color = up ? UIColor.up : UIColor.down
        pill.backgroundColor = color
        pillText.text = formatChange(c.change24h)
        let cg = color.cgColor
        line.strokeColor = cg; ring.fillColor = cg; dot.fillColor = cg
        let path = CGMutablePath()
        let pts = points(c.series, w: 96, h: 32)
        if let first = pts.first { path.move(to: first); path.addLines(between: pts) }
        line.path = path
        lastPoint = pts.last ?? .zero
        ring.position = lastPoint
        dot.position = lastPoint
        setNeedsLayout()
    }

    /// Each time the row comes on screen: draw-in 600 ms ease-out, then the dot and the pulse.
    func appear() {
        guard !freeze else { return }
        let now = CACurrentMediaTime()
        let draw = CABasicAnimation(keyPath: "strokeEnd")
        draw.fromValue = 0; draw.toValue = 1; draw.duration = 0.6; draw.timingFunction = easeOut
        line.add(draw, forKey: "draw")
        let hide = CABasicAnimation(keyPath: "opacity")
        hide.fromValue = 0; hide.toValue = 0; hide.duration = 0.6
        dot.add(hide, forKey: "hide")
        let scale = CABasicAnimation(keyPath: "transform.scale")
        scale.fromValue = 1.0 / 3; scale.toValue = 1
        let fade = CABasicAnimation(keyPath: "opacity")
        fade.fromValue = 0.5; fade.toValue = 0
        let pulse = CAAnimationGroup()
        pulse.animations = [scale, fade]
        pulse.duration = 1.2
        pulse.timingFunction = easeOut
        pulse.repeatCount = .infinity
        pulse.beginTime = ring.convertTime(now, from: nil) + 0.6
        ring.add(pulse, forKey: "pulse")
    }

    func disappear() {
        line.removeAllAnimations(); dot.removeAllAnimations(); ring.removeAllAnimations()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        let w = bounds.width, h: CGFloat = 64
        icon.frame = CGRect(x: 16, y: (h - 32) / 2, width: 32, height: 32)
        letter.frame = icon.bounds
        let priceX = w - 16 - 104
        let chartX = priceX - 12 - 96
        chart.frame = CGRect(x: chartX, y: (h - 32) / 2, width: 96, height: 32)
        // Name block: 16 pt line, 2, 13 pt line, centred.
        let nameX: CGFloat = 16 + 32 + 12
        let nameW = max(0, chartX - 12 - nameX)
        let n1 = ceil(F.name.lineHeight), n2 = ceil(F.ticker.lineHeight)
        let ny = (h - (n1 + 2 + n2)) / 2
        name.frame = CGRect(x: nameX, y: ny, width: nameW, height: n1)
        ticker.frame = CGRect(x: nameX, y: ny + n1 + 2, width: nameW, height: n2)
        // Price block: price, 4, pill (2/6 padding), right-aligned, centred.
        let p1 = ceil(F.price.lineHeight)
        let ts = pillText.sizeThatFits(CGSize(width: 104, height: 100))
        let pw = ceil(ts.width) + 12, ph = ceil(F.pill.lineHeight) + 4
        let py = (h - (p1 + 4 + ph)) / 2
        price.frame = CGRect(x: priceX, y: py, width: 104, height: p1)
        priceFlash.frame = price.frame
        pill.frame = CGRect(x: w - 16 - pw, y: py + p1 + 4, width: pw, height: ph)
        pillText.frame = CGRect(x: 6, y: 2, width: ceil(ts.width), height: ph - 4)
        let px = 1 / (window?.screen.scale ?? UIScreen.main.scale)
        separator.frame = CGRect(x: 0, y: h - px, width: w, height: px)
    }
}

/// `x_i = 96·i/47`, `y_i = 32 − 32·(v − min)/(max − min)` (16 for a flat series).
func points(_ s: [Double], w: Double, h: Double) -> [CGPoint] {
    guard s.count > 1, let lo = s.min(), let hi = s.max() else { return [] }
    let n = Double(s.count - 1)
    return s.enumerated().map { i, v in
        CGPoint(x: w * Double(i) / n, y: hi > lo ? h - h * (v - lo) / (hi - lo) : h / 2)
    }
}
