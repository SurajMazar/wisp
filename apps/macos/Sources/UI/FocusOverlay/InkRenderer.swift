import AppKit
import CoreGraphics
import WispPlatform

/// Draws the user's pen strokes onto a captured image. The overlay itself is excluded from
/// capture, so without this the model would see the area but not what was marked in it.
public enum InkRenderer {
    /// `strokes` are global CG points (top-left origin); `crop` is the captured rect in the same space.
    public static func burn(_ strokes: [[CGPoint]], into image: CGImage, crop: CGRect, color: NSColor) -> CGImage? {
        guard crop.width > 0, crop.height > 0,
              let ctx = CGContext(data: nil, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: 0,
                                  space: CGColorSpace(name: CGColorSpace.sRGB) ?? CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return nil }
        let sx = CGFloat(image.width) / crop.width, sy = CGFloat(image.height) / crop.height
        ctx.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        // CG points → image pixels (bitmap contexts have a bottom-left origin).
        let px = { (p: CGPoint) in CGPoint(x: (p.x - crop.minX) * sx, y: CGFloat(image.height) - (p.y - crop.minY) * sy) }
        ctx.setStrokeColor(color.cgColor)
        ctx.setFillColor(color.cgColor)
        ctx.setLineWidth(4 * sx)
        ctx.setLineCap(.round)
        ctx.setLineJoin(.round)
        for s in strokes where !s.isEmpty {
            let pts = s.map(px)
            if let box = FocusGeometry.bounds(of: s), max(box.width, box.height) < 6, let c = pts.last {
                let r = 5 * sx
                ctx.fillEllipse(in: CGRect(x: c.x - r, y: c.y - r, width: 2 * r, height: 2 * r))
            } else {
                ctx.addLines(between: pts)
                ctx.strokePath()
            }
        }
        return ctx.makeImage()
    }
}
