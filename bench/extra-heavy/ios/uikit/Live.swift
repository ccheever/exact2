// 15 live — countdown and "Updated" from the 1 Hz live clock; the rings and the waveform playhead on the
// motion clock as Core Animation keyframe animations (run by the render server, phase-locked to launch).
import UIKit

let ringColors = [UIColor(hex: 0xFF3B30), UIColor(hex: 0x34C759), UIColor(hex: 0x007AFF)]
func hms(_ v: Int) -> String { String(format: "%02d:%02d:%02d", v / 3600, v / 60 % 60, v % 60) }

/// Animations begin at launch, so every layer shows the motion clock's phase t mod period.
func launchBegin(_ layer: CALayer) -> CFTimeInterval { layer.convertTime(launchTime, from: nil) }

let fullFrameRate = CAFrameRateRange(minimum: 80, maximum: 120, preferred: 120)

final class LiveCell: FeedCell {
    let card = UIView()
    let title = label(sys(15, .semibold))
    let updated = label(sys(12), .secondaryGray)
    let endsIn = label(sys(15), .label2)
    let clock = label(UIFont.monospacedDigitSystemFont(ofSize: 22, weight: .semibold))
    var tracks: [CAShapeLayer] = []
    var arcs: [CAShapeLayer] = []
    var pcts: [UILabel] = []
    let ringsView = UIView()
    let wave = UIView()
    let play = CAShapeLayer()
    let tri = CAShapeLayer()
    let grayBars = CAShapeLayer()
    let blueBars = CAShapeLayer()
    let blueMask = CALayer()
    let time = label(UIFont.monospacedDigitSystemFont(ofSize: 12, weight: .regular), .secondaryGray)

    override func setup() {
        card.layer.borderWidth = 0.5
        card.layer.borderColor = UIColor.border.cgColor
        card.layer.cornerRadius = 12
        card.layer.cornerCurve = .continuous
        endsIn.text = "Ends in "
        for v in [title, updated, endsIn, clock, ringsView, wave, time] as [UIView] { card.addSubview(v) }
        contentView.addSubview(card)
        let ring = UIBezierPath(arcCenter: CGPoint(x: 28, y: 28), radius: 25, startAngle: -.pi / 2, endAngle: 1.5 * .pi, clockwise: true).cgPath
        for i in 0..<3 {
            let t = CAShapeLayer(), a = CAShapeLayer()
            for l in [t, a] { l.path = ring; l.fillColor = nil; l.lineWidth = 6; l.frame = CGRect(x: CGFloat(i) * 72, y: 0, width: 56, height: 56) }
            t.strokeColor = UIColor.hairline.cgColor
            a.strokeColor = ringColors[i].cgColor
            a.lineCap = .round
            ringsView.layer.addSublayer(t)
            ringsView.layer.addSublayer(a)
            tracks.append(t); arcs.append(a)
            let l = label(sys(12, .semibold))
            l.textAlignment = .center
            l.frame = CGRect(x: CGFloat(i) * 72, y: 0, width: 56, height: 56)
            ringsView.addSubview(l)
            pcts.append(l)
        }
        play.path = UIBezierPath(ovalIn: CGRect(x: 0, y: 4, width: 32, height: 32)).cgPath
        play.fillColor = UIColor.xblue.cgColor
        let tp = UIBezierPath()
        tp.move(to: CGPoint(x: 11, y: 14)); tp.addLine(to: CGPoint(x: 21, y: 20)); tp.addLine(to: CGPoint(x: 11, y: 26)); tp.close()
        tri.path = tp.cgPath
        tri.fillColor = UIColor.white.cgColor
        grayBars.fillColor = UIColor.gray3.cgColor
        blueBars.fillColor = UIColor.xblue.cgColor
        blueMask.backgroundColor = UIColor.black.cgColor
        blueMask.anchorPoint = CGPoint(x: 0, y: 0.5)
        blueBars.mask = blueMask
        for l in [play, tri, grayBars, blueBars] { wave.layer.addSublayer(l) }
    }

