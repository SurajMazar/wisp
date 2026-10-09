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
    /// Freehand pen while the shortcut is held: several strokes, finished by releasing the keys
    /// (`finishInk`) rather than by lifting the mouse.
    var ink = false

    /// Finished pen strokes (view-local points).
    private(set) var strokes: [[CGPoint]] = []
    private var strokeLayers: [CAShapeLayer] = []
    private let edge = CAShapeLayer()
    private let tip = CATextLayer()
    private let tipBackground = CALayer()
    private var tracking: NSTrackingArea?

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
        // Nearly invisible, not clear: macOS sends clicks on fully transparent pixels to the app
        // underneath, so a clear overlay would let the drag go through.
        layer?.backgroundColor = NSColor.black.withAlphaComponent(0.002).cgColor
        edge.fillColor = .clear
        edge.lineWidth = 3
        edge.opacity = 0
        dimLayer.fillRule = .evenOdd
        dimLayer.fillColor = NSColor.black.withAlphaComponent(0.26).cgColor
        dimLayer.opacity = 0
        outline.fillColor = .clear
        outline.lineWidth = 2
        outline.lineJoin = .round
        outline.shadowOffset = .zero
        outline.shadowRadius = 6
        outline.shadowOpacity = 0.6
        for l in [dimLayer, edge, outline, hintBackground, focusedBackground, tipBackground] { layer?.addSublayer(l) }
        for (text, bg) in [(hint, hintBackground), (focused, focusedBackground), (tip, tipBackground)] {
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
        if ink {
            prepareInk()
            return
        }
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

    // MARK: Ink mode

    private func prepareInk() {
        // A glowing frame says "Wisp is listening and you can draw"; a tip follows the pointer.
        edge.frame = bounds
        edge.path = CGPath(rect: bounds.insetBy(dx: 1.5, dy: 1.5), transform: nil)
        edge.strokeColor = accent.withAlphaComponent(0.75).cgColor
        edge.shadowColor = accent.cgColor
        edge.shadowRadius = 10
        edge.shadowOpacity = 0.9
        edge.shadowOffset = .zero
        edge.opacity = 1
        if !reduceMotion {
            let breathe = CABasicAnimation(keyPath: "opacity")
            breathe.fromValue = 1
            breathe.toValue = 0.45
            breathe.duration = 0.9
            breathe.autoreverses = true
            breathe.repeatCount = .infinity
            edge.add(breathe, forKey: "breathe")
        }
        tipBackground.backgroundColor = accent.withAlphaComponent(0.92).cgColor
        if let w = window {
            let p = convert(w.mouseLocationOutsideOfEventStream, from: nil)
            if bounds.contains(p) { moveTip(to: p) }
        }
    }

    private func moveTip(to p: CGPoint) {
        let text = strokes.isEmpty && phase != .drawing ? "Listening · draw to point" : "Release keys to send"
        place(text: text, in: tip, background: tipBackground, at: CGPoint(x: p.x + 18, y: p.y - 26), centered: false)
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let tracking { removeTrackingArea(tracking) }
        let t = NSTrackingArea(rect: .zero, options: [.activeAlways, .mouseMoved, .cursorUpdate, .inVisibleRect], owner: self)
        addTrackingArea(t)
        tracking = t
    }

    override func cursorUpdate(with event: NSEvent) {
        NSCursor.crosshair.set()
    }

    override func mouseMoved(with event: NSEvent) {
        NSCursor.crosshair.set()
        if ink { moveTip(to: convert(event.locationInWindow, from: nil)) }
    }

    private func newStrokeLayer() -> CAShapeLayer {
        let l = CAShapeLayer()
        l.frame = bounds
        l.fillColor = .clear
        l.strokeColor = accent.cgColor
        l.lineWidth = 4
        l.lineCap = .round
        l.lineJoin = .round
        l.shadowColor = accent.cgColor
        l.shadowRadius = 5
        l.shadowOpacity = 0.7
        l.shadowOffset = .zero
        layer?.insertSublayer(l, below: tipBackground)
        return l
    }

    /// Everything drawn so far, including a stroke still in progress; nil if nothing was drawn.
    func finishInk() -> Selection? {
        if phase == .drawing, points.count > 0 { strokes.append(points) }
        phase = .done
        let all = strokes.flatMap { $0 }
        guard let last = all.last else { return nil }
        let isTap = strokes.allSatisfy { s in
            guard let box = FocusGeometry.bounds(of: s) else { return true }
            return max(box.width, box.height) < 6
        }
        if isTap { return Selection(kind: .point, rect: CGRect(origin: last, size: .zero), path: [last]) }
        // One closed-ish loop reads as "this area"; anything else (arrows, underlines, several
        // marks) selects the box around all of it.
        if strokes.count == 1, let s = strokes.first, let a = s.first, let b = s.last, let box = FocusGeometry.bounds(of: s),
           hypot(a.x - b.x, a.y - b.y) < max(24, 0.3 * max(box.width, box.height)) {
            return Selection(kind: .lasso, rect: box, path: s)
        }
        let box = (FocusGeometry.bounds(of: all) ?? .zero).insetBy(dx: -12, dy: -12)
        return Selection(kind: .rectangle, rect: box, path: [])
    }

    // MARK: Input

    override func mouseDown(with event: NSEvent) {
        guard phase != .done else { return }
        if ink {
            start = convert(event.locationInWindow, from: nil)
            points = [start]
            strokeLayers.append(newStrokeLayer())
            phase = .drawing
            onPhase?(.drawing)
            moveTip(to: start)
            return
        }
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
        if ink {
            let path = CGMutablePath()
            path.addLines(between: points)
            CATransaction.begin()
            CATransaction.setDisableActions(true)
            strokeLayers.last?.path = path
            CATransaction.commit()
            moveTip(to: p)
            return
        }
        render(current: p)
    }

    override func mouseUp(with event: NSEvent) {
        guard phase == .drawing else { return }
        let end = convert(event.locationInWindow, from: nil)
        points.append(end)
        if ink {
            // Keep the mark and stay ready for more; releasing the keys sends.
            if !FocusGeometry.isDrag(from: start, to: end), points.count < 4 {
                // A click: show a dot.
                strokeLayers.last?.path = CGPath(ellipseIn: CGRect(x: end.x - 5, y: end.y - 5, width: 10, height: 10), transform: nil)
                strokeLayers.last?.fillColor = accent.cgColor
            }
            strokes.append(points)
            points = []
            phase = .armed
            moveTip(to: end)
            return
        }
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
        tipBackground.opacity = 0
        edge.removeAllAnimations()
        edge.opacity = 0
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
            for l in [self.dimLayer, self.outline, self.focusedBackground] + self.strokeLayers { l.opacity = 0 }
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
