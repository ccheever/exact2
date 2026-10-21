// Heavy list benchmark, UIKit (see ../SPEC.md "UIKit" and README.md).
// Written the way a performance-minded iOS engineer builds a feed:
// UICollectionView + compositional layout + diffable data source, reused cells laid out by hand
// from a row layout computed off the main thread during prefetch (attributed strings + measured
// frames), UIImage.byPreparingThumbnail decode off the main thread with an NSCache, prefetching.
import UIKit

// MARK: - Data (the same JSON as the other apps)

struct Run: Codable { let t: String; let s: String? }
struct Photo: Codable { let src: String; let w: Double; let h: Double }
struct LinkCard: Codable { let thumb: String; let title: String; let description: String; let site: String }
struct Quote: Codable { let id: String; let author: String; let excerpt: String }
struct Reaction: Codable { let emoji: String; var count: Int }

struct Message: Codable {
    var id: String
    var index: Int
    let author: String
    let avatar: String
    var minutesAgo: Int
    let paragraphs: [[Run]]
    let photos: [Photo]?
    let link: LinkCard?
    let quote: Quote?
    var reactions: [Reaction]?
    var bornAt: Int? = nil
    var key: Int? = nil        // the diffable item identifier (ordinal; live inserts 1,000,000 + k)
    var rev: Int? = nil        // bumped when reactions change (the layout cache key)
}

private struct MessageFile: Codable { let version: Int; let messages: [Message] }

private func loadMessages() -> [Message] {
    guard let url = Bundle.main.url(forResource: "messages", withExtension: "json"),
          let data = try? Data(contentsOf: url),
          let file = try? JSONDecoder().decode(MessageFile.self, from: data) else { return [] }
    return file.messages
}

private let env = ProcessInfo.processInfo.environment
/// BENCH_TIMING=1: diagnostics to NSLog and Documents/timing.txt (devicectl copies it back).
func timingLog(_ s: String) {
    guard env["BENCH_TIMING"] == "1" else { return }
    NSLog("%@", s)
    let url = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("timing.txt")
    let line = String(format: "%.1f %@\n", CACurrentMediaTime() * 1000, s)
    if let h = try? FileHandle(forWritingTo: url) { h.seekToEndOfFile(); h.write(line.data(using: .utf8)!); try? h.close() }
    else { try? line.write(to: url, atomically: false, encoding: .utf8) }
}
private let liveMode = env["BENCH_LIVE"] == "1"
private let startIndex = env["BENCH_START_INDEX"].flatMap(Int.init)

