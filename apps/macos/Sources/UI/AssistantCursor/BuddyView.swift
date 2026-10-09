import AppKit
import QuartzCore
import WispIPC

/// The buddy's visuals: a luminous orb with a glow and a state ring, all Core Animation.
/// Each assistant state has a distinct but subtle treatment (spec §28).
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

    var onClick: (() -> Void)?

    private let glowLayer = CAShapeLayer()
    private let coreLayer = CAGradientLayer()
    private let coreMask = CAShapeLayer()
    private let ringLayer = CAShapeLayer()
    private let echoLayer = CAShapeLayer()
    private(set) var state: AssistantState = .idle
    private var style: Style
    private var hovering = false

    static let baseOrb: CGFloat = 16
    static let basePad: CGFloat = 16

    static func side(for scale: CGFloat) -> CGFloat { (baseOrb + 2 * basePad) * scale }

    init(style: Style) {
        self.style = style
        let side = Self.side(for: style.scale)
        super.init(frame: NSRect(x: 0, y: 0, width: side, height: side))
        wantsLayer = true
        layer?.backgroundColor = .clear
        for l in [glowLayer, echoLayer, ringLayer] { l.fillColor = .clear; layer?.addSublayer(l) }
        coreLayer.type = .radial
        coreLayer.startPoint = CGPoint(x: 0.42, y: 0.62)
        coreLayer.endPoint = CGPoint(x: 1, y: 0)
        coreLayer.mask = coreMask
        layer?.addSublayer(coreLayer)
        setAccessibilityElement(true)
        setAccessibilityRole(.button)
        setAccessibilityLabel("Wisp assistant")
        setAccessibilityHelp("Opens the assistant")
        apply(style: style)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not supported") }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func mouseDown(with event: NSEvent) {
        onClick?()
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        // Only the orb (plus a little slack) is clickable; the glow is click-through.
        let local = convert(point, from: superview)
        let r = Self.baseOrb * style.scale / 2 + 6
        return hypot(local.x - bounds.midX, local.y - bounds.midY) <= r ? self : nil
    }

    func apply(style newStyle: Style) {
        style = newStyle
        let side = Self.side(for: style.scale)
        setFrameSize(NSSize(width: side, height: side))
        let orb = Self.baseOrb * style.scale
        let c = CGPoint(x: side / 2, y: side / 2)
        let orbRect = CGRect(x: c.x - orb / 2, y: c.y - orb / 2, width: orb, height: orb)

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        for l in [glowLayer, ringLayer, echoLayer] {
            l.frame = bounds
        }
        coreLayer.frame = bounds
        coreMask.frame = bounds
        coreMask.path = CGPath(ellipseIn: orbRect, transform: nil)
        coreLayer.colors = [NSColor.white.cgColor, style.accent.blended(withFraction: 0.35, of: .white)!.cgColor, style.accent.cgColor]
        coreLayer.locations = [0, 0.35, 1]

        glowLayer.path = CGPath(ellipseIn: orbRect.insetBy(dx: 1, dy: 1), transform: nil)
        glowLayer.fillColor = style.secondary.withAlphaComponent(0.55).cgColor
        glowLayer.shadowColor = style.secondary.cgColor
        glowLayer.shadowOffset = .zero
        glowLayer.shadowRadius = (3 + 9 * style.glow) * style.scale
        glowLayer.shadowOpacity = Float(0.25 + 0.75 * style.glow)

        let ringRect = orbRect.insetBy(dx: -5 * style.scale, dy: -5 * style.scale)
        for l in [ringLayer, echoLayer] {
            l.path = CGPath(ellipseIn: ringRect, transform: nil)
            l.lineWidth = 1.6 * style.scale
            l.bounds = bounds
            l.position = c
        }
        layer?.opacity = Float(style.opacity)
        CATransaction.commit()
        render(state: state, animated: false)
    }

    func set(state newState: AssistantState) {
        guard newState != state else { return }
        state = newState
        render(state: newState, animated: true)
        setAccessibilityValue(BuddyTheme.label(for: newState) ?? "Ready")
    }

    func set(hovering h: Bool) {
        guard h != hovering else { return }
        hovering = h
        let s: CGFloat = h ? 1.18 : 1
        CATransaction.begin()
        CATransaction.setAnimationDuration(style.reduceMotion ? 0 : 0.16)
        coreLayer.setAffineTransform(CGAffineTransform(scaleX: s, y: s))
        glowLayer.setAffineTransform(CGAffineTransform(scaleX: s, y: s))
        CATransaction.commit()
    }

    // MARK: State rendering

    private func render(state: AssistantState, animated: Bool) {
        for l in [coreLayer, glowLayer, ringLayer, echoLayer] { l.removeAllAnimations() }
        let color = BuddyTheme.stateColor(state, accent: style.accent).cgColor
        CATransaction.begin()
        CATransaction.setDisableActions(!animated || style.reduceMotion)
        ringLayer.strokeColor = color
        echoLayer.strokeColor = color
        ringLayer.strokeEnd = 1
        ringLayer.lineDashPattern = nil
        ringLayer.opacity = 0
        echoLayer.opacity = 0
        coreLayer.opacity = 1
        CATransaction.commit()

        let a = style.amplitude
        switch state {
        case .idle:
            coreLayer.opacity = 0.88
            if style.animates { breathe(coreLayer, amount: 0.05 * a, duration: 2.6) }
        case .following:
            ringLayer.opacity = 0.45
            if style.animates { breathe(ringLayer, amount: 0.06 * a, duration: 1.8) }
        case .listening:
            ringLayer.opacity = 0.9
            if style.animates { pulse(ringLayer, to: 1 + 0.4 * a, duration: 1.1) } else { ringLayer.opacity = 0.9 }
        case .selecting, .drawing:
            ringLayer.opacity = 0.85
            ringLayer.lineDashPattern = [3, 3]
        case .analyzing, .thinking:
            ringLayer.opacity = 0.95
            ringLayer.strokeEnd = 0.3
            if style.animates { spin(ringLayer, duration: state == .thinking ? 0.9 : 1.4) } else { ringLayer.strokeEnd = 1 }
        case .speaking:
            ringLayer.opacity = 0.8
            if style.animates {
                pulse(ringLayer, to: 1 + 0.35 * a, duration: 0.9)
                pulse(echoLayer, to: 1 + 0.35 * a, duration: 0.9, delay: 0.45)
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
            if style.animates { pulse(ringLayer, to: 1.5, duration: 0.6, repeats: false) }
        case .error:
            ringLayer.opacity = 1
            if style.animates { shake(coreLayer) }
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
