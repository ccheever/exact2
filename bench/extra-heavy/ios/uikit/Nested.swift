// Nested collection views: 12 carousel (10 cards, starts at 0 on reuse), 18 filmstrip (2,000 items,
// horizontal) and 19 inbox (1,000 messages in a 400 pt box). Each is its own UICollectionView with cell
// reuse; the strip and the inbox prefetch their images and keep their offset per row id in app state.
import UIKit

func noInsets(_ cv: UICollectionView) {
    cv.contentInsetAdjustmentBehavior = .never
    cv.contentInset = .zero
    cv.backgroundColor = .white
}

// MARK: 12 carousel

final class CardCell: UICollectionViewCell {
    let image = LoadingImageView()
    let title = label(sys(13, .semibold), lines: 2)
    let meta = label(sys(12), .secondaryGray)
    override init(frame: CGRect) {
        super.init(frame: frame)
        image.layer.cornerRadius = 10
        image.frame = CGRect(x: 0, y: 0, width: 140, height: 100)
        for v in [image, title, meta] as [UIView] { contentView.addSubview(v) }
    }
    required init?(coder: NSCoder) { fatalError() }
    func configure(_ c: Card) {
        image.set(c.image, CGSize(width: 140, height: 100))
        title.text = c.title
        meta.text = c.meta
        let th = fit(title, 140)
        title.frame = CGRect(x: 0, y: 106, width: 140, height: min(th, 34))
        meta.frame = CGRect(x: 0, y: 106 + 34 + 4, width: 140, height: ceil(meta.font.lineHeight))
    }
}

final class CarouselCell: FeedCell, UICollectionViewDataSource {
    let title = label(sys(17, .semibold))
    lazy var strip: UICollectionView = {
        let l = UICollectionViewFlowLayout()
        l.scrollDirection = .horizontal
        l.itemSize = CGSize(width: 140, height: 170)
        l.minimumLineSpacing = 10
        let cv = FeedCollectionView(frame: .zero, collectionViewLayout: l)
        cv.allowsFocus = false
        noInsets(cv)
        cv.showsHorizontalScrollIndicator = false
        cv.dataSource = self
        cv.register(CardCell.self, forCellWithReuseIdentifier: "c")
        return cv
    }()
    private var cards: [Card] = []
    override func setup() {
        contentView.addSubview(title)
        contentView.addSubview(strip)
    }
    override func configureBody(_ r: Row) {
        title.text = r.title
        cards = r.cards!
        strip.reloadData()
        strip.contentOffset = .zero   // a recycled row starts at its first card
    }
    func collectionView(_ cv: UICollectionView, numberOfItemsInSection s: Int) -> Int { cards.count }
    func collectionView(_ cv: UICollectionView, cellForItemAt ip: IndexPath) -> UICollectionViewCell {
        let c = cv.dequeueReusableCell(withReuseIdentifier: "c", for: ip) as! CardCell
        c.configure(cards[ip.item])
        return c
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let th = ceil(title.font.lineHeight)
        title.frame = CGRect(x: x, y: y, width: C, height: th)
        strip.frame = CGRect(x: x, y: y + th + 10, width: C, height: 170)
        return th + 10 + 170
    }
}

// MARK: shared inner-list plumbing (offset kept per row id)

class InnerListCell: FeedCell, UICollectionViewDataSource, UICollectionViewDelegate, UICollectionViewDataSourcePrefetching {
    let title = label(sys(15, .semibold))
    var inner: UICollectionView!
    var horizontal: Bool { false }
    private var restoring = false

    func makeLayout() -> UICollectionViewLayout { UICollectionViewFlowLayout() }
    func registerCells(_ cv: UICollectionView) {}
    func itemCount() -> Int { 0 }
    func cell(_ cv: UICollectionView, _ ip: IndexPath) -> UICollectionViewCell { UICollectionViewCell() }
    func prefetch(_ j: Int) {}

