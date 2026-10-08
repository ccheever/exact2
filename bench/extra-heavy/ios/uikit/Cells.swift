// The feed cell (header, separator, manual layout, self-sizing) and the media-heavy row kinds.
import Lottie
import MapKit
import UIKit

func rounded(_ v: UIView, _ r: CGFloat) {
    v.layer.cornerRadius = r
    v.layer.cornerCurve = .continuous
    v.clipsToBounds = true
}

class FeedCell: UICollectionViewCell {
    private(set) var row: Row!
    let avatar = LoadingImageView()
    let author = label(sys(15, .semibold))
    let sub = label(sys(13), .secondaryGray)
    let kindLabel = label(sys(11, .semibold), .secondaryGray)
    private let sep = CALayer()
    private var laidOut: (id: String, w: CGFloat, h: CGFloat)?
    private(set) var visible = false

    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .white
        contentView.backgroundColor = .white
        avatar.layer.cornerRadius = 18
        for v in [avatar, author, sub, kindLabel] as [UIView] { contentView.addSubview(v) }
        sep.backgroundColor = UIColor.hairline.cgColor
        layer.addSublayer(sep)
        setup()
    }
    required init?(coder: NSCoder) { fatalError() }

    /// Subclasses add their body views here.
    func setup() {}
    /// Subclasses fill their body from the row.
    func configureBody(_ r: Row) {}
    /// Subclasses lay out their body at (x, y) in column width C; return its height.
    func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat { 0 }
    /// The 1 Hz live clock ticked (SPEC "Clocks"); visible cells only.
    func tickBody(in cv: UICollectionView) {}
    func visibilityChanged() {}

    final func configure(_ r: Row) {
        row = r
        laidOut = nil
        author.text = r.author
        sub.text = "\(r.handle) · \(relativeTime(r.minutesAgo, s: AppState.seconds))"
        kindLabel.text = r.kind.uppercased()
        avatar.set(r.avatar, CGSize(width: 36, height: 36))
        configureBody(r)
        setNeedsLayout()
    }

    final func tick(in cv: UICollectionView) {
        sub.text = "\(row.handle) · \(relativeTime(row.minutesAgo, s: AppState.seconds))"
        tickBody(in: cv)
    }

    final func setVisible(_ v: Bool) {
        guard v != visible else { return }
        visible = v
        visibilityChanged()
    }

    /// Marks the layout stale (a height change); the next layout pass re-measures.
    final func relayout() { laidOut = nil }

    @discardableResult
    final func layoutAll(width W: CGFloat) -> CGFloat {
        if let l = laidOut, l.id == row?.id, l.w == W { return l.h }
        guard let row else { return 0 }
        let C = min(W - 32, 600)
        let x = ((W - C) / 2).rounded()
        // Header: 36 tall; avatar · 10 · column (author, 2, handle · rel) · space · KIND
        avatar.frame = CGRect(x: x, y: 16, width: 36, height: 36)
        let kw = ceil(kindLabel.sizeThatFits(.zero).width)
        let kh = ceil(kindLabel.font.lineHeight)
        kindLabel.frame = CGRect(x: x + C - kw, y: 16 + (36 - kh) / 2, width: kw, height: kh)
        let cx = x + 46
        let cw = max(0, kindLabel.frame.minX - 8 - cx)
        let ah = ceil(author.font.lineHeight), sh = ceil(sub.font.lineHeight)
        let top = 16 + (36 - (ah + 2 + sh)) / 2
        author.frame = CGRect(x: cx, y: top, width: cw, height: ah)
        sub.frame = CGRect(x: cx, y: top + ah + 2, width: cw, height: sh)
        let bh = layoutBody(x: x, y: 16 + 36 + 10, C: C)
        let h = ceil(16 + 36 + 10 + bh + 16)
        laidOut = (row.id, W, h)
        return h
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        layoutAll(width: bounds.width)
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        sep.frame = CGRect(x: 0, y: bounds.height - 0.5, width: bounds.width, height: 0.5)
        CATransaction.commit()
    }

    override func preferredLayoutAttributesFitting(_ la: UICollectionViewLayoutAttributes) -> UICollectionViewLayoutAttributes {
        let a = la.copy() as! UICollectionViewLayoutAttributes
        a.size.height = layoutAll(width: la.size.width)
        return a
    }
}

