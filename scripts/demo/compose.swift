// Composes demo frames (1920×1080) from Wisp's own renders: a desktop with a notch, the island,
// an app window, overlays, banners, titles and captions. Input: a JSON array of frames.
//   swiftc -O compose.swift -o compose && ./compose frames.json
import AppKit
import Foundation

struct Frame: Decodable {
    var out: String
    var kind: String            // "desktop" | "title" | "caption" | "mockapp" (a plain app window, 1800×1040)
    var window: String?          // app window content image
    var windowSize: [Double]?    // points (w, h); default 1280×760
    var overlay: String?         // drawn over the window rect (e.g. the drawing layer)
    var island: String?          // island render (640×140 pt @2x)
    var banner: String?          // notification text
    var title: String?
    var subtitle: String?
    var caption: String?
    var ink: [Double]?           // ellipse drawn with the pen: centre x, y and radii, as fractions of the window
    var pointer: [Double]?       // the pointer and Wisp's cursor beside it, as fractions of the window
}

let W = 1920.0, H = 1080.0
let accent = NSColor(srgbRed: 0.49, green: 0.42, blue: 0.95, alpha: 1)

func image(_ path: String) -> NSImage { NSImage(contentsOfFile: path)! }

func text(_ s: String, size: CGFloat, weight: NSFont.Weight, color: NSColor, in rect: CGRect, align: NSTextAlignment = .center) {
    let p = NSMutableParagraphStyle()
    p.alignment = align
    p.lineBreakMode = .byWordWrapping
    (s as NSString).draw(in: rect, withAttributes: [
        .font: NSFont.systemFont(ofSize: size, weight: weight), .foregroundColor: color, .paragraphStyle: p,
    ])
}

func backdrop() {
    let g = NSGradient(colors: [NSColor(srgbRed: 0.10, green: 0.09, blue: 0.20, alpha: 1),
                                NSColor(srgbRed: 0.20, green: 0.16, blue: 0.42, alpha: 1),
                                NSColor(srgbRed: 0.13, green: 0.30, blue: 0.48, alpha: 1)])!
    g.draw(in: CGRect(x: 0, y: 0, width: W, height: H), angle: -35)
}

func menuBar() {
    NSColor(white: 0, alpha: 0.35).setFill()
    CGRect(x: 0, y: H - 34, width: W, height: 34).fill()
    text("  Safari    File    Edit    View    History    Window    Help", size: 15, weight: .medium,
         color: NSColor(white: 1, alpha: 0.85), in: CGRect(x: 24, y: H - 28, width: 700, height: 22), align: .left)
    text("Mon 9:41 AM", size: 15, weight: .medium, color: NSColor(white: 1, alpha: 0.85),
         in: CGRect(x: W - 180, y: H - 28, width: 150, height: 22), align: .right)
    // The notch.
    NSColor.black.setFill()
    NSBezierPath(roundedRect: CGRect(x: W / 2 - 100, y: H - 38, width: 200, height: 52), xRadius: 14, yRadius: 14).fill()
}

func caption(_ s: String) {
    let r = CGRect(x: 160, y: 54, width: W - 320, height: 96)
    NSColor(white: 0.04, alpha: 0.78).setFill()
    NSBezierPath(roundedRect: r, xRadius: 22, yRadius: 22).fill()
    text(s, size: 30, weight: .medium, color: .white, in: r.insetBy(dx: 36, dy: 26))
}

