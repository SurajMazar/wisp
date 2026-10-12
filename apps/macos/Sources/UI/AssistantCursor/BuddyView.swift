import AppKit
import CoreImage
import QuartzCore
import WispIPC

/// The buddy's look: a flat second cursor. Shapes: Wisp's arrowhead, a classic pointer, or a dot —
/// each with a (gradient) glow and a ring that shows state. Purely visual: it never
/// receives mouse events.
@MainActor
final class BuddyView: NSView {
    struct Style: Equatable {
        var shape: CursorShape
        var accent: NSColor
        var glowFrom: NSColor
        var glowTo: NSColor
        var scale: CGFloat
        var opacity: CGFloat
        var glow: CGFloat
        var animation: AnimationLevel
        var reduceMotion: Bool

        var animates: Bool { !reduceMotion && animation != .minimal }
        var amplitude: CGFloat { animation == .expressive ? 1.6 : 1 }
    }

    private let glowLayer = CALayer()
    private let shapeGroup = CALayer()
    private let sideLayer = CAShapeLayer()      // 3D extrusion
    private let faceLayer = CAGradientLayer()
    private let faceMask = CAShapeLayer()
    private let shineLayer = CAGradientLayer()  // specular highlight
    private let shineMask = CAShapeLayer()
    private let edgeLayer = CAShapeLayer()
    private let ringLayer = CAShapeLayer()
    private let echoLayer = CAShapeLayer()
    private(set) var state: AssistantState = .idle
    private var style: Style
    private static let ci = CIContext(options: [.useSoftwareRenderer: false])

    static let artwork: CGFloat = 20
    static let pad: CGFloat = 14

    static func side(for scale: CGFloat) -> CGFloat { (artwork + 2 * pad) * scale }

    init(style: Style) {
        self.style = style
        let side = Self.side(for: style.scale)
        super.init(frame: NSRect(x: 0, y: 0, width: side, height: side))
        wantsLayer = true
        layer?.backgroundColor = .clear
        for l in [echoLayer, ringLayer] { l.fillColor = .clear; layer?.addSublayer(l) }
        layer?.addSublayer(glowLayer)
        layer?.addSublayer(shapeGroup)
        faceLayer.mask = faceMask
        shineLayer.mask = shineMask
        for l in [sideLayer, faceLayer, shineLayer, edgeLayer] as [CALayer] { shapeGroup.addSublayer(l) }
        setAccessibilityElement(false)  // decorative; the panel announces state
        apply(style: style)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not supported") }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    func apply(style newStyle: Style) {
        style = newStyle
        let s = style.scale
        let side = Self.side(for: s)
        setFrameSize(NSSize(width: side, height: side))
        let pad = Self.pad * s
        let art = CGRect(x: pad, y: pad, width: Self.artwork * s, height: Self.artwork * s)
        let path = BuddyShapes.path(style.shape, in: art)
        let solid = style.shape != .dot
        let center = CGPoint(x: art.minX + art.width * 0.45, y: art.maxY - art.height * 0.45)
        let tip = CGPoint(x: art.minX, y: art.maxY)

        CATransaction.begin()
        CATransaction.setDisableActions(true)
        for l: CALayer in [shapeGroup, glowLayer, ringLayer, echoLayer, sideLayer, faceLayer, faceMask, shineLayer, shineMask, edgeLayer] {
            l.frame = bounds
        }
        // Scale animations pivot on the tip of the shape.
        shapeGroup.anchorPoint = CGPoint(x: tip.x / side, y: tip.y / side)
        shapeGroup.position = tip

        // Flat: a single color fill, no depth or shine.
        sideLayer.path = nil
        shineMask.path = nil
        faceMask.path = path
        faceLayer.colors = [style.accent.cgColor, style.accent.cgColor]

        edgeLayer.path = path
        edgeLayer.fillColor = .clear
        edgeLayer.strokeColor = NSColor.white.withAlphaComponent(solid ? 0.5 : 0.8).cgColor
        edgeLayer.lineWidth = (solid ? 0.6 : 1.0) * s
        edgeLayer.lineJoin = .round

        glowLayer.contents = Self.glowImage(path: path, side: side, from: style.glowFrom, to: style.glowTo,
                                            radius: (3 + 8 * style.glow) * s, strength: style.glow)
        glowLayer.contentsScale = 2

        let r = 13 * s
        for l in [ringLayer, echoLayer] {
            l.path = CGPath(ellipseIn: CGRect(x: center.x - r, y: center.y - r, width: 2 * r, height: 2 * r), transform: nil)
            l.lineWidth = 1.5 * s
            l.anchorPoint = CGPoint(x: center.x / side, y: center.y / side)
            l.position = center
        }
        layer?.opacity = Float(style.opacity)
        CATransaction.commit()
        render(state: state, animated: false)
    }