/// A caption (15, wraps) laid out at the body's top; returns the y after it (+10) or y if absent.
func layoutCaption(_ l: UILabel, x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
    guard let t = l.text, !t.isEmpty else { l.frame = .zero; return y }
    let h = fit(l, C)
    l.frame = CGRect(x: x, y: y, width: C, height: h)
    return y + h + 10
}

// MARK: 1 photo

final class PhotoCell: FeedCell {
    let caption = label(sys(15), lines: 0)
    let photo = LoadingImageView()
    static func photoHeight(_ p: Photo, _ C: CGFloat) -> CGFloat { min(C * p.h / p.w, 1.25 * C) }
    override func setup() {
        photo.layer.cornerRadius = 12
        contentView.addSubview(caption)
        contentView.addSubview(photo)
    }
    override func configureBody(_ r: Row) {
        caption.text = r.caption
        photo.set(r.photo!.src, CGSize(width: Geo.C, height: PhotoCell.photoHeight(r.photo!, Geo.C)))
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let y1 = layoutCaption(caption, x: x, y: y, C: C)
        let h = PhotoCell.photoHeight(row.photo!, C)
        photo.frame = CGRect(x: x, y: y1, width: C, height: h)
        return y1 + h - y
    }
}

// MARK: 2 thumbs (skeleton shimmer for 1.2 s each time the row appears, then a 200 ms fade)

final class ThumbsCell: FeedCell {
    let title = label(sys(15, .semibold))
    let grid = UIView()
    var cells: [LoadingImageView] = []
    let skeleton = UIView()
    let skeletonMask = CAShapeLayer()
    let band = CAGradientLayer()
    private var epoch = 0

    override func setup() {
        contentView.addSubview(title)
        contentView.addSubview(grid)
        for _ in 0..<12 {
            let v = LoadingImageView()
            v.layer.cornerRadius = 8
            grid.addSubview(v)
            cells.append(v)
        }
        skeleton.backgroundColor = .hairline
        skeleton.layer.mask = skeletonMask
        band.colors = [UIColor(white: 1, alpha: 0).cgColor, UIColor(white: 1, alpha: 0.65).cgColor, UIColor(white: 1, alpha: 0).cgColor]
        band.startPoint = CGPoint(x: 0, y: 0.5)
        band.endPoint = CGPoint(x: 1, y: 0.5)
        skeleton.layer.addSublayer(band)
        contentView.addSubview(skeleton)
    }
    override func configureBody(_ r: Row) {
        title.text = r.title
        let side = (Geo.C - 12) / 4
        for (i, v) in cells.enumerated() { v.set(r.thumbs![i], CGSize(width: side, height: side)) }
        showSkeleton(!freeze)
    }
    private func showSkeleton(_ on: Bool) {
        epoch += 1
        skeleton.isHidden = !on
        grid.alpha = on ? 0 : 1
        if !on { band.removeAllAnimations() }
    }
    override func visibilityChanged() {
        guard !freeze else { return }
        if visible {
            // Each appearance: 1.2 s of skeleton + shimmer, then the images fade in over 200 ms.
            showSkeleton(true)
            startShimmer()
            let e = epoch
            DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) { [weak self] in
                guard let self, self.epoch == e else { return }
                self.skeleton.isHidden = true
                self.band.removeAllAnimations()
                UIView.animate(withDuration: 0.2, delay: 0, options: [.curveLinear]) { self.grid.alpha = 1 }
            }
        } else {
            epoch += 1
            band.removeAllAnimations()
        }
    }
    private func startShimmer() {
        let C = skeleton.bounds.width
        guard C > 0 else { return }
        let a = CABasicAnimation(keyPath: "position.x")
        a.fromValue = -0.4 * C + 0.2 * C
        a.toValue = C + 0.2 * C
        a.duration = 1
        a.repeatCount = .infinity
        band.add(a, forKey: "shimmer")
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let th = ceil(title.font.lineHeight)
        title.frame = CGRect(x: x, y: y, width: C, height: th)
        let side = (C - 12) / 4
        let gh = side * 3 + 8
        let gy = y + th + 10
        grid.frame = CGRect(x: x, y: gy, width: C, height: gh)
        skeleton.frame = grid.frame
        let path = UIBezierPath()
        for i in 0..<12 {
            let f = CGRect(x: CGFloat(i % 4) * (side + 4), y: CGFloat(i / 4) * (side + 4), width: side, height: side)
            cells[i].frame = f
            path.append(UIBezierPath(roundedRect: f, cornerRadius: 8))
        }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        skeletonMask.path = path.cgPath
        band.bounds = CGRect(x: 0, y: 0, width: 0.4 * C, height: gh)
        band.position = CGPoint(x: -0.2 * C, y: gh / 2)
        CATransaction.commit()
        if visible && !skeleton.isHidden && band.animation(forKey: "shimmer") == nil { startShimmer() }
        return th + 10 + gh
    }
}

