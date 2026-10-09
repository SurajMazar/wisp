import AppKit
import QuartzCore
import WispIPC

/// The buddy's look: a small luminous pointer — a second cursor — with a glow and a state ring.
/// Purely visual: it never receives mouse events.
@MainActor
final class BuddyView: NSView {
    struct Style: Equatable {
        var accent: NSColor
        var secondary: NSColor
        var scale: CGFloat
        var opacity: CGFloat
        var glow: CGFloat
        var animation: AnimationLevel
        var reduceMotion: Bool

        var animates: Bool { !reduceMotion && animation != .minimal }
        var amplitude: CGFloat { animation == .expressive ? 1.6 : 1 }
    }

    private let glowLayer = CAShapeLayer()
    private let fillLayer = CAGradientLayer()
    private let fillMask = CAShapeLayer()
    private let outlineLayer = CAShapeLayer()
    private let ringLayer = CAShapeLayer()
    private let echoLayer = CAShapeLayer()
    private let arrowGroup = CALayer()
    private(set) var state: AssistantState = .idle
    private var style: Style

    /// Pointer artwork size (points at scale 1) and padding for glow and rings.
    static let arrow = CGSize(width: 14, height: 20)
    static let pad: CGFloat = 12

    static func side(for scale: CGFloat) -> CGFloat { (max(arrow.width, arrow.height) + 2 * pad) * scale }

