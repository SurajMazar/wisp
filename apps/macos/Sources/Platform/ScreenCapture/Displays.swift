import AppKit
import WispIPC

/// Display metadata in the contract's coordinate space (global points, top-left origin).
@MainActor
public enum Displays {
    /// Height of the primary display — the pivot for AppKit ↔ CoreGraphics conversion.
    public static var primaryHeight: CGFloat { NSScreen.screens.first?.frame.height ?? 0 }

    public static func id(of screen: NSScreen) -> CGDirectDisplayID {
        (screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? NSNumber)?.uint32Value ?? CGMainDisplayID()
    }

    public static func info(for screen: NSScreen) -> DisplayInfo {
        let f = FocusGeometry.cgRect(fromAppKit: screen.frame, primaryHeight: primaryHeight)
        return DisplayInfo(
            frame: Rect(height: f.height, width: f.width, x: f.minX, y: f.minY),
            id: id(of: screen),
            name: screen.localizedName,
            scale: screen.backingScaleFactor
        )
    }

    /// Screen containing an AppKit global point.
    public static func screen(containingAppKit p: CGPoint) -> NSScreen? {
        NSScreen.screens.first { NSMouseInRect(p, $0.frame, false) } ?? NSScreen.main
    }

    /// Screen containing a CoreGraphics global point.
    public static func screen(containingCG p: CGPoint) -> NSScreen? {
        screen(containingAppKit: FocusGeometry.appKitPoint(fromCG: p, primaryHeight: primaryHeight))
    }
}

public extension Rect {
    var cg: CGRect { CGRect(x: x, y: y, width: width, height: height) }
    init(_ r: CGRect) { self.init(height: r.height, width: r.width, x: r.minX, y: r.minY) }
}

public extension Point {
    var cg: CGPoint { CGPoint(x: x, y: y) }
    init(_ p: CGPoint) { self.init(x: p.x, y: p.y) }
}