// MARK: 4 canvas (Core Graphics, drawn when the row is configured)

final class CanvasView: UIView {
    var row: Row?
    var image: UIImage?
    override func draw(_ rect: CGRect) {
        guard let row, let ctx = UIGraphicsGetCurrentContext() else { return }
        let W = bounds.width, H = bounds.height
        // 1. the band: rounded rect, left→right gradient
        ctx.saveGState()
        let band = CGRect(x: 16, y: H - 60, width: 0.4 * W, height: 44)
        ctx.addPath(UIBezierPath(roundedRect: band, cornerRadius: 10).cgPath)
        ctx.clip()
        let cols = row.band!.map { UIColor(hexString: $0).cgColor } as CFArray
        if let g = CGGradient(colorsSpace: CGColorSpace(name: CGColorSpace.sRGB), colors: cols, locations: [0, 1]) {
            ctx.drawLinearGradient(g, start: CGPoint(x: 16, y: 0), end: CGPoint(x: 16 + 0.4 * W, y: 0), options: [])
        }
        ctx.restoreGState()
        // 2. the strokes
        ctx.setLineCap(.round)
        ctx.setLineJoin(.round)
        for s in row.strokes! {
            let p = s.p
            ctx.beginPath()
            ctx.move(to: CGPoint(x: p[0] * W, y: p[1] * H))
            ctx.addCurve(to: CGPoint(x: p[6] * W, y: p[7] * H), control1: CGPoint(x: p[2] * W, y: p[3] * H),
                         control2: CGPoint(x: p[4] * W, y: p[5] * H))
            ctx.setStrokeColor(UIColor(hexString: s.color).cgColor)
            ctx.setLineWidth(s.width)
            ctx.strokePath()
        }
        // 3. the dots at 60 %
        for d in row.dots! {
            ctx.setFillColor(UIColor(hexString: d.color).withAlphaComponent(0.6).cgColor)
            ctx.fillEllipse(in: CGRect(x: d.x * W - d.r, y: d.y * H - d.r, width: d.r * 2, height: d.r * 2))
        }
        // 4. the image, clipped to a circle
        if let image {
            ctx.saveGState()
            let r = CGRect(x: W - 80, y: 16, width: 64, height: 64)
            ctx.addEllipse(in: r)
            ctx.clip()
            let s = image.size, k = max(64 / s.width, 64 / s.height)
            image.draw(in: CGRect(x: r.midX - s.width * k / 2, y: r.midY - s.height * k / 2, width: s.width * k, height: s.height * k))
            ctx.restoreGState()
        }
        // 5. the title
        (row.title ?? "" as NSString as String).draw(at: CGPoint(x: 16, y: 16),
            withAttributes: [.font: sys(20, .semibold), .foregroundColor: UIColor.ink])
    }
}