extension UIColor {
    convenience init(hex: UInt32) {
        self.init(red: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
                  blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
    }
    static let hairline = UIColor(hex: 0xE5E5EA)
    static let secondaryGray = UIColor(hex: 0x8E8E93)
    static let quoteBar = UIColor(hex: 0xC7C7CC)
    static let fill = UIColor(hex: 0xF2F2F7)
    static let label2 = UIColor(hex: 0x3C3C43)
    static let cardBorder = UIColor(hex: 0xD1D1D6)
    static let linkBlue = UIColor(hex: 0x007AFF)
    static let tagPurple = UIColor(hex: 0x5856D6)
}

enum Fonts {
    static let author = UIFont.systemFont(ofSize: 15, weight: .semibold)
    static let time = UIFont.systemFont(ofSize: 13)
    static let quoteAuthor = UIFont.systemFont(ofSize: 13, weight: .semibold)
    static let quoteText = UIFont.systemFont(ofSize: 13)
    static let body = UIFont.systemFont(ofSize: 16)
    static let bold = UIFont.systemFont(ofSize: 16, weight: .semibold)
    static let italic = UIFont.italicSystemFont(ofSize: 16)
    static let code = UIFont.monospacedSystemFont(ofSize: 15, weight: .regular)
    static let site = UIFont.systemFont(ofSize: 12)
    static let linkTitle = UIFont.systemFont(ofSize: 15, weight: .semibold)
    static let linkDesc = UIFont.systemFont(ofSize: 13)
    static let emoji = UIFont.systemFont(ofSize: 14)
    static let count = UIFont.systemFont(ofSize: 13, weight: .semibold)
}

func relativeTime(_ m: Message, seconds: Int) -> String {
    let minutes = m.minutesAgo + (seconds - (m.bornAt ?? 0)) / 60
    if minutes < 60 { return "\(minutes)m ago" }
    if minutes < 60 * 24 { return "\(minutes / 60)h ago" }
    return "\(minutes / (60 * 24))d ago"
}

/// One paragraph as one attributed string (the SwiftUI app's run styles).
func paragraphText(_ runs: [Run]) -> NSAttributedString {
    let out = NSMutableAttributedString()
    for run in runs {
        var a: [NSAttributedString.Key: Any] = [.font: Fonts.body, .foregroundColor: UIColor.black]
        switch run.s {
        case "bold": a[.font] = Fonts.bold
        case "italic": a[.font] = Fonts.italic
        case "code": a[.font] = Fonts.code; a[.backgroundColor] = UIColor.fill
        case "link": a[.foregroundColor] = UIColor.linkBlue; a[.underlineStyle] = NSUnderlineStyle.single.rawValue
        case "mention": a[.font] = Fonts.bold; a[.foregroundColor] = UIColor.linkBlue
        case "tag": a[.foregroundColor] = UIColor.tagPurple
        default: break
        }
        out.append(NSAttributedString(string: run.t, attributes: a))
    }
    return out
}

// MARK: - Row layout: every frame of a row, computed once per (id, rev, width), thread-safe

private func textHeight(_ s: NSAttributedString, _ w: CGFloat) -> CGFloat {
    ceil(s.boundingRect(with: CGSize(width: w, height: .greatestFiniteMagnitude),
                        options: [.usesLineFragmentOrigin, .usesFontLeading], context: nil).height)
}
private func textHeight(_ s: String, _ f: UIFont, _ w: CGFloat, maxLines: Int) -> CGFloat {
    let full = textHeight(NSAttributedString(string: s, attributes: [.font: f]), w)
    return min(full, ceil(f.lineHeight * CGFloat(maxLines)))
}
private func textWidth(_ s: String, _ f: UIFont) -> CGFloat {
    ceil((s as NSString).size(withAttributes: [.font: f]).width)
}

final class RowLayout {
    let width: CGFloat
    var height: CGFloat = 0
    let avatar = CGRect(x: 16, y: 12, width: 40, height: 40)
    var authorFrame = CGRect.zero, timeOrigin = CGPoint.zero, timeMaxWidth: CGFloat = 0
    var quoteBox: CGRect?, quoteAuthor = CGRect.zero, quoteText = CGRect.zero
    var paragraphs: [(NSAttributedString, CGRect)] = []
    var photoBox: CGRect?, photoFrames: [CGRect] = []          // photo frames are relative to photoBox
    var linkBox: CGRect?, linkSite = CGRect.zero, linkTitle = CGRect.zero, linkDesc = CGRect.zero
    var chips: [CGRect] = []                                       // absolute