    init(style: Style) {
        self.style = style
        let side = Self.side(for: style.scale)
        super.init(frame: NSRect(x: 0, y: 0, width: side, height: side))
        wantsLayer = true
        layer?.backgroundColor = .clear
        for l in [echoLayer, ringLayer] { l.fillColor = .clear; layer?.addSublayer(l) }
        layer?.addSublayer(arrowGroup)
        arrowGroup.addSublayer(glowLayer)
        fillLayer.mask = fillMask
        arrowGroup.addSublayer(fillLayer)
        arrowGroup.addSublayer(outlineLayer)
        setAccessibilityElement(false)  // decorative; state is announced by the assistant panel
        apply(style: style)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not supported") }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    /// The pointer outline with its tip at the top-left of `rect` (y-up layer coordinates).
    static func arrowPath(in rect: CGRect) -> CGPath {
        // Classic pointer, designed on a 14×20 grid with y pointing down from the tip.
        let pts: [(CGFloat, CGFloat)] = [(0, 0), (0, 16.5), (4.2, 12.8), (7.1, 19.6), (9.6, 18.5), (6.8, 11.9), (12.4, 11.9)]
        let sx = rect.width / arrow.width, sy = rect.height / arrow.height
        let p = CGMutablePath()
        for (i, (x, y)) in pts.enumerated() {
            let pt = CGPoint(x: rect.minX + x * sx, y: rect.maxY - y * sy)
            if i == 0 { p.move(to: pt) } else { p.addLine(to: pt) }
        }
        p.closeSubpath()
        return p
    }

    func apply(style newStyle: Style) {
        style = newStyle
        let s = style.scale
        let side = Self.side(for: s)
        setFrameSize(NSSize(width: side, height: side))
        let pad = Self.pad * s
        let arrowRect = CGRect(x: pad, y: side - pad - Self.arrow.height * s, width: Self.arrow.width * s, height: Self.arrow.height * s)
        let path = Self.arrowPath(in: arrowRect)
        let center = CGPoint(x: arrowRect.midX - 1 * s, y: arrowRect.midY + 1 * s)

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        for l in [arrowGroup, ringLayer, echoLayer] { l.frame = bounds }
        for l: CALayer in [glowLayer, fillLayer, fillMask, outlineLayer] { l.frame = bounds }
        // Scale animations pivot on the pointer, not the window center.
        arrowGroup.anchorPoint = CGPoint(x: arrowRect.minX / side, y: arrowRect.maxY / side)
        arrowGroup.position = CGPoint(x: arrowRect.minX, y: arrowRect.maxY)

        glowLayer.path = path
        glowLayer.fillColor = style.secondary.withAlphaComponent(0.9).cgColor
        glowLayer.shadowColor = style.secondary.cgColor
        glowLayer.shadowOffset = .zero
        glowLayer.shadowRadius = (2 + 7 * style.glow) * s
        glowLayer.shadowOpacity = Float(0.2 + 0.8 * style.glow)

        fillMask.path = path
        fillLayer.colors = [style.accent.blended(withFraction: 0.35, of: .white)!.cgColor, style.accent.cgColor, style.secondary.cgColor]
        fillLayer.locations = [0, 0.55, 1]
        fillLayer.startPoint = CGPoint(x: 0, y: 1)
        fillLayer.endPoint = CGPoint(x: 1, y: 0)

        outlineLayer.path = path
        outlineLayer.fillColor = .clear
        outlineLayer.strokeColor = NSColor.white.withAlphaComponent(0.95).cgColor
        outlineLayer.lineWidth = 1.3 * s
        outlineLayer.lineJoin = .round

        let r = 11 * s
        for l in [ringLayer, echoLayer] {
            l.path = CGPath(ellipseIn: CGRect(x: center.x - r, y: center.y - r, width: 2 * r, height: 2 * r), transform: nil)
            l.lineWidth = 1.5 * s
            l.bounds = bounds
            l.position = CGPoint(x: bounds.midX, y: bounds.midY)
            l.anchorPoint = CGPoint(x: center.x / side, y: center.y / side)
            l.position = center
        }
        layer?.opacity = Float(style.opacity)
        CATransaction.commit()
        render(state: state, animated: false)
    }

    func set(state newState: AssistantState) {
        guard newState != state else { return }
        state = newState
        render(state: newState, animated: true)
    }

    // MARK: State rendering (spec §28: distinct but subtle)

    private func render(state: AssistantState, animated: Bool) {
        for l in [arrowGroup, ringLayer, echoLayer] { l.removeAllAnimations() }
        let color = BuddyTheme.stateColor(state, accent: style.accent).cgColor
        CATransaction.begin()
        CATransaction.setDisableActions(!animated || style.reduceMotion)
        ringLayer.strokeColor = color
        echoLayer.strokeColor = color
        ringLayer.strokeEnd = 1
        ringLayer.lineDashPattern = nil
        ringLayer.opacity = 0
        echoLayer.opacity = 0
        arrowGroup.opacity = 1
        CATransaction.commit()

        let a = style.amplitude
        switch state {
        case .idle:
            arrowGroup.opacity = 0.92
        case .following:
            if style.animates { breathe(arrowGroup, amount: 0.08 * a, duration: 1.6) }
        case .listening:
            ringLayer.opacity = 0.9
            if style.animates { pulse(ringLayer, to: 1 + 0.45 * a, duration: 1.0) }
        case .selecting, .drawing:
            ringLayer.opacity = 0.8
            ringLayer.lineDashPattern = [3, 3]
        case .analyzing, .thinking:
            ringLayer.opacity = 0.95
            ringLayer.strokeEnd = 0.3
            if style.animates { spin(ringLayer, duration: state == .thinking ? 0.9 : 1.4) } else { ringLayer.strokeEnd = 1 }
        case .speaking:
            if style.animates {
                pulse(ringLayer, to: 1 + 0.35 * a, duration: 0.9)
                pulse(echoLayer, to: 1 + 0.35 * a, duration: 0.9, delay: 0.45)
                breathe(arrowGroup, amount: 0.06 * a, duration: 0.9)
            } else {
                ringLayer.opacity = 0.8
            }
        case .executing:
            ringLayer.opacity = 0.9
            ringLayer.lineDashPattern = [5, 4]
            if style.animates { spin(ringLayer, duration: 2.4) }
        case .waitingForPermission:
            ringLayer.opacity = 1
            if style.animates { blink(ringLayer) }
        case .success:
            ringLayer.opacity = 1
            if style.animates { pulse(ringLayer, to: 1.6, duration: 0.6, repeats: false) }
        case .error:
            ringLayer.opacity = 1
            if style.animates { shake(arrowGroup) }
        }
    }

    private func breathe(_ l: CALayer, amount: CGFloat, duration: CFTimeInterval) {
        let anim = CABasicAnimation(keyPath: "transform.scale")
        anim.fromValue = 1
        anim.toValue = 1 + amount
        anim.duration = duration / 2
        anim.autoreverses = true
        anim.repeatCount = .infinity
        anim.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        l.add(anim, forKey: "breathe")
    }

    private func pulse(_ l: CALayer, to scale: CGFloat, duration: CFTimeInterval, delay: CFTimeInterval = 0, repeats: Bool = true) {
        let s = CABasicAnimation(keyPath: "transform.scale")
        s.fromValue = 1
        s.toValue = scale
        let o = CABasicAnimation(keyPath: "opacity")
        o.fromValue = 0.9
        o.toValue = 0
        let g = CAAnimationGroup()
        g.animations = [s, o]
        g.duration = duration
        g.beginTime = CACurrentMediaTime() + delay
        g.repeatCount = repeats ? .infinity : 1
        g.fillMode = .backwards
        g.timingFunction = CAMediaTimingFunction(name: .easeOut)
        l.add(g, forKey: "pulse")
    }

    private func spin(_ l: CALayer, duration: CFTimeInterval) {
        let r = CABasicAnimation(keyPath: "transform.rotation.z")
        r.fromValue = 0
        r.toValue = -2 * Double.pi
        r.duration = duration
        r.repeatCount = .infinity
        l.add(r, forKey: "spin")
    }

    private func blink(_ l: CALayer) {
        let o = CABasicAnimation(keyPath: "opacity")
        o.fromValue = 1
        o.toValue = 0.35
        o.duration = 0.7
        o.autoreverses = true
        o.repeatCount = .infinity
        l.add(o, forKey: "blink")
    }

    private func shake(_ l: CALayer) {
        let k = CAKeyframeAnimation(keyPath: "transform.translation.x")
        k.values = [0, -3, 3, -2, 2, 0].map { $0 * style.scale }
        k.duration = 0.4
        l.add(k, forKey: "shake")
    }
}