final class CanvasCell: FeedCell {
    let canvas = CanvasView()
    private var token = 0
    override func setup() {
        rounded(canvas, 12)
        canvas.isOpaque = true
        canvas.contentMode = .redraw
        contentView.addSubview(canvas)
    }
    override func configureBody(_ r: Row) {
        canvas.row = r
        canvas.backgroundColor = UIColor(hexString: r.bg!)
        ImageLoader.shared.cancel(token)
        let name = r.image!, id = r.id
        canvas.image = ImageLoader.shared.cached(name, CGSize(width: 64, height: 64))
        canvas.setNeedsDisplay()
        if canvas.image == nil {
            token = ImageLoader.shared.load(name, CGSize(width: 64, height: 64)) { [weak self] img in
                guard let self, self.row?.id == id else { return }
                self.canvas.image = img
                self.canvas.setNeedsDisplay()
            }
        }
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        canvas.frame = CGRect(x: x, y: y, width: C, height: 220)
        return 220
    }
}

// MARK: 5 svg (asset-catalog SVGs, vector data preserved, rendered at display size)

final class SvgCell: FeedCell {
    var icons: [UIImageView] = []
    let chart = UIImageView()
    let art = UIImageView()
    override func setup() {
        for _ in 0..<6 { let v = UIImageView(); icons.append(v); contentView.addSubview(v) }
        rounded(art, 12)
        contentView.addSubview(chart)
        contentView.addSubview(art)
    }
    override func configureBody(_ r: Row) {
        for (i, v) in icons.enumerated() { v.image = i < r.icons!.count ? UIImage(named: "icon-\(r.icons![i])") : nil }
        chart.image = UIImage(named: r.chart!)
        art.image = UIImage(named: r.art!)
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        for (i, v) in icons.enumerated() { v.frame = CGRect(x: x + CGFloat(i) * 44, y: y, width: 28, height: 28) }
        let ch = C * 160 / 600
        chart.frame = CGRect(x: x, y: y + 40, width: C, height: ch)
        art.frame = CGRect(x: x, y: y + 40 + ch + 12, width: C, height: C / 2)
        return 40 + ch + 12 + C / 2
    }
}

// MARK: 6 video

final class VideoCell: FeedCell {
    let caption = label(sys(15), lines: 0)
    let player = PlayerView()
    let pill = UIView()
    let pillText = label(sys(12, .semibold), .white)
    override func setup() {
        rounded(player, 12)
        pill.backgroundColor = UIColor(white: 0, alpha: 0.55)
        rounded(pill, 6)
        pillText.text = "0:04"
        pill.addSubview(pillText)
        contentView.addSubview(caption)
        contentView.addSubview(player)
        contentView.addSubview(pill)
    }
    override func configureBody(_ r: Row) {
        caption.text = r.caption
        player.load(r.video!)
    }
    override func visibilityChanged() { player.setVisible(visible) }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let y1 = layoutCaption(caption, x: x, y: y, C: C)
        let h = (C * 9 / 16).rounded()
        player.frame = CGRect(x: x, y: y1, width: C, height: h)
        let s = pillText.sizeThatFits(.zero)
        let pw = ceil(s.width) + 14, ph = ceil(s.height) + 6
        pill.frame = CGRect(x: x + 8, y: y1 + h - 8 - ph, width: pw, height: ph)
        pillText.frame = CGRect(x: 7, y: 3, width: ceil(s.width), height: ceil(s.height))
        return y1 + h - y
    }
}

// MARK: 7 map (MKMapView, interaction off, the default marker)

