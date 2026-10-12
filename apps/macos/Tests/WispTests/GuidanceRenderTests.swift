import AppKit
import Testing
@testable import WispIPC
@testable import WispUI

/// Renders the on-screen marks over a mock window (set WISP_GUIDANCE_SNAPSHOTS=<dir> to save PNGs).
@MainActor
@Suite struct GuidanceRenderTests {
    private let size = CGSize(width: 900, height: 520)

    /// A fake app window with a few labelled buttons (CG top-left coordinates).
    private let buttons: [(String, CGRect)] = [
        ("Save", CGRect(x: 640, y: 440, width: 90, height: 30)),
        ("Export…", CGRect(x: 520, y: 440, width: 100, height: 30)),
        ("Total: $1,240.00", CGRect(x: 60, y: 300, width: 200, height: 22)),
        ("Settings", CGRect(x: 60, y: 60, width: 110, height: 30)),
    ]

    private func render(_ annotations: [Annotation], name: String) throws -> CGImage {
        let view = GuidanceView(frame: NSRect(origin: .zero, size: size))
        view.tint = NSColor(srgbHex: 0x7C6CF2)
        view.map = { p in CGPoint(x: p.x, y: self.size.height - p.y) }
        view.layer?.frame = view.bounds
        view.draw(annotations: annotations)
        let scale: CGFloat = 2
        let ctx = try #require(CGContext(data: nil, width: Int(size.width * scale), height: Int(size.height * scale), bitsPerComponent: 8,
                                         bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                         bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.scaleBy(x: scale, y: scale)
        // Backdrop: a light window with buttons.
        ctx.setFillColor(NSColor(white: 0.96, alpha: 1).cgColor)
        ctx.fill(CGRect(origin: .zero, size: size))
        for (title, r) in buttons {
            let rr = CGRect(x: r.minX, y: size.height - r.maxY, width: r.width, height: r.height)
            ctx.setFillColor(NSColor.white.cgColor)
            ctx.addPath(CGPath(roundedRect: rr, cornerWidth: 6, cornerHeight: 6, transform: nil))
            ctx.fillPath()
            ctx.setStrokeColor(NSColor(white: 0.8, alpha: 1).cgColor)
            ctx.addPath(CGPath(roundedRect: rr, cornerWidth: 6, cornerHeight: 6, transform: nil))
            ctx.strokePath()
            NSGraphicsContext.saveGraphicsState()
            NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: false)
            (title as NSString).draw(at: CGPoint(x: rr.minX + 10, y: rr.minY + 7),
                                     withAttributes: [.font: NSFont.systemFont(ofSize: 13), .foregroundColor: NSColor.black])
            NSGraphicsContext.restoreGraphicsState()
        }
        try #require(view.layer).render(in: ctx)
        let image = try #require(ctx.makeImage())
        if let dir = ProcessInfo.processInfo.environment["WISP_GUIDANCE_SNAPSHOTS"] {
            try NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:])?
                .write(to: URL(fileURLWithPath: dir).appendingPathComponent("\(name).png"))
        }
        return image
    }

    private func rect(_ r: CGRect, pad: Double = 0) -> Rect {
        Rect(height: r.height + 2 * pad, width: r.width + 2 * pad, x: r.minX - pad, y: r.minY - pad)
    }

    /// The tint must actually appear where a mark is drawn, and nowhere near unmarked buttons.
    private func tinted(_ image: CGImage, at p: CGPoint) -> Bool {
        let data = CFDataGetBytePtr(image.dataProvider!.data!)!
        let x = Int(p.x * 2), y = Int(p.y * 2)
        let i = y * image.bytesPerRow + x * 4
        let (r, g, b) = (Int(data[i]), Int(data[i + 1]), Int(data[i + 2]))
        return b > r + 40 && b > g + 40  // bluish-purple
    }

    @Test func answerMarksAppearWhereAsked() throws {
        let save = buttons[0].1, export = buttons[1].1, total = buttons[2].1, settings = buttons[3].1
        let image = try render([
            .circle(CircleAnnotation(center: Point(x: save.midX, y: save.midY), label: "Save", radius: hypot(save.width, save.height) / 2 + 8)),
            .arrow(ArrowAnnotation(from: Point(x: export.minX - 110, y: export.minY - 82), label: "Export…", to: Point(x: export.minX - 2, y: export.minY - 2))),
            .highlight(RectAnnotation(label: nil, rect: Rect(height: 3, width: total.width, x: total.minX, y: total.maxY + 1))),
            .highlight(RectAnnotation(label: "Settings", rect: rect(settings, pad: 4))),
        ], name: "guidance-marks")
        #expect(tinted(image, at: CGPoint(x: save.midX, y: save.midY - (hypot(save.width, save.height) / 2 + 8))), "circle drawn around Save")
        #expect(tinted(image, at: CGPoint(x: total.midX, y: total.maxY + 2.5)), "underline drawn under the total")
        #expect(tinted(image, at: CGPoint(x: settings.minX - 4, y: settings.midY)), "outline drawn around Settings")
        #expect(!tinted(image, at: CGPoint(x: 450, y: 200)), "empty areas stay untouched")
    }

    @Test func spotlightDimsEverythingElse() throws {
        let save = buttons[0].1
        let image = try render([.spotlight(RectAnnotation(label: "Save", rect: rect(save, pad: 8)))], name: "guidance-spotlight")
        let data = CFDataGetBytePtr(image.dataProvider!.data!)!
        let lum = { (p: CGPoint) -> Int in let i = Int(p.y * 2) * image.bytesPerRow + Int(p.x * 2) * 4; return Int(data[i]) }
        #expect(lum(CGPoint(x: 450, y: 200)) < 200, "outside is dimmed")
        #expect(lum(CGPoint(x: save.minX + 3, y: save.minY + 3)) > 230, "the target stays bright")
    }

    @Test func agentActionPreviewMarksTheTarget() throws {
        let save = buttons[0].1
        let image = try render([.circle(CircleAnnotation(center: Point(x: save.midX, y: save.midY), label: "Click “Save”", radius: 22))],
                               name: "guidance-agent-preview")
        #expect(tinted(image, at: CGPoint(x: save.midX + 22, y: save.midY)), "circle on the click target")
    }
}
