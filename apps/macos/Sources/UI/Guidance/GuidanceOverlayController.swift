import AppKit
import QuartzCore
import WispIPC
import WispPlatform

/// Draws visual guidance (spec §54) on an always-on-top, click-through, transparent overlay on
/// each display: highlights, spotlights, arrows, circles, labels and pointer indicators.
///
/// It never takes focus or clicks, is excluded from Wisp's own screenshots, and goes away on
/// Esc, any click, the next invocation, or after its duration.
@MainActor
public final class GuidanceOverlayController {
    private var panels: [NSPanel] = []
    private var monitors: [Any] = []
    private var dismissTask: Task<Void, Never>?
    public private(set) var currentID: String?
    public var tint = NSColor(srgbHex: 0x7C6CF2)
    public var onDismiss: ((String) -> Void)?

    public init() {}

    public var isShowing: Bool { !panels.isEmpty }

    public func show(_ g: GuidanceShow) {
        clear(notify: false)
        currentID = g.id
        let primary = Displays.primaryHeight
        for screen in NSScreen.screens {
            let mine = g.annotations.filter { Self.bounds(of: $0).map { screenRectCG(screen, primary).intersects($0) } ?? false }
            guard !mine.isEmpty else { continue }
            let panel = Self.makePanel(frame: screen.frame)
            let view = GuidanceView(frame: NSRect(origin: .zero, size: screen.frame.size))
            view.tint = tint
            // Convert global CG (top-left) → this window's view coordinates (bottom-left).
            view.map = { p in
                let a = FocusGeometry.appKitPoint(fromCG: p, primaryHeight: primary)
                return CGPoint(x: a.x - screen.frame.minX, y: a.y - screen.frame.minY)
            }
            panel.contentView = view
            panel.orderFrontRegardless()
            view.draw(annotations: mine)
            panels.append(panel)
        }
        // Esc or any click dismisses (observed, never consumed).
        if let m = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown, .keyDown], handler: { [weak self] e in
            MainActor.assumeIsolated {
                if e.type != .keyDown || e.keyCode == 53 { self?.clear(notify: true) }
            }
        }) { monitors.append(m) }
        if let m = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .keyDown], handler: { [weak self] e in
            if e.type != .keyDown || e.keyCode == 53 { self?.clear(notify: true) }
            return e
        }) { monitors.append(m) }
        let ms = max(1500, min(Int(g.durationMs), 60_000))
        dismissTask = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(ms))
            guard !Task.isCancelled else { return }
            self?.clear(notify: true)
        }
    }

    public func clear(notify: Bool = false) {
        dismissTask?.cancel()
        dismissTask = nil
        monitors.forEach(NSEvent.removeMonitor)
        monitors.removeAll()
        let closing = panels
        panels.removeAll()
        for p in closing {
            NSAnimationContext.runAnimationGroup({ ctx in
                ctx.duration = 0.18
                p.animator().alphaValue = 0
            }, completionHandler: { MainActor.assumeIsolated { p.orderOut(nil) } })
        }
        if notify, let id = currentID { onDismiss?(id) }
        currentID = nil
    }

    private func screenRectCG(_ s: NSScreen, _ primary: CGFloat) -> CGRect {
        FocusGeometry.cgRect(fromAppKit: s.frame, primaryHeight: primary)
    }

    static func bounds(of a: Annotation) -> CGRect? {
        switch a {
        case .highlight(let r), .spotlight(let r), .rectangle(let r): r.rect.cg
        case .circle(let c): CGRect(x: c.center.x - c.radius, y: c.center.y - c.radius, width: c.radius * 2, height: c.radius * 2)
        case .arrow(let a): CGRect(x: min(a.from.x, a.to.x), y: min(a.from.y, a.to.y), width: abs(a.to.x - a.from.x) + 1, height: abs(a.to.y - a.from.y) + 1)
        case .label(let l), .pointer(let l): CGRect(x: l.at.x - 1, y: l.at.y - 1, width: 2, height: 2)
        }
    }

    private static func makePanel(frame: NSRect) -> NSPanel {
        let panel = NSPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.level = .popUpMenu
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.ignoresMouseEvents = true
        panel.sharingType = .none
        panel.isReleasedWhenClosed = false
        panel.hidesOnDeactivate = false
        return panel
    }
}