    init(_ m: Message, width: CGFloat) {
        self.width = width
        let x: CGFloat = 68, cw = width - 68 - 16
        var y: CGFloat = 12
        let aw = min(textWidth(m.author, Fonts.author), cw)
        let ah = ceil(Fonts.author.lineHeight)
        authorFrame = CGRect(x: x, y: y, width: aw, height: ah)
        // first-baseline alignment of the time with the author
        timeOrigin = CGPoint(x: x + aw + 6, y: y + (Fonts.author.ascender - Fonts.time.ascender))
        timeMaxWidth = max(0, cw - aw - 6)
        y += ah
        if let q = m.quote {
            let tw = cw - 11 - 8
            let qa = ceil(Fonts.quoteAuthor.lineHeight)
            let qt = textHeight(q.excerpt, Fonts.quoteText, tw, maxLines: 2)
            let box = CGRect(x: x, y: y + 6, width: cw, height: 8 + qa + qt + 8)
            quoteBox = box
            quoteAuthor = CGRect(x: 11, y: 8, width: tw, height: qa)
            quoteText = CGRect(x: 11, y: 8 + qa, width: tw, height: qt)
            y = box.maxY
        }
        y += 6
        for (i, p) in m.paragraphs.enumerated() {
            let s = paragraphText(p)
            let h = textHeight(s, cw)
            if i > 0 { y += 8 }
            paragraphs.append((s, CGRect(x: x, y: y, width: cw, height: h)))
            y += h
        }
        if let photos = m.photos, !photos.isEmpty {
            y += 8
            var f: [CGRect] = []
            var h: CGFloat
            switch photos.count {
            case 1:
                h = min(cw * photos[0].h / photos[0].w, 320)
                f = [CGRect(x: 0, y: 0, width: cw, height: h)]
            case 2:
                let s = (cw - 4) / 2; h = s
                f = [CGRect(x: 0, y: 0, width: s, height: s), CGRect(x: s + 4, y: 0, width: s, height: s)]
            case 3:
                let small = (cw - 4) / 3, big = cw - 4 - small, sh = (big - 4) / 2; h = big
                f = [CGRect(x: 0, y: 0, width: big, height: big),
                     CGRect(x: big + 4, y: 0, width: small, height: sh),
                     CGRect(x: big + 4, y: sh + 4, width: small, height: sh)]
            default:
                let s = (cw - 4) / 2; h = s * 2 + 4
                f = [CGRect(x: 0, y: 0, width: s, height: s), CGRect(x: s + 4, y: 0, width: s, height: s),
                     CGRect(x: 0, y: s + 4, width: s, height: s), CGRect(x: s + 4, y: s + 4, width: s, height: s)]
            }
            photoBox = CGRect(x: x, y: y, width: cw, height: h)
            photoFrames = f
            y += h
        }
        if let l = m.link {
            y += 8
            let tw = cw - 20
            let sh = ceil(Fonts.site.lineHeight)
            let th = textHeight(l.title, Fonts.linkTitle, tw, maxLines: 2)
            let dh = textHeight(l.description, Fonts.linkDesc, tw, maxLines: 2)
            linkSite = CGRect(x: 10, y: 150, width: tw, height: sh)
            linkTitle = CGRect(x: 10, y: 150 + sh, width: tw, height: th)
            linkDesc = CGRect(x: 10, y: 150 + sh + th, width: tw, height: dh)
            let box = CGRect(x: x, y: y, width: cw, height: 150 + sh + th + dh + 10)
            linkBox = box
            y = box.maxY
        }
        if let rs = m.reactions, !rs.isEmpty {
            y += 8
            var cx: CGFloat = 0, cy: CGFloat = 0
            for r in rs {
                let w = 10 + textWidth(r.emoji, Fonts.emoji) + 4 + textWidth("\(r.count)", Fonts.count) + 10
                if cx > 0 && cx + w > cw { cx = 0; cy += 28 + 6 }
                chips.append(CGRect(x: x + cx, y: y + cy, width: w, height: 28))
                cx += w + 6
            }
            y += cy + 28
        }
        height = ceil(y + 12)
    }
}

/// Layouts by "id#rev@width", bounded (the collection view keeps measured heights itself).
/// Built on the prefetch queue or, on a miss, on the main thread. NSCache is thread-safe.
final class LayoutCache: @unchecked Sendable {
    static let shared = LayoutCache()
    private let map: NSCache<NSString, RowLayout> = { let c = NSCache<NSString, RowLayout>(); c.countLimit = 400; return c }()
    static func key(_ m: Message, _ w: CGFloat) -> String { "\(m.id)#\(m.rev ?? 0)@\(Int(w))" }
    func layout(_ m: Message, _ w: CGFloat) -> RowLayout {
        let k = Self.key(m, w) as NSString
        if let l = map.object(forKey: k) { return l }
        let l = RowLayout(m, width: w); map.setObject(l, forKey: k); return l
    }
}

// MARK: - Images: byPreparingThumbnail off the main thread, NSCache (300, as the SwiftUI app)

final class ImageLoader {
    static let shared = ImageLoader()
    private let cache: NSCache<NSString, UIImage> = { let c = NSCache<NSString, UIImage>(); c.countLimit = 300; return c }()
    private var waiting: [String: [(UIImage?) -> Void]] = [:]     // main thread only
    private let queue = DispatchQueue(label: "decode", qos: .userInitiated, attributes: .concurrent)
    let scale = UIScreen.main.scale

    static func key(_ name: String, _ size: CGSize) -> String { "\(name)@\(Int(size.width))x\(Int(size.height))" }
    func cached(_ key: String) -> UIImage? { cache.object(forKey: key as NSString) }

