import CoreGraphics

/// Coordinate conversion and focus geometry. Pure functions, unit-tested.
///
/// Two global spaces exist on macOS:
/// * **AppKit** — origin at the bottom-left of the primary display, y up (NSEvent, NSWindow).
/// * **CoreGraphics** — origin at the top-left of the primary display, y down (CGWindow, ScreenCaptureKit, IPC).
/// Both are in points; a display's backing scale converts to pixels.
public enum FocusGeometry {
    public static func cgPoint(fromAppKit p: CGPoint, primaryHeight: CGFloat) -> CGPoint {
        CGPoint(x: p.x, y: primaryHeight - p.y)
    }

    public static func appKitPoint(fromCG p: CGPoint, primaryHeight: CGFloat) -> CGPoint {
        CGPoint(x: p.x, y: primaryHeight - p.y)
    }

    public static func cgRect(fromAppKit r: CGRect, primaryHeight: CGFloat) -> CGRect {
        CGRect(x: r.minX, y: primaryHeight - r.maxY, width: r.width, height: r.height)
    }

    public static func appKitRect(fromCG r: CGRect, primaryHeight: CGFloat) -> CGRect {
        CGRect(x: r.minX, y: primaryHeight - r.maxY, width: r.width, height: r.height)
    }

    /// Rect spanned by a drag from `a` to `b`, any direction.
    public static func rect(from a: CGPoint, to b: CGPoint) -> CGRect {
        CGRect(x: min(a.x, b.x), y: min(a.y, b.y), width: abs(b.x - a.x), height: abs(b.y - a.y))
    }

    public static func bounds(of points: [CGPoint]) -> CGRect? {
        guard let f = points.first else { return nil }
        return points.dropFirst().reduce(CGRect(origin: f, size: .zero)) { $0.union(CGRect(origin: $1, size: .zero)) }
    }

    /// Selection plus `margin` of context, clipped to the display (spec §34 step 5).
    public static func crop(selection: CGRect, margin: CGFloat, display: CGRect) -> CGRect {
        selection.insetBy(dx: -margin, dy: -margin).intersection(display)
    }

    /// Square of `side` centered on `point`, shifted (not clipped) to stay inside the display.
    public static func pointRegion(center: CGPoint, side: CGFloat, display: CGRect) -> CGRect {
        let w = min(side, display.width), h = min(side, display.height)
        var r = CGRect(x: center.x - w / 2, y: center.y - h / 2, width: w, height: h)
        r.origin.x = min(max(r.minX, display.minX), display.maxX - w)
        r.origin.y = min(max(r.minY, display.minY), display.maxY - h)
        return r
    }

    /// `rect` relative to its display's origin (ScreenCaptureKit `sourceRect`), in points.
    public static func local(_ rect: CGRect, in display: CGRect) -> CGRect {
        rect.offsetBy(dx: -display.minX, dy: -display.minY)
    }

    /// Pixel size of `rect` at `scale`, rounded.
    public static func pixelSize(_ rect: CGRect, scale: CGFloat) -> (width: Int, height: Int) {
        (Int((rect.width * scale).rounded()), Int((rect.height * scale).rounded()))
    }

    /// Whether a drag is big enough to be a deliberate selection rather than a click.
    public static func isDrag(from a: CGPoint, to b: CGPoint, threshold: CGFloat = 6) -> Bool {
        hypot(b.x - a.x, b.y - a.y) >= threshold
    }
}