    override func setup() {
        inner = FeedCollectionView(frame: .zero, collectionViewLayout: makeLayout())
        inner.allowsFocus = false
        noInsets(inner)
        inner.dataSource = self
        inner.delegate = self
        inner.prefetchDataSource = self
        registerCells(inner)
        contentView.addSubview(title)
    }
    override func configureBody(_ r: Row) {
        title.text = r.title
        restoring = true
        inner.reloadData()
        restoring = false
        restoreOffset()
    }
    func restoreOffset() {
        guard let row, inner.bounds.width > 0 else { return }
        restoring = true
        inner.layoutIfNeeded()
        let v = AppState.innerOffsets[row.id] ?? 0
        inner.contentOffset = horizontal ? CGPoint(x: v, y: 0) : CGPoint(x: 0, y: v)
        restoring = false
    }
    func scrollViewDidScroll(_ sv: UIScrollView) {
        guard !restoring, let row, sv === inner else { return }
        AppState.innerOffsets[row.id] = horizontal ? sv.contentOffset.x : sv.contentOffset.y
    }
    func collectionView(_ cv: UICollectionView, numberOfItemsInSection s: Int) -> Int { row == nil ? 0 : itemCount() }
    func collectionView(_ cv: UICollectionView, cellForItemAt ip: IndexPath) -> UICollectionViewCell { cell(cv, ip) }
    func collectionView(_ cv: UICollectionView, prefetchItemsAt ips: [IndexPath]) { for ip in ips { prefetch(ip.item) } }
}

// MARK: 18 filmstrip

func filmImage(_ row: Row, _ j: Int) -> String { String(format: "small-%02d.jpg", (row.img0! + j * row.imgStep!) % 48) }

final class FilmCell: UICollectionViewCell {
    let image = LoadingImageView()
    let caption = label(sys(12), .label2)
    override init(frame: CGRect) {
        super.init(frame: frame)
        image.layer.cornerRadius = 8
        image.frame = CGRect(x: 0, y: 0, width: 112, height: 112)
        caption.frame = CGRect(x: 0, y: 116, width: 112, height: ceil(caption.font.lineHeight))
        contentView.addSubview(image)
        contentView.addSubview(caption)
    }
    required init?(coder: NSCoder) { fatalError() }
}

final class FilmstripCell: InnerListCell {
    override var horizontal: Bool { true }
    override func makeLayout() -> UICollectionViewLayout {
        let l = UICollectionViewFlowLayout()
        l.scrollDirection = .horizontal
        l.itemSize = CGSize(width: 112, height: 136)
        l.minimumLineSpacing = 8
        return l
    }
    override func setup() {
        super.setup()
        inner.showsHorizontalScrollIndicator = false
        contentView.addSubview(inner)
    }
    override func registerCells(_ cv: UICollectionView) { cv.register(FilmCell.self, forCellWithReuseIdentifier: "f") }
    override func itemCount() -> Int { row.count ?? 0 }
    override func cell(_ cv: UICollectionView, _ ip: IndexPath) -> UICollectionViewCell {
        let c = cv.dequeueReusableCell(withReuseIdentifier: "f", for: ip) as! FilmCell
        c.image.set(filmImage(row, ip.item), CGSize(width: 112, height: 112))
        c.caption.text = "IMG_\(row.num0! + ip.item)"
        return c
    }
    override func prefetch(_ j: Int) { ImageLoader.shared.prefetch(filmImage(row, j), CGSize(width: 112, height: 112)) }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let th = ceil(title.font.lineHeight)
        title.frame = CGRect(x: x, y: y, width: C, height: th)
        let f = CGRect(x: x, y: y + th + 10, width: C, height: 136)
        if inner.frame != f { inner.frame = f; restoreOffset() }
        return th + 10 + 136
    }
}

// MARK: 19 inbox

func clockLabel(_ row: Row, _ j: Int) -> String {
    let m = ((row.clock0! - 7 * j) % 1440 + 1440) % 1440
    return String(format: "%02d:%02d", m / 60, m % 60)
}

/// Whether each pool message's text needs two lines at the inbox width: measured once per width, off
/// the main thread at launch (the row height is 10 + name 17 + 2 + text + 10, clamped to two lines).
final class InboxMetrics {
    static let shared = InboxMetrics()
    private var width: CGFloat = 0
    private var twoLine: [Bool] = []
    private let lock = NSLock()
    static let textFont = sys(13)
    static func textWidth(_ C: CGFloat) -> CGFloat { C - 24 - 42 }

    func warm(_ C: CGFloat) {
        DispatchQueue.global(qos: .userInitiated).async { _ = self.lines(C) }
    }
    func lines(_ C: CGFloat) -> [Bool] {
        lock.lock(); defer { lock.unlock() }
        if width == C, !twoLine.isEmpty { return twoLine }
        let w = InboxMetrics.textWidth(C)
        let one = ceil(InboxMetrics.textFont.lineHeight)
        twoLine = (feed.messages ?? []).map { m in
            let r = (m.text as NSString).boundingRect(with: CGSize(width: w, height: .greatestFiniteMagnitude),
                                                      options: [.usesLineFragmentOrigin], attributes: [.font: InboxMetrics.textFont], context: nil)
            return ceil(r.height) > one + 1
        }
        width = C
        return twoLine
    }
    static func height(twoLine: Bool) -> CGFloat {
        let name = ceil(sys(14, .semibold).lineHeight), text = ceil(textFont.lineHeight) * (twoLine ? 2 : 1)
        return 10 + max(32, name + 2 + text) + 10
    }
}

