import CoreGraphics
import Foundation

/// Where the buddy cursor should be, frame by frame. Pure value type so it can be unit-tested.
///
/// Coordinates are AppKit global points (origin bottom-left of the main display, y up).
///
/// * The buddy is a second cursor that trails the real one: its tip rests diagonally below-right
///   of the pointer's tip, flipping inward near screen edges so it never leaves the pointer's display.
/// * It always follows — it never stops, dodges or waits to be clicked (it can't be clicked).
/// * It glides with a spring; `smoothing` 1 means "snap" (Reduce Motion).
public struct BuddyMotion: Sendable {
    public var gap: CGFloat = 14

    public private(set) var center: CGPoint?

    public init() {}

    /// Resting center of a buddy window of `size` beside `cursor`, kept inside `bounds`.
    public static func anchor(cursor: CGPoint, size: CGSize, bounds: CGRect, gap: CGFloat) -> CGPoint {
        let dx = gap + size.width / 2
        let dy = gap + size.height / 2
        var x = cursor.x + dx
        var y = cursor.y - dy  // below the pointer tip
        if x + size.width / 2 > bounds.maxX { x = cursor.x - dx }
        if y - size.height / 2 < bounds.minY { y = cursor.y + dy }
        x = min(max(x, bounds.minX + size.width / 2), bounds.maxX - size.width / 2)
        y = min(max(y, bounds.minY + size.height / 2), bounds.maxY - size.height / 2)
        return CGPoint(x: x, y: y)
    }

    /// Advance one frame. Returns the new center.
    public mutating func step(cursor: CGPoint, size: CGSize, bounds: CGRect, smoothing: CGFloat) -> CGPoint {
        let target = Self.anchor(cursor: cursor, size: size, bounds: bounds, gap: gap)
        guard var c = center else {
            center = target
            return target
        }
        let k = min(max(smoothing, 0.05), 1)
        c.x += (target.x - c.x) * k
        c.y += (target.y - c.y) * k
        if hypot(target.x - c.x, target.y - c.y) < 0.3 { c = target }
        center = c
        return c
    }

    /// Jump straight to the resting spot (screen change, re-show).
    public mutating func reset() {
        center = nil
    }
}