    /// Decodes `name` so it aspect-fills `size` points; calls back on the main thread (or never if prefetch).
    func load(_ name: String, size: CGSize, done: ((UIImage?) -> Void)? = nil) {
        let key = Self.key(name, size)
        if let hit = cached(key) { done?(hit); return }
        if waiting[key] != nil { if let done { waiting[key]!.append(done) }; return }
        waiting[key] = done.map { [$0] } ?? []
        let px = CGSize(width: size.width * scale, height: size.height * scale)
        queue.async {
            var out: UIImage?
            if let path = Bundle.main.path(forResource: name, ofType: nil), let full = UIImage(contentsOfFile: path) {
                let iw = full.size.width * full.scale, ih = full.size.height * full.scale
                let fill = min(max(px.width / iw, px.height / ih), 1)
                out = full.preparingThumbnail(of: CGSize(width: ceil(iw * fill), height: ceil(ih * fill)))
            }
            DispatchQueue.main.async {
                if let out { self.cache.setObject(out, forKey: key as NSString) }
                let cbs = self.waiting.removeValue(forKey: key) ?? []
                for cb in cbs { cb(out) }
            }
        }
    }
}

/// An aspect-fill image view with the #E5E5EA placeholder that shows `name` decoded for its size.
final class BundleImageView: UIImageView {
    private var want: String?
    override init(frame: CGRect) {
        super.init(frame: frame)
        contentMode = .scaleAspectFill; clipsToBounds = true; backgroundColor = .hairline
    }
    convenience init() { self.init(frame: .zero) }
    required init?(coder: NSCoder) { fatalError() }
    func show(_ name: String, size: CGSize) {
        let key = ImageLoader.key(name, size)
        if want == key { return }
        want = key
        if let hit = ImageLoader.shared.cached(key) { image = hit; return }
        image = nil
        ImageLoader.shared.load(name, size: size) { [weak self] img in
            guard let self, self.want == key else { return }
            self.image = img
        }
    }
}

// MARK: - Cell

final class ChipView: UIControl {
    let emoji = UILabel(), count = UILabel()
    var index = 0
    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .fill; layer.cornerRadius = 14
        emoji.font = Fonts.emoji
        count.font = Fonts.count; count.textColor = .label2
        addSubview(emoji); addSubview(count)
    }
    required init?(coder: NSCoder) { fatalError() }
    func set(_ r: Reaction) {
        emoji.text = r.emoji; count.text = "\(r.count)"
        let ew = textWidth(r.emoji, Fonts.emoji), cw = textWidth("\(r.count)", Fonts.count)
        emoji.frame = CGRect(x: 10, y: 0, width: ew, height: 28)
        count.frame = CGRect(x: 10 + ew + 4, y: 0, width: cw, height: 28)
    }
}