final class MessageCell: UICollectionViewCell {
    let avatar = LoadingImageView()
    let name = label(sys(14, .semibold))
    let time = label(UIFont.monospacedDigitSystemFont(ofSize: 12, weight: .regular), .secondaryGray)
    let text = label(InboxMetrics.textFont, .label2, lines: 2)
    let line = CALayer()
    override init(frame: CGRect) {
        super.init(frame: frame)
        contentView.backgroundColor = .white
        avatar.layer.cornerRadius = 16
        for v in [avatar, name, time, text] as [UIView] { contentView.addSubview(v) }
        line.backgroundColor = UIColor.hairline.cgColor
        layer.addSublayer(line)
    }
    required init?(coder: NSCoder) { fatalError() }
    func configure(_ m: Message, _ t: String) {
        avatar.set(m.avatar, CGSize(width: 32, height: 32))
        name.text = m.name
        time.text = t
        text.text = m.text
        setNeedsLayout()
    }
    override func layoutSubviews() {
        super.layoutSubviews()
        let W = bounds.width
        avatar.frame = CGRect(x: 12, y: 10, width: 32, height: 32)
        let x = 54.0
        let nh = ceil(name.font.lineHeight)
        let tw = ceil(time.sizeThatFits(.zero).width)
        time.frame = CGRect(x: W - 12 - tw, y: (nh - ceil(time.font.lineHeight)) / 2 + 10, width: tw, height: ceil(time.font.lineHeight))
        name.frame = CGRect(x: x, y: 10, width: max(0, time.frame.minX - 8 - x), height: nh)
        text.frame = CGRect(x: x, y: 10 + nh + 2, width: W - 12 - x, height: bounds.height - 20 - nh - 2)
        CATransaction.begin(); CATransaction.setDisableActions(true)
        line.frame = CGRect(x: 54, y: bounds.height - 0.5, width: W - 54, height: 0.5)
        CATransaction.commit()
    }
}

final class InboxCell: InnerListCell, UICollectionViewDelegateFlowLayout {
    let box = UIView()
    private var twoLine: [Bool] = []
    private var pool: [Message] { feed.messages ?? [] }
    override func setup() {
        super.setup()
        rounded(box, 12)
        box.backgroundColor = .white
        box.layer.borderWidth = 0.5
        box.layer.borderColor = UIColor.border.cgColor
        box.addSubview(inner)
        contentView.addSubview(box)
    }
    override func makeLayout() -> UICollectionViewLayout {
        let l = UICollectionViewFlowLayout()
        l.minimumLineSpacing = 0
        return l
    }
    override func registerCells(_ cv: UICollectionView) { cv.register(MessageCell.self, forCellWithReuseIdentifier: "m") }
    private func msgIndex(_ j: Int) -> Int { (row.m0! + j * row.mStep!) % pool.count }
    override func itemCount() -> Int { pool.isEmpty ? 0 : row.count ?? 0 }
    override func cell(_ cv: UICollectionView, _ ip: IndexPath) -> UICollectionViewCell {
        let c = cv.dequeueReusableCell(withReuseIdentifier: "m", for: ip) as! MessageCell
        c.configure(pool[msgIndex(ip.item)], clockLabel(row, ip.item))
        return c
    }
    override func prefetch(_ j: Int) { ImageLoader.shared.prefetch(pool[msgIndex(j)].avatar, CGSize(width: 32, height: 32)) }
    func collectionView(_ cv: UICollectionView, layout l: UICollectionViewLayout, sizeForItemAt ip: IndexPath) -> CGSize {
        let two = twoLine.isEmpty ? true : twoLine[msgIndex(ip.item)]
        return CGSize(width: cv.bounds.width, height: InboxMetrics.height(twoLine: two))
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let th = ceil(title.font.lineHeight)
        title.frame = CGRect(x: x, y: y, width: C, height: th)
        let f = CGRect(x: x, y: y + th + 10, width: C, height: 400)
        if box.frame != f || twoLine.isEmpty {
            box.frame = f
            twoLine = InboxMetrics.shared.lines(C)
            inner.frame = box.bounds
            inner.collectionViewLayout.invalidateLayout()
            restoreOffset()
        }
        return th + 10 + 400
    }
}