    override func configureBody(_ r: Row) {
        title.text = r.title
        for i in 0..<3 { pcts[i].text = "\(Int((r.rings![i] * 100).rounded()))%" }
        let bars = UIBezierPath()
        for (j, h) in r.wave!.enumerated() {
            bars.append(UIBezierPath(roundedRect: CGRect(x: CGFloat(j) * 5, y: (40 - h) / 2, width: 3, height: h), cornerRadius: 1.5))
        }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        grayBars.path = bars.cgPath
        blueBars.path = bars.cgPath
        grayBars.frame = CGRect(x: 42, y: 0, width: 238, height: 40)
        blueBars.frame = grayBars.frame
        blueMask.bounds = CGRect(x: 0, y: 0, width: 5 * 20 - 1, height: 40)  // frozen q = 0.4: bars j < 19.2
        blueMask.position = CGPoint(x: 0, y: 20)
        for i in 0..<3 { arcs[i].strokeEnd = r.rings![i] }
        CATransaction.commit()
        updateClockText()
        startMotion()
    }

    private func updateClockText() {
        let s = AppState.seconds
        updated.text = "Updated \((row.updatedSec! + s) % 60)s ago"
        clock.text = hms(max(0, row.endsInSec! - s))
        time.text = String(format: "0:%02d / 0:12", s % 12)
    }

    override func tickBody(in cv: UICollectionView) {
        updateClockText()
        relayout()
        setNeedsLayout()
    }

    override func visibilityChanged() { startMotion() }

    /// p_i(t) = clamp(base + 0.2 sin(2π(t/4 + i/3)), 0, 1), sampled at 120 Hz over its 4 s period;
    /// the playhead's blue bar count ceil(48 q), q = (t mod 12)/12, as a discrete 12 s animation.
    private func startMotion() {
        for a in arcs { a.removeAnimation(forKey: "p") }
        blueMask.removeAnimation(forKey: "q")
        guard !freeze, visible, let row else { return }
        for i in 0..<3 {
            let base = row.rings![i]
            let n = 480
            let a = CAKeyframeAnimation(keyPath: "strokeEnd")
            a.values = (0...n).map { k -> Double in
                let t = 4 * Double(k) / Double(n)
                return min(1, max(0, base + 0.2 * sin(2 * .pi * (t / 4 + Double(i) / 3))))
            }
            a.keyTimes = (0...n).map { NSNumber(value: Double($0) / Double(n)) }
            a.duration = 4
            a.repeatCount = .infinity
            a.beginTime = launchBegin(arcs[i])
            a.isRemovedOnCompletion = false
            a.preferredFrameRateRange = fullFrameRate
            arcs[i].add(a, forKey: "p")
        }
        let q = CAKeyframeAnimation(keyPath: "bounds.size.width")
        q.calculationMode = .discrete
        q.values = (1...48).map { CGFloat(5 * $0 - 1) }
        q.keyTimes = (0...48).map { NSNumber(value: Double($0) / 48) }
        q.duration = 12
        q.repeatCount = .infinity
        q.beginTime = launchBegin(blueMask)
        q.isRemovedOnCompletion = false
        blueMask.add(q, forKey: "q")
    }

    override func layoutBody(x: CGFloat, y: CGFloat, C: CGFloat) -> CGFloat {
        let w = C - 28
        let th = ceil(title.font.lineHeight)
        let us = updated.sizeThatFits(.zero)
        updated.frame = CGRect(x: 14 + w - ceil(us.width), y: 14 + (th - ceil(us.height)) / 2, width: ceil(us.width), height: ceil(us.height))
        title.frame = CGRect(x: 14, y: 14, width: updated.frame.minX - 22, height: th)
        // "Ends in " and the countdown share a baseline
        let ey = 14 + th + 10
        let cl = ceil(clock.font.lineHeight), el = ceil(endsIn.font.lineHeight)
        let ew = ceil(endsIn.sizeThatFits(.zero).width)
        let base = ey + clock.font.ascender
        endsIn.frame = CGRect(x: 14, y: base - endsIn.font.ascender, width: ew, height: el)
        clock.frame = CGRect(x: 14 + ew, y: ey, width: ceil(clock.sizeThatFits(.zero).width), height: cl)
        let ry = ey + cl + 12
        ringsView.frame = CGRect(x: 14, y: ry, width: 200, height: 56)
        let wy = ry + 56 + 12
        wave.frame = CGRect(x: 14, y: wy, width: 290, height: 40)
        CATransaction.begin(); CATransaction.setDisableActions(true)
        play.frame = CGRect(x: 0, y: 0, width: 32, height: 40)
        tri.frame = play.frame
        CATransaction.commit()
        let ts = time.sizeThatFits(.zero)
        time.frame = CGRect(x: 14 + 32 + 10 + 238 + 10, y: wy + (40 - ceil(ts.height)) / 2, width: ceil(ts.width), height: ceil(ts.height))
        let h = wy + 40 + 14
        card.frame = CGRect(x: x, y: y, width: C, height: h)
        return h
    }
}