final class MessageCell: UICollectionViewCell {
    static let id = "m"
    let avatar = BundleImageView()
    let author = UILabel(), time = UILabel()
    let quoteBox = UIView(), quoteBar = UIView(), quoteAuthor = UILabel(), quoteText = UILabel()
    var paragraphs: [UILabel] = []
    let photoBox = UIView()
    var photos: [BundleImageView] = []
    let linkBox = UIView(), linkThumb = BundleImageView(), linkSite = UILabel(), linkTitle = UILabel(), linkDesc = UILabel()
    var chips: [ChipView] = []
    let separator = UIView()
    var layout: RowLayout?
    var onChip: ((Int) -> Void)?

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .white; contentView.backgroundColor = .white
        let c = contentView
        avatar.layer.cornerRadius = 20
        c.addSubview(avatar)
        author.font = Fonts.author; author.textColor = .black
        time.font = Fonts.time; time.textColor = .secondaryGray
        c.addSubview(author); c.addSubview(time)
        quoteBox.backgroundColor = .fill; quoteBox.layer.cornerRadius = 8; quoteBox.clipsToBounds = true
        quoteBar.backgroundColor = .quoteBar
        quoteAuthor.font = Fonts.quoteAuthor; quoteAuthor.textColor = .label2
        quoteText.font = Fonts.quoteText; quoteText.textColor = .label2; quoteText.numberOfLines = 2
        quoteText.lineBreakMode = .byTruncatingTail
        quoteBox.addSubview(quoteBar); quoteBox.addSubview(quoteAuthor); quoteBox.addSubview(quoteText)
        c.addSubview(quoteBox)
        photoBox.layer.cornerRadius = 12; photoBox.clipsToBounds = true
        c.addSubview(photoBox)
        linkBox.layer.cornerRadius = 12; linkBox.clipsToBounds = true
        linkBox.layer.borderWidth = 0.5; linkBox.layer.borderColor = UIColor.cardBorder.cgColor
        linkSite.font = Fonts.site; linkSite.textColor = .secondaryGray
        linkTitle.font = Fonts.linkTitle; linkTitle.textColor = .black; linkTitle.numberOfLines = 2
        linkDesc.font = Fonts.linkDesc; linkDesc.textColor = .label2; linkDesc.numberOfLines = 2
        for v in [linkThumb, linkSite, linkTitle, linkDesc] as [UIView] { linkBox.addSubview(v) }
        c.addSubview(linkBox)
        separator.backgroundColor = .hairline
        c.addSubview(separator)
        isAccessibilityElement = true
    }
    required init?(coder: NSCoder) { fatalError() }

    func configure(_ m: Message, layout l: RowLayout, time t: String) {
        layout = l
        avatar.show(m.avatar, size: l.avatar.size)
        author.text = m.author
        time.text = t
        accessibilityLabel = "\(m.author), \(t)"
        if let q = m.quote {
            quoteBox.isHidden = false
            quoteAuthor.text = q.author; quoteText.text = q.excerpt
        } else { quoteBox.isHidden = true }
        while paragraphs.count < l.paragraphs.count {
            let p = UILabel(); p.numberOfLines = 0; contentView.addSubview(p); paragraphs.append(p)
        }
        for (i, p) in paragraphs.enumerated() {
            if i < l.paragraphs.count { p.isHidden = false; p.attributedText = l.paragraphs[i].0 } else { p.isHidden = true }
        }
        if let ps = m.photos, !ps.isEmpty {
            photoBox.isHidden = false
            while photos.count < ps.count { let v = BundleImageView(); photoBox.addSubview(v); photos.append(v) }
            for (i, v) in photos.enumerated() {
                if i < ps.count { v.isHidden = false; v.show(ps[i].src, size: l.photoFrames[i].size) } else { v.isHidden = true }
            }
        } else { photoBox.isHidden = true }
        if let k = m.link, let box = l.linkBox {
            linkBox.isHidden = false
            linkThumb.show(k.thumb, size: CGSize(width: box.width, height: 140))
            linkSite.text = k.site.uppercased(); linkTitle.text = k.title; linkDesc.text = k.description
        } else { linkBox.isHidden = true }
        let rs = m.reactions ?? []
        while chips.count < rs.count {
            let v = ChipView(); v.index = chips.count
            v.addTarget(self, action: #selector(chipTapped(_:)), for: .touchUpInside)
            contentView.addSubview(v); chips.append(v)
        }
        for (i, v) in chips.enumerated() {
            if i < rs.count { v.isHidden = false; v.set(rs[i]) } else { v.isHidden = true }
        }
        setNeedsLayout()
    }

    @objc private func chipTapped(_ v: ChipView) { onChip?(v.index) }

    func setTime(_ t: String, author a: String) { time.text = t; accessibilityLabel = "\(a), \(t)" }

    override func layoutSubviews() {
        super.layoutSubviews()
        guard let l = layout else { return }
        author.frame = l.authorFrame
        time.frame = CGRect(origin: l.timeOrigin, size: CGSize(width: l.timeMaxWidth, height: ceil(Fonts.time.lineHeight)))
        if let q = l.quoteBox {
            quoteBox.frame = q
            quoteBar.frame = CGRect(x: 0, y: 0, width: 3, height: q.height)
            quoteAuthor.frame = l.quoteAuthor; quoteText.frame = l.quoteText
        }
        for (i, p) in l.paragraphs.enumerated() { paragraphs[i].frame = p.1 }
        if let b = l.photoBox {
            photoBox.frame = b
            for (i, f) in l.photoFrames.enumerated() { photos[i].frame = f }
        }
        if let b = l.linkBox {
            linkBox.frame = b
            linkThumb.frame = CGRect(x: 0, y: 0, width: b.width, height: 140)
            linkSite.frame = l.linkSite; linkTitle.frame = l.linkTitle; linkDesc.frame = l.linkDesc
        }
        for (i, f) in l.chips.enumerated() { chips[i].frame = f }
        avatar.frame = l.avatar
        let px = 1 / (window?.screen.scale ?? UIScreen.main.scale)
        separator.frame = CGRect(x: 68, y: bounds.height - px, width: bounds.width - 68 - 16, height: px)
    }

    override func preferredLayoutAttributesFitting(_ a: UICollectionViewLayoutAttributes) -> UICollectionViewLayoutAttributes {
        if let l = layout { a.size.height = l.height }
        return a
    }
}

