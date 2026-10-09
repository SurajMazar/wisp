import AppKit
import QuartzCore
import WispIPC
import WispPlatform

/// Drawing surface covering one display. Coordinates are view-local AppKit points.
@MainActor
final class FocusOverlayView: NSView {
    enum Phase { case armed, drawing, done }

    struct Selection {
        var kind: FocusKind
        var rect: CGRect
        var path: [CGPoint]
    }

    var style: DrawStyle = .rectangle
    var dimSurroundings = true
    var accent: NSColor = .systemPurple
    var onFinish: ((Selection) -> Void)?
    var onCancel: (() -> Void)?
    var onPhase: ((Phase) -> Void)?
    var reduceMotion = false

    private(set) var phase: Phase = .armed
    private var start: CGPoint = .zero
    private var points: [CGPoint] = []
    private let dimLayer = CAShapeLayer()
    private let outline = CAShapeLayer()
    private let hint = CATextLayer()
    private let hintBackground = CALayer()
    private let focused = CATextLayer()
    private let focusedBackground = CALayer()

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.backgroundColor = .clear
        dimLayer.fillRule = .evenOdd
        dimLayer.fillColor = NSColor.black.withAlphaComponent(0.26).cgColor
        dimLayer.opacity = 0
        outline.fillColor = .clear
        outline.lineWidth = 2
        outline.lineJoin = .round
        outline.shadowOffset = .zero
        outline.shadowRadius = 6
        outline.shadowOpacity = 0.6
        for l in [dimLayer, outline, hintBackground, focusedBackground] { layer?.addSublayer(l) }
        for (text, bg) in [(hint, hintBackground), (focused, focusedBackground)] {
            bg.backgroundColor = NSColor(white: 0.08, alpha: 0.86).cgColor
            bg.cornerRadius = 11
            bg.opacity = 0
            text.fontSize = 12
            text.font = NSFont.systemFont(ofSize: 12, weight: .medium)
            text.foregroundColor = NSColor.white.cgColor
            text.alignmentMode = .center
            text.contentsScale = 2
            bg.addSublayer(text)
        }
        setAccessibilityElement(true)
        setAccessibilityRole(.group)
        setAccessibilityLabel("Focus selection. Drag around something, click a point, or press Escape to cancel.")
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not supported") }

    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    func prepare(showHint: Bool) {
        outline.strokeColor = accent.cgColor
        outline.shadowColor = accent.cgColor
        dimLayer.frame = bounds
        outline.frame = bounds
        if showHint {
            let s: String
            switch style {
            case .rectangle: s = "Drag a box around something · click a point · Esc to cancel"
            case .lasso: s = "Draw around something · click a point · Esc to cancel"
            case .circle: s = "Drag a circle around something · click a point · Esc to cancel"
            }
            place(text: s, in: hint, background: hintBackground, at: CGPoint(x: bounds.midX, y: bounds.maxY - 70), centered: true)
        }
    }

    /// Dim the whole display (another display holds the selection).
    func dimAll(_ on: Bool) {
        guard dimSurroundings else { return }
        dimLayer.path = CGPath(rect: bounds, transform: nil)
        dimLayer.opacity = on ? 1 : 0
    }

    // MARK: Input

    override func mouseDown(with event: NSEvent) {
        guard phase != .done else { return }
        start = convert(event.locationInWindow, from: nil)
        points = [start]
        phase = .drawing
        onPhase?(.drawing)
        hintBackground.opacity = 0
    }

    override func mouseDragged(with event: NSEvent) {
        guard phase == .drawing else { return }
        let p = convert(event.locationInWindow, from: nil)
        if let last = points.last, hypot(p.x - last.x, p.y - last.y) < 1.5 { return }
        points.append(p)
        render(current: p)
    }