@discardableResult
func window(_ content: NSImage, size: CGSize, overlay: NSImage?) -> CGRect {
    // Between the caption (bottom) and the island (top), a little smaller than life.
    let w = size.width * 0.92, h = size.height * 0.92
    let r = CGRect(x: (W - w) / 2, y: 180 + (700 - h) / 2, width: w, height: h)
    let shadow = NSShadow()
    shadow.shadowBlurRadius = 40
    shadow.shadowOffset = NSSize(width: 0, height: -12)
    shadow.shadowColor = NSColor(white: 0, alpha: 0.55)
    NSGraphicsContext.saveGraphicsState()
    shadow.set()
    NSColor(white: 0.12, alpha: 1).setFill()
    NSBezierPath(roundedRect: r.insetBy(dx: 0, dy: -18).offsetBy(dx: 0, dy: 18), xRadius: 14, yRadius: 14).fill()
    NSGraphicsContext.restoreGraphicsState()
    // Title bar with traffic lights.
    for (i, c) in [NSColor.systemRed, .systemYellow, .systemGreen].enumerated() {
        c.setFill()
        NSBezierPath(ovalIn: CGRect(x: r.minX + 18 + CGFloat(i) * 22, y: r.maxY + 8, width: 13, height: 13)).fill()
    }
    NSGraphicsContext.saveGraphicsState()
    NSBezierPath(roundedRect: r, xRadius: 6, yRadius: 6).addClip()
    content.draw(in: r)
    overlay?.draw(in: r)
    NSGraphicsContext.restoreGraphicsState()
    return r
}

/// A point in the window from fractions measured from its top-left corner.
func at(_ r: CGRect, _ fx: Double, _ fy: Double) -> CGPoint { CGPoint(x: r.minX + fx * r.width, y: r.maxY - fy * r.height) }

/// The pen overlay: a faint tint at the screen edge, the stroke with its glow, and the hint pill.
func ink(_ e: [Double], in r: CGRect) {
    accent.withAlphaComponent(0.55).setStroke()
    let edge = NSBezierPath(rect: CGRect(x: 0, y: 0, width: W, height: H).insetBy(dx: 2, dy: 2))
    edge.lineWidth = 4
    edge.stroke()
    let c = at(r, e[0], e[1])
    let rx = e[2] * r.width, ry = e[3] * r.height
    let path = NSBezierPath(ovalIn: CGRect(x: c.x - rx, y: c.y - ry, width: rx * 2, height: ry * 2))
    path.lineWidth = 4
    let glow = NSShadow()
    glow.shadowBlurRadius = 10
    glow.shadowColor = accent.withAlphaComponent(0.8)
    NSGraphicsContext.saveGraphicsState()
    glow.set()
    accent.setStroke()
    path.stroke()
    NSGraphicsContext.restoreGraphicsState()
    let pill = CGRect(x: c.x + rx + 16, y: c.y - ry - 6, width: 200, height: 34)
    accent.setFill()
    NSBezierPath(roundedRect: pill, xRadius: 17, yRadius: 17).fill()
    text("Release keys to send", size: 16, weight: .semibold, color: .white, in: pill.insetBy(dx: 8, dy: 7))
}

/// The system arrow, with Wisp's glowing cursor just beside it.
func pointer(_ p: [Double], in r: CGRect) {
    let tip = at(r, p[0], p[1])
    let orb = NSGradient(colors: [NSColor(white: 1, alpha: 0.95), accent, NSColor(srgbRed: 0.37, green: 0.78, blue: 0.95, alpha: 0)])!
    orb.draw(in: NSBezierPath(ovalIn: CGRect(x: tip.x + 22, y: tip.y - 52, width: 30, height: 30)), relativeCenterPosition: NSPoint(x: -0.2, y: 0.2))
    let a = NSBezierPath()
    for (i, q) in [(0, 0), (0, -30), (7, -23), (12, -34), (17, -32), (12, -21), (21, -21)].enumerated() {
        let pt = CGPoint(x: tip.x + CGFloat(q.0), y: tip.y + CGFloat(q.1))
        if i == 0 { a.move(to: pt) } else { a.line(to: pt) }
    }
    a.close()
    NSColor.black.setFill()
    a.fill()
    NSColor.white.setStroke()
    a.lineWidth = 2
    a.stroke()
}