/// Layer-backed drawing of annotations, animated in.
final class GuidanceView: NSView {
    var tint: NSColor = .systemPurple
    var map: (CGPoint) -> CGPoint = { $0 }

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
    }

    required init?(coder: NSCoder) { fatalError("unused") }

    private func rect(_ r: Rect) -> CGRect {
        let a = map(CGPoint(x: r.x, y: r.y)), b = map(CGPoint(x: r.x + r.width, y: r.y + r.height))
        return CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(b.x - a.x), height: abs(b.y - a.y))
    }

    private var reduceMotion: Bool { NSWorkspace.shared.accessibilityDisplayShouldReduceMotion }

    func draw(annotations: [Annotation]) {
        guard let root = layer else { return }
        let scale = window?.backingScaleFactor ?? 2
        for a in annotations {
            switch a {
            case .spotlight(let s):
                let dim = CAShapeLayer()
                let path = CGMutablePath()
                path.addRect(bounds)
                path.addRoundedRect(in: rect(s.rect).insetBy(dx: -6, dy: -6), cornerWidth: 10, cornerHeight: 10)
                dim.path = path
                dim.fillRule = .evenOdd
                dim.fillColor = NSColor.black.withAlphaComponent(0.45).cgColor
                root.addSublayer(dim)
                fadeIn(dim)
                if let l = s.label { addLabel(l, near: rect(s.rect), scale: scale) }
            case .highlight(let h):
                let r = rect(h.rect)
                let shape = CAShapeLayer()
                shape.path = CGPath(roundedRect: r, cornerWidth: 8, cornerHeight: 8, transform: nil)
                shape.fillColor = tint.withAlphaComponent(0.14).cgColor
                shape.strokeColor = tint.cgColor
                shape.lineWidth = 3
                shape.shadowColor = tint.cgColor
                shape.shadowRadius = 10
                shape.shadowOpacity = 0.9
                shape.shadowOffset = .zero
                root.addSublayer(shape)
                drawStroke(shape)
                pulse(shape)
                if let l = h.label { addLabel(l, near: r, scale: scale) }
            case .rectangle(let h):
                let shape = outline(CGPath(roundedRect: rect(h.rect), cornerWidth: 6, cornerHeight: 6, transform: nil))
                root.addSublayer(shape)
                drawStroke(shape)
                if let l = h.label { addLabel(l, near: rect(h.rect), scale: scale) }
            case .circle(let c):
                let center = map(CGPoint(x: c.center.x, y: c.center.y))
                let r = CGRect(x: center.x - c.radius, y: center.y - c.radius, width: c.radius * 2, height: c.radius * 2)
                let shape = outline(CGPath(ellipseIn: r, transform: nil))
                root.addSublayer(shape)
                drawStroke(shape)
                if let l = c.label { addLabel(l, near: r, scale: scale) }
            case .arrow(let a):
                let from = map(CGPoint(x: a.from.x, y: a.from.y)), to = map(CGPoint(x: a.to.x, y: a.to.y))
                let path = CGMutablePath()
                path.move(to: from)
                path.addLine(to: to)
                let angle = atan2(to.y - from.y, to.x - from.x), head: CGFloat = 16
                path.move(to: CGPoint(x: to.x - head * cos(angle - .pi / 7), y: to.y - head * sin(angle - .pi / 7)))
                path.addLine(to: to)
                path.addLine(to: CGPoint(x: to.x - head * cos(angle + .pi / 7), y: to.y - head * sin(angle + .pi / 7)))
                let shape = outline(path)
                shape.lineCap = .round
                shape.lineJoin = .round
                root.addSublayer(shape)
                drawStroke(shape)
                if let l = a.label { addLabel(l, near: CGRect(origin: from, size: .zero), scale: scale) }
            case .label(let l):
                addLabel(l.text, near: CGRect(origin: map(CGPoint(x: l.at.x, y: l.at.y)), size: .zero), scale: scale)
            case .pointer(let p):
                let c = map(CGPoint(x: p.at.x, y: p.at.y))
                for i in 0..<2 {
                    let ring = CAShapeLayer()
                    ring.frame = CGRect(x: c.x - 14, y: c.y - 14, width: 28, height: 28)
                    ring.path = CGPath(ellipseIn: CGRect(x: 0, y: 0, width: 28, height: 28), transform: nil)
                    ring.fillColor = i == 0 ? tint.withAlphaComponent(0.35).cgColor : NSColor.clear.cgColor
                    ring.strokeColor = tint.cgColor
                    ring.lineWidth = 2.5
                    root.addSublayer(ring)
                    if i == 1, !reduceMotion { ripple(ring) }
                }
                if !p.text.isEmpty { addLabel(p.text, near: CGRect(x: c.x - 14, y: c.y - 14, width: 28, height: 28), scale: scale) }
            }
        }
    }

    private func outline(_ path: CGPath) -> CAShapeLayer {
        let s = CAShapeLayer()
        s.path = path
        s.fillColor = NSColor.clear.cgColor
        s.strokeColor = tint.cgColor
        s.lineWidth = 3
        s.shadowColor = tint.cgColor
        s.shadowRadius = 8
        s.shadowOpacity = 0.8
        s.shadowOffset = .zero
        return s
    }

    /// A pill with the target's name, above the target (below it near the top of the screen).
    private func addLabel(_ text: String, near r: CGRect, scale: CGFloat) {
        guard let root = layer, !text.isEmpty else { return }
        let font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        let str = NSAttributedString(string: String(text.prefix(60)), attributes: [.font: font, .foregroundColor: NSColor.white])
        let size = str.size()
        let w = ceil(size.width) + 20, h: CGFloat = 26
        var y = r.maxY + 8
        if y + h > bounds.maxY - 4 { y = r.minY - h - 8 }
        let x = min(max(r.midX - w / 2, 6), bounds.maxX - w - 6)
        let pill = CALayer()
        pill.frame = CGRect(x: x, y: y, width: w, height: h)
        pill.backgroundColor = tint.cgColor
        pill.cornerRadius = h / 2
        pill.shadowColor = NSColor.black.cgColor
        pill.shadowOpacity = 0.25
        pill.shadowRadius = 6
        pill.shadowOffset = CGSize(width: 0, height: -2)
        let t = CATextLayer()
        t.string = str
        t.contentsScale = scale
        t.alignmentMode = .center
        t.frame = CGRect(x: 0, y: (h - ceil(size.height)) / 2 - 1, width: w, height: ceil(size.height) + 2)
        pill.addSublayer(t)
        root.addSublayer(pill)
        fadeIn(pill)
    }

    private func drawStroke(_ s: CAShapeLayer) {
        guard !reduceMotion else { return }
        let a = CABasicAnimation(keyPath: "strokeEnd")
        a.fromValue = 0
        a.toValue = 1
        a.duration = 0.35
        a.timingFunction = CAMediaTimingFunction(name: .easeOut)
        s.add(a, forKey: "draw")
    }

    private func pulse(_ s: CAShapeLayer) {
        guard !reduceMotion else { return }
        let a = CABasicAnimation(keyPath: "shadowRadius")
        a.fromValue = 6
        a.toValue = 16
        a.duration = 0.9
        a.autoreverses = true
        a.repeatCount = .infinity
        a.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        s.add(a, forKey: "pulse")
    }

    private func ripple(_ s: CAShapeLayer) {
        let g = CAAnimationGroup()
        let scale = CABasicAnimation(keyPath: "transform.scale")
        scale.fromValue = 1
        scale.toValue = 2.2
        let fade = CABasicAnimation(keyPath: "opacity")
        fade.fromValue = 1
        fade.toValue = 0
        g.animations = [scale, fade]
        g.duration = 1.2
        g.repeatCount = .infinity
        s.add(g, forKey: "ripple")
    }

    private func fadeIn(_ l: CALayer) {
        guard !reduceMotion else { return }
        let a = CABasicAnimation(keyPath: "opacity")
        a.fromValue = 0
        a.toValue = 1
        a.duration = 0.25
        l.add(a, forKey: "fade")
    }
}