// MARK: - Screen

/// The list gives UIKit's focus system nothing to search: its cells are not focus items (`allowsFocus = false`)
/// and it answers focus searches with no items, so an iPad with a hardware keyboard does not walk every visible
/// cell's subtree on each focus update while scrolling (the cryptobench UIKit app traced it at 79 % of its late
/// frames; exact2's host does the same, `FocusSearchIOS.swift`). Taps, selection and first responders are unaffected.
final class FeedCollectionView: UICollectionView {
    override func focusItems(in rect: CGRect) -> [any UIFocusItem] { [] }
}


final class FeedViewController: UIViewController, UICollectionViewDelegate, UICollectionViewDataSourcePrefetching {
    /// Messages by diffable id (`key`): originals 0..<10,000 in data order, live inserts 1,000,000 + k.
    private var byKey: [Int: Message] = [:]
    private var originalCount = 0
    private var seconds = 0
    private var inserted = 0
    private var collection: UICollectionView!
    private var dataSource: UICollectionViewDiffableDataSource<Int, Int>!
    private let prefetchQueue = DispatchQueue(label: "layout", qos: .userInitiated)
    private var width: CGFloat = 0

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .white
        overrideUserInterfaceStyle = .light
        timingLog("viewDidLoad")
        let tl0 = CACurrentMediaTime()
        var loaded = loadMessages()
        timingLog(String(format: "decode %.1f ms", (CACurrentMediaTime() - tl0) * 1000))
        for i in loaded.indices { loaded[i].key = i; byKey[i] = loaded[i] }
        originalCount = loaded.count

        let bar = UIView(); bar.backgroundColor = .white
        let title = UILabel(); title.text = "Heavy list"; title.font = .systemFont(ofSize: 17, weight: .semibold)
        let live = UILabel(); live.text = liveMode ? "Live: on" : "Live: off"; live.font = .systemFont(ofSize: 13)
        live.textColor = .secondaryGray
        let divider = UIView(); divider.backgroundColor = .separator
        for v in [bar, title, live, divider] { v.translatesAutoresizingMaskIntoConstraints = false }
        view.addSubview(bar); bar.addSubview(title); bar.addSubview(live); view.addSubview(divider)

        let item = NSCollectionLayoutItem(layoutSize: .init(widthDimension: .fractionalWidth(1), heightDimension: .estimated(330)))
        let group = NSCollectionLayoutGroup.vertical(layoutSize: .init(widthDimension: .fractionalWidth(1), heightDimension: .estimated(330)), subitems: [item])
        let layout = UICollectionViewCompositionalLayout(section: NSCollectionLayoutSection(group: group))
        collection = FeedCollectionView(frame: .zero, collectionViewLayout: layout)
        collection.allowsFocus = false
        collection.translatesAutoresizingMaskIntoConstraints = false
        collection.backgroundColor = .white
        collection.delegate = self
        collection.prefetchDataSource = self
        collection.register(MessageCell.self, forCellWithReuseIdentifier: MessageCell.id)
        view.addSubview(collection)
        NSLayoutConstraint.activate([
            bar.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            bar.leadingAnchor.constraint(equalTo: view.leadingAnchor), bar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            title.leadingAnchor.constraint(equalTo: bar.leadingAnchor, constant: 16),
            title.topAnchor.constraint(equalTo: bar.topAnchor, constant: 10),
            title.bottomAnchor.constraint(equalTo: bar.bottomAnchor, constant: -10),
            live.trailingAnchor.constraint(equalTo: bar.trailingAnchor, constant: -16),
            live.centerYAnchor.constraint(equalTo: title.centerYAnchor),
            divider.topAnchor.constraint(equalTo: bar.bottomAnchor),
            divider.leadingAnchor.constraint(equalTo: view.leadingAnchor), divider.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            divider.heightAnchor.constraint(equalToConstant: 1 / UIScreen.main.scale),
            collection.topAnchor.constraint(equalTo: divider.bottomAnchor),
            collection.leadingAnchor.constraint(equalTo: view.leadingAnchor), collection.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            collection.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])