    /// The glow: the shape filled with a gradient, blurred. (A layer shadow can only be one color.)
    static func glowImage(path: CGPath, side: CGFloat, from: NSColor, to: NSColor, radius: CGFloat, strength: CGFloat) -> CGImage? {
        guard strength > 0.01 else { return nil }
        let scale: CGFloat = 2
        let px = Int(side * scale)
        guard let ctx = CGContext(data: nil, width: px, height: px, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
              let gradient = CGGradient(colorsSpace: CGColorSpace(name: CGColorSpace.sRGB), colors: [from.cgColor, to.cgColor] as CFArray, locations: [0, 1])
        else { return nil }
        ctx.scaleBy(x: scale, y: scale)
        ctx.addPath(path)
        ctx.clip()
        let b = path.boundingBoxOfPath
        ctx.drawLinearGradient(gradient, start: CGPoint(x: b.minX, y: b.maxY), end: CGPoint(x: b.maxX, y: b.minY), options: [.drawsBeforeStartLocation, .drawsAfterEndLocation])
        guard let shape = ctx.makeImage() else { return nil }
        let input = CIImage(cgImage: shape)
        let blur = input.clampedToExtent().applyingGaussianBlur(sigma: Double(radius * scale / 2)).cropped(to: input.extent)
        // Brighten the halo a little with strength.
        let boosted = blur.applyingFilter("CIColorMatrix", parameters: ["inputAVector": CIVector(x: 0, y: 0, z: 0, w: 0.55 + 0.9 * strength)])
        return ci.createCGImage(boosted, from: input.extent)
    }

    func set(state newState: AssistantState) {
        guard newState != state else { return }
        state = newState
        render(state: newState, animated: true)
    }

    // MARK: State rendering (spec §28: distinct but subtle)

    private func render(state: AssistantState, animated: Bool) {
        for l in [shapeGroup, glowLayer, ringLayer, echoLayer] { l.removeAllAnimations() }
        let color = BuddyTheme.stateColor(state, accent: style.accent).cgColor
        CATransaction.begin()
        CATransaction.setDisableActions(!animated || style.reduceMotion)
        ringLayer.strokeColor = color
        echoLayer.strokeColor = color
        ringLayer.strokeEnd = 1
        ringLayer.lineDashPattern = nil
        ringLayer.opacity = 0
        echoLayer.opacity = 0
        shapeGroup.opacity = 1
        glowLayer.opacity = 0.85
        CATransaction.commit()

        let a = style.amplitude
        switch state {
        case .idle:
            shapeGroup.opacity = 0.95
        case .following:
            glowLayer.opacity = 1
            if style.animates { breathe(shapeGroup, amount: 0.08 * a, duration: 1.6); breathe(glowLayer, amount: 0.12 * a, duration: 1.6) }
        case .listening, .dictating:
            glowLayer.opacity = 1
            ringLayer.opacity = 0.9
            if style.animates { pulse(ringLayer, to: 1 + 0.45 * a, duration: 1.0); breathe(glowLayer, amount: 0.18 * a, duration: 1.0) }
        case .selecting, .drawing:
            ringLayer.opacity = 0.8
            ringLayer.lineDashPattern = [3, 3]
        case .analyzing, .thinking:
            ringLayer.opacity = 0.95
            ringLayer.strokeEnd = 0.3
            if style.animates { spin(ringLayer, duration: state == .thinking ? 0.9 : 1.4) } else { ringLayer.strokeEnd = 1 }
        case .speaking:
            glowLayer.opacity = 1
            if style.animates {
                pulse(ringLayer, to: 1 + 0.35 * a, duration: 0.9)
                pulse(echoLayer, to: 1 + 0.35 * a, duration: 0.9, delay: 0.45)
                breathe(glowLayer, amount: 0.2 * a, duration: 0.9)
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
            if style.animates { shake(shapeGroup) }
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