final class MapCell: FeedCell {
    let place = label(sys(15, .semibold))
    let address = label(sys(13), .secondaryGray)
    let map = MKMapView()
    let pin = MKPointAnnotation()
    override func setup() {
        rounded(map, 12)
        map.isUserInteractionEnabled = false
        map.isZoomEnabled = false
        map.isScrollEnabled = false
        map.isRotateEnabled = false
        map.isPitchEnabled = false
        map.preferredConfiguration = MKStandardMapConfiguration()
        map.addAnnotation(pin)
        contentView.addSubview(place)
        contentView.addSubview(address)
        contentView.addSubview(map)
    }
    override func configureBody(_ r: Row) {
        place.text = r.place
        address.text = r.address
        let c = CLLocationCoordinate2D(latitude: r.lat!, longitude: r.lon!)
        map.removeAnnotation(pin)
        pin.coordinate = c
        pin.title = r.place
        map.setRegion(MKCoordinateRegion(center: c, span: MKCoordinateSpan(latitudeDelta: 0.02, longitudeDelta: 0.02)), animated: false)
        map.addAnnotation(pin)
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let ph = ceil(place.font.lineHeight), ah = ceil(address.font.lineHeight)
        place.frame = CGRect(x: x, y: y, width: C, height: ph)
        address.frame = CGRect(x: x, y: y + ph + 2, width: C, height: ah)
        let my = y + ph + 2 + ah + 10
        map.frame = CGRect(x: x, y: my, width: C, height: 200)
        return my + 200 - y
    }
}

// MARK: 11 typeface

final class TypefaceCell: FeedCell {
    let card = UIView()
    let quote = label(sys(20), .ink, lines: 0)
    let by = label(sys(13), .label2, lines: 0)
    override func setup() {
        rounded(card, 12)
        card.addSubview(quote)
        card.addSubview(by)
        contentView.addSubview(card)
    }
    override func configureBody(_ r: Row) {
        let f = feed.fonts[r.font ?? ""]
        quote.font = UIFont(name: f?.name ?? "", size: f?.size ?? 20) ?? sys(20)
        quote.text = r.quote
        by.text = "— \(r.by ?? "") · \(f?.family ?? "")"
        card.backgroundColor = UIColor(hexString: r.bg!)
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let w = C - 40
        let qh = fit(quote, w), bh = fit(by, w)
        quote.frame = CGRect(x: 20, y: 20, width: w, height: qh)
        by.frame = CGRect(x: 20, y: 20 + qh + 10, width: w, height: bh)
        let h = 20 + qh + 10 + bh + 20
        card.frame = CGRect(x: x, y: y, width: C, height: h)
        return h
    }
}

// MARK: 10 intl (platform font fallback; RTL blocks base right-to-left, right-aligned)

final class IntlCell: FeedCell {
    var labels: [UILabel] = []
    override func configureBody(_ r: Row) {
        let blocks = r.blocks!
        while labels.count < blocks.count { let l = label(sys(17), lines: 0); labels.append(l); contentView.addSubview(l) }
        for (i, l) in labels.enumerated() {
            guard i < blocks.count else { l.isHidden = true; continue }
            l.isHidden = false
            let rtl = blocks[i].dir == "rtl"
            let ps = NSMutableParagraphStyle()
            ps.baseWritingDirection = rtl ? .rightToLeft : .leftToRight
            ps.alignment = rtl ? .right : .left
            l.attributedText = NSAttributedString(string: blocks[i].text,
                                                  attributes: [.font: sys(17), .foregroundColor: UIColor.black, .paragraphStyle: ps])
        }
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        var yy = y
        for l in labels where !l.isHidden {
            let h = fit(l, C)
            l.frame = CGRect(x: x, y: yy, width: C, height: h)
            yy += h + 8
        }
        return max(0, yy - 8 - y)
    }
}

// MARK: 13 motion (GIF, WebP, Lottie tiles)

