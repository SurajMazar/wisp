import AppKit
import CoreGraphics
import Testing
@testable import WispIPC
@testable import WispPlatform

@Suite struct TextRecognizerTests {
    /// Renders two labels into a 2× bitmap, as if captured from `crop`, and reads them back.
    @Test func readsTextInReadingOrderWithGlobalFrames() async throws {
        let crop = CGRect(x: 500, y: 300, width: 400, height: 200)  // global points
        let scale: CGFloat = 2
        let w = Int(crop.width * scale), h = Int(crop.height * scale)
        let ctx = try #require(CGContext(
            data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ))
        ctx.setFillColor(.white)
        ctx.fill(CGRect(x: 0, y: 0, width: w, height: h))
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: false)
        let attrs: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: 44, weight: .semibold), .foregroundColor: NSColor.black]
        // Bitmap y is up: "Delete files" near the top, "Cancel" near the bottom.
        ("Delete files" as NSString).draw(at: CGPoint(x: 40, y: 280), withAttributes: attrs)
        ("Cancel" as NSString).draw(at: CGPoint(x: 40, y: 60), withAttributes: attrs)
        NSGraphicsContext.restoreGraphicsState()
        let image = try #require(ctx.makeImage())

        let text = try #require(await TextRecognizer.recognize(image, crop: crop))
        #expect(text.lines.map(\.text) == ["Delete files", "Cancel"])
        let top = try #require(text.lines.first).frame
        // "Delete files" sits in the upper half of the crop, in global coordinates.
        #expect(top.x >= crop.minX && top.x < crop.minX + 60)
        #expect(top.y >= crop.minY && top.y < crop.midY)
        let bottom = try #require(text.lines.last).frame
        #expect(bottom.y > crop.midY && bottom.y + bottom.height <= crop.maxY + 1)
    }
}