    override func mouseUp(with event: NSEvent) {
        guard phase == .drawing else { return }
        let end = convert(event.locationInWindow, from: nil)
        points.append(end)
        phase = .done
        if FocusGeometry.isDrag(from: start, to: end) || (style == .lasso && pathLength() > 24) {
            let sel: Selection
            switch style {
            case .rectangle:
                sel = Selection(kind: .rectangle, rect: FocusGeometry.rect(from: start, to: end), path: [])
            case .circle:
                sel = Selection(kind: .circle, rect: FocusGeometry.rect(from: start, to: end), path: [])
            case .lasso:
                sel = Selection(kind: .lasso, rect: FocusGeometry.bounds(of: points) ?? .zero, path: points)
            }
            onFinish?(sel)
        } else {
            onFinish?(Selection(kind: .point, rect: CGRect(origin: start, size: .zero), path: [start]))
        }
    }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 { onCancel?() } else { super.keyDown(with: event) }
    }

    override func cancelOperation(_ sender: Any?) {
        onCancel?()
    }

    // MARK: Rendering

    private func pathLength() -> CGFloat {
        zip(points, points.dropFirst()).reduce(0) { $0 + hypot($1.1.x - $1.0.x, $1.1.y - $1.0.y) }
    }

    private func shapePath(current: CGPoint) -> CGPath {
        switch style {
        case .rectangle:
            return CGPath(roundedRect: FocusGeometry.rect(from: start, to: current), cornerWidth: 4, cornerHeight: 4, transform: nil)
        case .circle:
            return CGPath(ellipseIn: FocusGeometry.rect(from: start, to: current), transform: nil)
        case .lasso:
            let p = CGMutablePath()
            p.addLines(between: points)
            if points.count > 2 { p.closeSubpath() }
            return p
        }
    }

    private func render(current: CGPoint) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        let shape = shapePath(current: current)
        outline.path = shape
        if dimSurroundings {
            let dim = CGMutablePath()
            dim.addRect(bounds)
            dim.addPath(shape)
            dimLayer.path = dim
            dimLayer.opacity = 1
        }
        CATransaction.commit()
    }

    /// Show the final shape with a "Focused" label (spec §33), then fade.
    func showFocused(rect: CGRect, path: [CGPoint], kind: FocusKind, completion: @escaping () -> Void) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        let shape: CGPath
        switch kind {
        case .circle: shape = CGPath(ellipseIn: rect, transform: nil)
        case .lasso:
            let p = CGMutablePath()
            p.addLines(between: path)
            p.closeSubpath()
            shape = p
        default: shape = CGPath(roundedRect: rect, cornerWidth: 4, cornerHeight: 4, transform: nil)
        }
        outline.path = shape
        if dimSurroundings {
            let dim = CGMutablePath()
            dim.addRect(bounds)
            dim.addPath(shape)
            dimLayer.path = dim
            dimLayer.opacity = 1
        }
        CATransaction.commit()
        place(text: "Focused", in: focused, background: focusedBackground,
              at: CGPoint(x: rect.minX, y: rect.maxY + 16), centered: false)

        if !reduceMotion {
            let pulse = CABasicAnimation(keyPath: "lineWidth")
            pulse.fromValue = 2
            pulse.toValue = 4
            pulse.duration = 0.18
            pulse.autoreverses = true
            outline.add(pulse, forKey: "pulse")
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + (reduceMotion ? 0.35 : 0.55)) { [weak self] in
            guard let self else { return completion() }
            CATransaction.begin()
            CATransaction.setAnimationDuration(self.reduceMotion ? 0 : 0.2)
            CATransaction.setCompletionBlock(completion)
            for l in [self.dimLayer, self.outline, self.focusedBackground] { l.opacity = 0 }
            CATransaction.commit()
        }
    }

    private func place(text: String, in t: CATextLayer, background bg: CALayer, at p: CGPoint, centered: Bool) {
        let attrs: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 12, weight: .medium)]
        let size = (text as NSString).size(withAttributes: attrs)
        let w = ceil(size.width) + 22, h: CGFloat = 22
        var x = centered ? p.x - w / 2 : p.x
        x = min(max(x, bounds.minX + 8), bounds.maxX - w - 8)
        let y = min(max(p.y - h / 2, bounds.minY + 8), bounds.maxY - h - 8)
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        bg.frame = CGRect(x: x, y: y, width: w, height: h)
        t.string = text
        t.frame = CGRect(x: 0, y: (h - ceil(size.height)) / 2 - 1, width: w, height: ceil(size.height) + 2)
        bg.opacity = 1
        CATransaction.commit()
    }
}