final class MotionCell: FeedCell {
    let caption = label(sys(15), lines: 0)
    let gif = AnimatedImageView()
    let webp = AnimatedImageView()
    let lottie = LottieAnimationView()
    var tiles: [UIView] = []
    var tileLabels: [UILabel] = []
    override func setup() {
        contentView.addSubview(caption)
        for (v, name) in [(gif as UIView, "GIF"), (webp, "WebP"), (lottie, "Lottie")] {
            let tile = UIView()
            tile.backgroundColor = .xfill
            rounded(tile, 12)
            v.contentMode = .scaleAspectFit
            tile.addSubview(v)
            contentView.addSubview(tile)
            tiles.append(tile)
            let l = label(sys(11, .semibold), .secondaryGray)
            l.text = name
            l.textAlignment = .center
            contentView.addSubview(l)
            tileLabels.append(l)
        }
        lottie.loopMode = .loop
        lottie.backgroundBehavior = .pauseAndRestore
    }
    override func configureBody(_ r: Row) {
        caption.text = r.caption
        gif.load(r.gif!)
        webp.load(r.webp!)
        if lottie.animation !== lottieAnimation(r.lottie!) {
            lottie.animation = lottieAnimation(r.lottie!)
            if freeze { lottie.currentProgress = 0.5 } else if visible { lottie.play() }
        }
    }
    override func visibilityChanged() {
        gif.setVisible(visible)
        webp.setVisible(visible)
        if freeze { lottie.currentProgress = 0.5; return }
        if visible { lottie.play() } else { lottie.pause() }
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let y1 = layoutCaption(caption, x: x, y: y, C: C)
        let T = (C - 24) / 3
        let lh = ceil(tileLabels[0].font.lineHeight)
        for i in 0..<3 {
            let f = CGRect(x: x + CGFloat(i) * (T + 12), y: y1, width: T, height: T)
            tiles[i].frame = f
            tiles[i].subviews.first?.frame = tiles[i].bounds
            tileLabels[i].frame = CGRect(x: f.minX, y: f.maxY + 4, width: T, height: lh)
        }
        return y1 + T + 4 + lh - y
    }
}

// MARK: 14 glass (the system ultra-thin material over the photo)

final class GlassCell: FeedCell {
    let photo = LoadingImageView()
    let bar = UIVisualEffectView(effect: UIBlurEffect(style: .systemUltraThinMaterial))
    let pill = UIVisualEffectView(effect: UIBlurEffect(style: .systemUltraThinMaterial))
    let title = label(sys(16, .semibold))
    let subtitle = label(sys(13), .label2)
    let rating = label(sys(13, .semibold))
    override func setup() {
        photo.layer.cornerRadius = 16
        rounded(bar, 14)
        pill.clipsToBounds = true
        pill.layer.cornerCurve = .continuous
        bar.contentView.addSubview(title)
        bar.contentView.addSubview(subtitle)
        pill.contentView.addSubview(rating)
        contentView.addSubview(photo)
        photo.addSubview(bar)
        photo.addSubview(pill)
    }
    override func configureBody(_ r: Row) {
        title.text = r.title
        subtitle.text = r.subtitle
        rating.text = r.rating
        photo.set(r.photo!.src, CGSize(width: Geo.C, height: (Geo.C * 3 / 4).rounded()))
    }
    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let h = (C * 3 / 4).rounded()
        photo.frame = CGRect(x: x, y: y, width: C, height: h)
        bar.frame = CGRect(x: 12, y: h - 12 - 64, width: C - 24, height: 64)
        let th = ceil(title.font.lineHeight), sh = ceil(subtitle.font.lineHeight)
        let ty = (64 - th - 2 - sh) / 2
        title.frame = CGRect(x: 12, y: ty, width: C - 48, height: th)
        subtitle.frame = CGRect(x: 12, y: ty + th + 2, width: C - 48, height: sh)
        let rs = rating.sizeThatFits(.zero)
        let pw = ceil(rs.width) + 20, ph = ceil(rs.height) + 12
        pill.frame = CGRect(x: C - 12 - pw, y: 12, width: pw, height: ph)
        pill.layer.cornerRadius = ph / 2
        rating.frame = CGRect(x: 10, y: 6, width: ceil(rs.width), height: ceil(rs.height))
        return h
    }
}