/// A plain app window: the controls the guidance renders point at, without any guidance.
func mockApp() {
    NSColor(white: 0.957, alpha: 1).setFill()
    CGRect(x: 0, y: 0, width: 1800, height: 1040).fill()
    for (label, r) in [("Settings", CGRect(x: 120, y: 120, width: 220, height: 60)),
                       ("Total: $1,240.00", CGRect(x: 120, y: 600, width: 400, height: 44)),
                       ("Export…", CGRect(x: 1040, y: 880, width: 200, height: 60)),
                       ("Save", CGRect(x: 1280, y: 880, width: 180, height: 60))] {
        let flipped = CGRect(x: r.minX, y: 1040 - r.maxY, width: r.width, height: r.height)
        let box = NSBezierPath(roundedRect: flipped, xRadius: 12, yRadius: 12)
        NSColor.white.setFill()
        box.fill()
        NSColor(white: 0.78, alpha: 1).setStroke()
        box.lineWidth = 2
        box.stroke()
        text(label, size: 26, weight: .regular, color: .black,
             in: CGRect(x: flipped.minX + 20, y: flipped.midY - 18, width: flipped.width - 30, height: 36), align: .left)
    }
}

func render(_ f: Frame) {
    let mock = f.kind == "mockapp"
    let px = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: mock ? 1800 : Int(W), pixelsHigh: mock ? 1040 : Int(H), bitsPerSample: 8,
                              samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                              bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: px)
    switch f.kind {
    case "mockapp":
        mockApp()
    case "caption":
        NSColor.clear.setFill()
        CGRect(x: 0, y: 0, width: W, height: H).fill()
    case "title":
        backdrop()
        let orb = NSGradient(colors: [NSColor(white: 1, alpha: 0.95), accent, NSColor(srgbRed: 0.37, green: 0.78, blue: 0.95, alpha: 0.0)])!
        orb.draw(in: NSBezierPath(ovalIn: CGRect(x: W / 2 - 70, y: H / 2 + 90, width: 140, height: 140)), relativeCenterPosition: NSPoint(x: -0.2, y: 0.2))
        if let t = f.title { text(t, size: 92, weight: .bold, color: .white, in: CGRect(x: 0, y: H / 2 - 60, width: W, height: 120)) }
        if let s = f.subtitle { text(s, size: 34, weight: .regular, color: NSColor(white: 1, alpha: 0.8), in: CGRect(x: 200, y: H / 2 - 150, width: W - 400, height: 90)) }
    default:
        backdrop()
        menuBar()
        if let w = f.window {
            let s = f.windowSize ?? [1280, 760]
            let r = window(image(w), size: CGSize(width: s[0], height: s[1]), overlay: f.overlay.map(image))
            if let e = f.ink { ink(e, in: r) }
            if let p = f.pointer { pointer(p, in: r) }
        }
        if let i = f.island {
            // Hangs from the notch: 640×140 pt, top centre.
            image(i).draw(in: CGRect(x: W / 2 - 320 * 1.3, y: H - 140 * 1.3, width: 640 * 1.3, height: 140 * 1.3))
        }
        if let b = f.banner {
            let r = CGRect(x: W - 520, y: H - 150, width: 480, height: 92)
            NSColor(white: 0.16, alpha: 0.95).setFill()
            NSBezierPath(roundedRect: r, xRadius: 18, yRadius: 18).fill()
            accent.setFill()
            NSBezierPath(ovalIn: CGRect(x: r.minX + 18, y: r.midY - 18, width: 36, height: 36)).fill()
            text("Wisp · Done", size: 17, weight: .semibold, color: .white, in: CGRect(x: r.minX + 70, y: r.maxY - 38, width: 380, height: 24), align: .left)
            text(b, size: 16, weight: .regular, color: NSColor(white: 1, alpha: 0.85), in: CGRect(x: r.minX + 70, y: r.minY + 12, width: 390, height: 40), align: .left)
        }
    }
    if let c = f.caption { caption(c) }
    NSGraphicsContext.restoreGraphicsState()
    try! px.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: f.out))
}

let frames = try! JSONDecoder().decode([Frame].self, from: Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1])))
for f in frames { render(f) }
print("composed \(frames.count) frames")