        dataSource = UICollectionViewDiffableDataSource<Int, Int>(collectionView: collection) { [unowned self] cv, ip, id in
            let cell = cv.dequeueReusableCell(withReuseIdentifier: MessageCell.id, for: ip) as! MessageCell
            let m = self.byKey[id]!
            cell.configure(m, layout: LayoutCache.shared.layout(m, self.rowWidth(cv)), time: relativeTime(m, seconds: self.seconds))
            cell.onChip = { [unowned self] i in self.bump(id: id, reaction: i) }
            return cell
        }
        // Sections of `chunk` rows (no headers, invisible): the compositional layout re-solves a
        // self-sized item's section and shifts later sections as units, instead of re-solving every
        // row after it in one 10,000-item section. BENCH_CHUNK=0 = one section (diagnostics).
        var snap = NSDiffableDataSourceSnapshot<Int, Int>()
        let chunk = env["BENCH_CHUNK"].flatMap(Int.init) ?? 500
        let keys = Array(0..<originalCount)
        if chunk <= 0 { snap.appendSections([0]); snap.appendItems(keys, toSection: 0) } else {
            for (si, start) in stride(from: 0, to: keys.count, by: chunk).enumerated() {
                snap.appendSections([si]); snap.appendItems(Array(keys[start..<min(start + chunk, keys.count)]), toSection: si)
            }
        }
        let ts0 = CACurrentMediaTime()
        dataSource.applySnapshotUsingReloadData(snap)
        timingLog(String(format: "snapshot %.1f ms", (CACurrentMediaTime() - ts0) * 1000))

