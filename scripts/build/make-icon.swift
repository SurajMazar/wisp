// Renders Wisp's app icon (a luminous core with a trailing halo on a deep, quiet ground)
// and writes apps/macos/Resources/AppIcon.icns. Run: swift scripts/build/make-icon.swift
import AppKit

let sizes = [16, 32, 64, 128, 256, 512, 1024]
let root = URL(fileURLWithPath: CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : ".")
let iconset = root.appendingPathComponent("build/AppIcon.iconset")
try? FileManager.default.removeItem(at: iconset)
try FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)

func render(_ px: Int) -> Data {
    let s = CGFloat(px)
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: px, pixelsHigh: px, bitsPerSample: 8,
                               samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                               bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    let ctx = NSGraphicsContext.current!.cgContext

    // macOS icon grid: 824/1024 body with ~185/1024 corner radius.
    let inset = s * 100 / 1024
    let body = CGRect(x: inset, y: inset, width: s - 2 * inset, height: s - 2 * inset)
    let path = CGPath(roundedRect: body, cornerWidth: s * 185 / 1024, cornerHeight: s * 185 / 1024, transform: nil)
    ctx.saveGState()
    ctx.setShadow(offset: CGSize(width: 0, height: -s * 0.012), blur: s * 0.03, color: NSColor.black.withAlphaComponent(0.35).cgColor)
    ctx.addPath(path); ctx.setFillColor(NSColor.black.cgColor); ctx.fillPath()
    ctx.restoreGState()

    ctx.saveGState()
    ctx.addPath(path); ctx.clip()
    let space = CGColorSpaceCreateDeviceRGB()
    let ground = CGGradient(colorsSpace: space, colors: [
        NSColor(srgbRed: 0.13, green: 0.12, blue: 0.20, alpha: 1).cgColor,
        NSColor(srgbRed: 0.06, green: 0.06, blue: 0.09, alpha: 1).cgColor,
    ] as CFArray, locations: [0, 1])!
    ctx.drawLinearGradient(ground, start: CGPoint(x: s / 2, y: body.maxY), end: CGPoint(x: s / 2, y: body.minY), options: [])

    let c = CGPoint(x: s * 0.5, y: s * 0.5)
    // Halo bloom.
    let bloom = CGGradient(colorsSpace: space, colors: [
        NSColor(srgbRed: 0.55, green: 0.49, blue: 0.96, alpha: 0.55).cgColor,
        NSColor(srgbRed: 0.37, green: 0.78, blue: 0.95, alpha: 0.18).cgColor,
        NSColor(srgbRed: 0.37, green: 0.78, blue: 0.95, alpha: 0).cgColor,
    ] as CFArray, locations: [0, 0.45, 1])!
    ctx.drawRadialGradient(bloom, startCenter: c, startRadius: 0, endCenter: c, endRadius: s * 0.36, options: [])

    // Trailing arc.
    ctx.setLineCap(.butt)
    // Orbit: a crescent on the trailing side, tapering in opacity like a moving light.
    let segments = 28
    for i in 0..<segments {
        let t = CGFloat(i) / CGFloat(segments)
        let a0 = -CGFloat.pi * 0.42 + t * CGFloat.pi * 0.9
        let a1 = a0 + CGFloat.pi * 0.9 / CGFloat(segments) + 0.004
        ctx.setLineWidth(s * (0.012 + 0.02 * t))
        ctx.setStrokeColor(NSColor(srgbRed: 0.62 - 0.2 * t, green: 0.62 + 0.15 * t, blue: 0.98, alpha: 0.15 + 0.75 * t).cgColor)
        ctx.addArc(center: c, radius: s * 0.235, startAngle: a0, endAngle: a1, clockwise: false)
        ctx.strokePath()
    }
    // Rounded head of the orbit.
    let head = -CGFloat.pi * 0.42 + CGFloat.pi * 0.9
    let hr = s * 0.016
    let hp = CGPoint(x: c.x + cos(head) * s * 0.235, y: c.y + sin(head) * s * 0.235)
    ctx.setFillColor(NSColor(srgbRed: 0.42, green: 0.77, blue: 0.98, alpha: 0.9).cgColor)
    ctx.fillEllipse(in: CGRect(x: hp.x - hr, y: hp.y - hr, width: hr * 2, height: hr * 2))

    // Core.
    let core = CGGradient(colorsSpace: space, colors: [
        NSColor.white.cgColor,
        NSColor(srgbRed: 0.80, green: 0.76, blue: 1.0, alpha: 1).cgColor,
        NSColor(srgbRed: 0.55, green: 0.49, blue: 0.96, alpha: 0).cgColor,
    ] as CFArray, locations: [0, 0.55, 1])!
    ctx.drawRadialGradient(core, startCenter: CGPoint(x: c.x - s * 0.015, y: c.y + s * 0.015), startRadius: 0,
                           endCenter: c, endRadius: s * 0.11, options: [])
    ctx.restoreGState()

    NSGraphicsContext.restoreGraphicsState()
    return rep.representation(using: .png, properties: [:])!
}

for size in sizes {
    let png = render(size)
    if size <= 512 { try png.write(to: iconset.appendingPathComponent("icon_\(size)x\(size).png")) }
    if size >= 32 { try png.write(to: iconset.appendingPathComponent("icon_\(size / 2)x\(size / 2)@2x.png")) }
}
print(iconset.path)
