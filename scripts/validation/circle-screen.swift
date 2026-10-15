// Draws the test screen for circle-check.ts: a document window with labelled toolbar buttons,
// unlabelled icons and plenty of body text, with a purple pen loop around one target.
//   swift circle-screen.swift <out.jpg> <x> <y> <w> <h>   (loop bounds, points, top-left origin)
import AppKit

let W = 1280.0, H = 720.0, S = 2.0
let a = CommandLine.arguments
let loop = CGRect(x: Double(a[2])!, y: Double(a[3])!, width: Double(a[4])!, height: Double(a[5])!)
let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(W * S), pixelsHigh: Int(H * S), bitsPerSample: 8,
                           samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                           bytesPerRow: 0, bitsPerPixel: 0)!
let ctx = NSGraphicsContext(bitmapImageRep: rep)!
NSGraphicsContext.current = ctx
// Top-left origin in points.
ctx.cgContext.translateBy(x: 0, y: H * S)
ctx.cgContext.scaleBy(x: S, y: -S)
let flipped = NSGraphicsContext(cgContext: ctx.cgContext, flipped: true)
NSGraphicsContext.current = flipped

func text(_ s: String, _ x: Double, _ y: Double, size: Double = 15, weight: NSFont.Weight = .regular, color: NSColor = .black) {
    (s as NSString).draw(at: CGPoint(x: x, y: y), withAttributes: [.font: NSFont.systemFont(ofSize: size, weight: weight), .foregroundColor: color])
}

NSColor.white.setFill(); CGRect(x: 0, y: 0, width: W, height: H).fill()
NSColor(white: 0.94, alpha: 1).setFill(); CGRect(x: 0, y: 0, width: W, height: 40).fill()
text("Q3 Report — Draft", 540, 10, size: 15, weight: .semibold)

// Toolbar buttons (labelled).
for (i, label) in ["Archive", "Export", "Share", "Delete"].enumerated() {
    let r = CGRect(x: 40 + Double(i) * 130, y: 60, width: 110, height: 36)
    let p = NSBezierPath(roundedRect: r, xRadius: 8, yRadius: 8)
    NSColor(white: 0.97, alpha: 1).setFill(); p.fill()
    NSColor(white: 0.75, alpha: 1).setStroke(); p.lineWidth = 1.5; p.stroke()
    text(label, r.minX + 26, r.minY + 9, size: 15)
}

// Icons (no labels): gear, trash, star, bell, centred at y = 160.
NSColor(white: 0.25, alpha: 1).setFill()
NSColor(white: 0.25, alpha: 1).setStroke()
// Gear at x 80.
let gear = NSBezierPath()
for k in 0..<16 {
    let r = k % 2 == 0 ? 22.0 : 15.0
    let t = Double(k) / 16 * 2 * .pi
    let pt = CGPoint(x: 80 + r * cos(t), y: 160 + r * sin(t))
    if k == 0 { gear.move(to: pt) } else { gear.line(to: pt) }
}
gear.close(); gear.fill()
NSColor.white.setFill(); NSBezierPath(ovalIn: CGRect(x: 72, y: 152, width: 16, height: 16)).fill()
NSColor(white: 0.25, alpha: 1).setFill()
// Trash can at x 200.
CGRect(x: 184, y: 142, width: 32, height: 5).fill()
CGRect(x: 194, y: 137, width: 12, height: 5).fill()
let can = NSBezierPath(); can.move(to: CGPoint(x: 187, y: 149)); can.line(to: CGPoint(x: 213, y: 149))
can.line(to: CGPoint(x: 209, y: 182)); can.line(to: CGPoint(x: 191, y: 182)); can.close(); can.fill()
// Star at x 320.
let star = NSBezierPath()
for k in 0..<10 {
    let r = k % 2 == 0 ? 22.0 : 9.0
    let t = -Double.pi / 2 + Double(k) * .pi / 5
    let pt = CGPoint(x: 320 + r * cos(t), y: 162 + r * sin(t))
    if k == 0 { star.move(to: pt) } else { star.line(to: pt) }
}
star.close(); NSColor.systemYellow.setFill(); star.fill()
// Bell at x 440.
NSColor(white: 0.25, alpha: 1).setFill()
let bell = NSBezierPath()
bell.move(to: CGPoint(x: 424, y: 176)); bell.curve(to: CGPoint(x: 440, y: 140), controlPoint1: CGPoint(x: 428, y: 150), controlPoint2: CGPoint(x: 428, y: 140))
bell.curve(to: CGPoint(x: 456, y: 176), controlPoint1: CGPoint(x: 452, y: 140), controlPoint2: CGPoint(x: 452, y: 150)); bell.close(); bell.fill()
NSBezierPath(ovalIn: CGRect(x: 435, y: 177, width: 10, height: 8)).fill()

// Body text.
let body = [
    "Revenue grew 18% quarter over quarter, driven by the enterprise plan and lower churn.",
    "Gross margin held at 71% despite higher hosting costs in the new European region.",
    "Hiring slowed in September; the support team is now fully staffed for the holidays.",
    "Next quarter: ship usage-based billing, renew the data-centre contract, close two deals.",
    "Risks: currency swings, a delayed SOC 2 audit, and one large customer up for renewal.",
    "Appendix A lists every metric with its definition; Appendix B has the raw numbers.",
]
for (i, l) in body.enumerated() { text(l, 40, 240 + Double(i) * 34, size: 16) }

// The user's pen loop.
let ink = NSBezierPath(ovalIn: loop)
ink.lineWidth = 4
NSColor(srgbRed: 0.49, green: 0.42, blue: 0.95, alpha: 1).setStroke()
ink.stroke()

try! rep.representation(using: .jpeg, properties: [.compressionFactor: 0.9])!.write(to: URL(fileURLWithPath: a[1]))