        if liveMode {
            var ticks = 0
            Timer.scheduledTimer(withTimeInterval: 0.25, repeats: true) { [unowned self] _ in
                ticks += 1
                self.liveStep()
                if ticks % 4 == 0 { self.seconds += 1; self.refreshTimes() }
            }
        }
    }

    private var didStart = false
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        guard !didStart, collection.bounds.width > 0 else { return }
        if env["BENCH_TIMING"] == "1" { let t0 = CACurrentMediaTime(); collection.layoutIfNeeded(); timingLog(String(format: "first layout %.1f ms", (CACurrentMediaTime() - t0) * 1000)) }
        didStart = true
        if let n = startIndex, n >= 0, n < originalCount {
            // estimated heights above the target settle as rows are measured: repeat, as SwiftUI's app does
            for k in 0..<3 {
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.25 * Double(k)) {
                    if let ip = self.dataSource.indexPath(for: n) { self.collection.scrollToItem(at: ip, at: .top, animated: false) }
                }
            }
        }
    }

    private func rowWidth(_ cv: UICollectionView) -> CGFloat { cv.bounds.width }

    // Prefetch: build row layouts off the main thread, start image decodes.
    func collectionView(_ cv: UICollectionView, prefetchItemsAt ips: [IndexPath]) {
        let w = rowWidth(cv)
        let ms = ips.compactMap { dataSource.itemIdentifier(for: $0).flatMap { byKey[$0] } }
        for m in ms {
            ImageLoader.shared.load(m.avatar, size: CGSize(width: 40, height: 40))
        }
        prefetchQueue.async {
            let ls = ms.map { LayoutCache.shared.layout($0, w) }
            DispatchQueue.main.async {
                for (m, l) in zip(ms, ls) {
                    if let ps = m.photos { for (i, p) in ps.enumerated() { ImageLoader.shared.load(p.src, size: l.photoFrames[i].size) } }
                    if let k = m.link, let b = l.linkBox { ImageLoader.shared.load(k.thumb, size: CGSize(width: b.width, height: 140)) }
                }
            }
        }
    }

    func collectionView(_ cv: UICollectionView, shouldHighlightItemAt ip: IndexPath) -> Bool { false }

    // MARK: live mode

    private func refreshTimes() {
        for case let cell as MessageCell in collection.visibleCells {
            guard let ip = collection.indexPath(for: cell), let id = dataSource.itemIdentifier(for: ip), let m = byKey[id] else { continue }
            cell.setTime(relativeTime(m, seconds: seconds), author: m.author)
        }
    }

    private func bump(id: Int, reaction i: Int) {
        bump(key: id, reaction: i)
        var snap = dataSource.snapshot(); snap.reconfigureItems([id]); dataSource.apply(snap, animatingDifferences: false)
    }

    private func bump(key: Int, reaction i: Int) {
        guard byKey[key]?.reactions != nil else { return }
        byKey[key]!.reactions![i].count += 1
        byKey[key]!.rev = (byKey[key]!.rev ?? 0) + 1
    }

    /// One 250 ms step: insert at the top keeping what is on screen still, and bump one reaction.
    private func liveStep() {
        let k = inserted + 1
        inserted = k
        var copy = byKey[(k * 37) % 10_000]!              // original message (k*37) % 10000
        copy.id = "live-\(k)"; copy.index = -k; copy.minutesAgo = 0; copy.bornAt = seconds; copy.rev = nil; copy.key = 1_000_000 + k
        byKey[copy.key!] = copy
        // Bump: original (k*101) % 10000, or the next original message that has reactions.
        var j = (k * 101) % 10_000
        while j < originalCount, (byKey[j]!.reactions ?? []).isEmpty { j += 1 }
        var reconfigure: [Int] = []
        if j < originalCount, let n = byKey[j]!.reactions?.count {
            bump(key: j, reaction: k % n)
            // Only an on-screen row needs its cell reconfigured; an off-screen one picks up the new
            // layout (keyed by rev) when it is next dequeued. A reconfigure costs a layout pass.
            if collection.indexPathsForVisibleItems.contains(where: { dataSource.itemIdentifier(for: $0) == j }) { reconfigure.append(j) }
        }
        // anchor: the first visible item keeps its on-screen position across the insert above it
        let anchorIP = collection.indexPathsForVisibleItems.min()
        let anchor = anchorIP.flatMap { dataSource.itemIdentifier(for: $0) }
        let before = anchorIP.flatMap { collection.layoutAttributesForItem(at: $0)?.frame.minY }
        let offsetBefore = collection.contentOffset.y
        let t0 = CACurrentMediaTime()
        var snap = dataSource.snapshot()
        if let first = snap.itemIdentifiers(inSection: snap.sectionIdentifiers[0]).first { snap.insertItems([copy.key!], beforeItem: first) }
        else { snap.appendItems([copy.key!], toSection: snap.sectionIdentifiers[0]) }
        if !reconfigure.isEmpty { snap.reconfigureItems(reconfigure) }
        let t1 = CACurrentMediaTime()
        dataSource.apply(snap, animatingDifferences: false)
        let t2 = CACurrentMediaTime()
        defer { if env["BENCH_TIMING"] == "1" { NSLog("live snap %.1f apply %.1f layout %.1f", (t1-t0)*1000, (t2-t1)*1000, (CACurrentMediaTime()-t2)*1000) } }
        if let anchor, let before, let ip = dataSource.indexPath(for: anchor) {
            collection.layoutIfNeeded()
            if let after = collection.layoutAttributesForItem(at: ip)?.frame.minY {
                let want = after - (before - offsetBefore)
                if collection.contentOffset.y != want { collection.contentOffset.y = want }
            }
        }
    }
}

@main
final class AppDelegate: UIResponder, UIApplicationDelegate {
    func application(_ application: UIApplication, didFinishLaunchingWithOptions o: [UIApplication.LaunchOptionsKey: Any]?) -> Bool { true }
}

@objc(SceneDelegate)
final class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?
    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options: UIScene.ConnectionOptions) {
        guard let ws = scene as? UIWindowScene else { return }
        let w = UIWindow(windowScene: ws)
        w.rootViewController = FeedViewController()
        w.makeKeyAndVisible()
        window = w
    }
}
