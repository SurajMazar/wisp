import CoreGraphics
import Foundation

/// Where the buddy should be, frame by frame. Pure value type so it can be unit-tested.
///
/// Coordinates are AppKit global points (origin bottom-left of the main display, y up).
///
/// * The buddy rests diagonally below-right of the pointer, flipping to the other side near
///   screen edges so it never leaves the display the pointer is on (spec §27).
/// * It follows with a spring; `smoothing` 1 means "snap" (Reduced Motion).
/// * When the pointer approaches it *slowly* — someone reaching for it — it holds still so it can
///   be clicked. Fast pointer movement is never mistaken for an approach.
public struct BuddyMotion: Sendable {
    public var gap: CGFloat = 14
    public var approachSpeedLimit: CGFloat = 320  // pt/s
    public var leash: CGFloat = 90

    public private(set) var center: CGPoint?
    public private(set) var isHeld = false
    private var lastCursor: CGPoint?
    private var lastTime: TimeInterval?

    public init() {}

    /// Resting spot beside `cursor` for a buddy of `size`, kept inside `bounds`.
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
    public mutating func step(cursor: CGPoint, time: TimeInterval, size: CGSize, bounds: CGRect, smoothing: CGFloat) -> CGPoint {
        let target = Self.anchor(cursor: cursor, size: size, bounds: bounds, gap: gap)
        guard var c = center else {
            center = target
            lastCursor = cursor
            lastTime = time
            return target
        }

        let dist = hypot(cursor.x - c.x, cursor.y - c.y)
        var speed: CGFloat = 0
        var approaching = false
        if let last = lastCursor, let t0 = lastTime, time > t0 {
            speed = hypot(cursor.x - last.x, cursor.y - last.y) / CGFloat(time - t0)
            let previousDist = hypot(last.x - c.x, last.y - c.y)
            approaching = dist < previousDist - 0.25
        }
        lastCursor = cursor
        lastTime = time

        let hovering = dist <= max(size.width, size.height) / 2 + 4
        let reaching = approaching && speed < approachSpeedLimit && dist < leash
        let stillNear = isHeld && speed < 1 && dist < leash
        isHeld = hovering || reaching || stillNear
        if isHeld { return c }

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
        lastCursor = nil
        lastTime = nil
        isHeld = false
    }
}
