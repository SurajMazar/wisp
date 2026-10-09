import CoreGraphics
import WispIPC

/// Outlines for the buddy's shapes. All are drawn in a square of `side` points with y up, and
/// "point" toward the top-left — toward the real cursor, which the buddy trails from below-right.
enum BuddyShapes {
    /// Design grid (y down from the top-left) for the arrowhead shapes.
    static let grid: CGFloat = 20

    /// Wisp's arrowhead: a rounded triangle with a notched base (tip toward the top-left).
    static func wisp(in rect: CGRect) -> CGPath {
        let pts: [(CGPoint, CGFloat)] = [
            (CGPoint(x: 1.0, y: 1.0), 1.4),    // tip
            (CGPoint(x: 19.4, y: 7.4), 2.0),   // right base corner
            (CGPoint(x: 9.2, y: 9.2), 1.2),    // notch (deep V)
            (CGPoint(x: 7.4, y: 19.4), 2.0),   // left base corner
        ]
        return rounded(pts, in: rect)
    }

    /// A classic pointer arrow.
    static func pointer(in rect: CGRect) -> CGPath {
        let pts: [(CGPoint, CGFloat)] = [
            (CGPoint(x: 2, y: 1), 0.8), (CGPoint(x: 2, y: 17.5), 0.8), (CGPoint(x: 6.2, y: 13.8), 0.6),
            (CGPoint(x: 9.1, y: 20.2), 0.8), (CGPoint(x: 11.6, y: 19.1), 0.8), (CGPoint(x: 8.8, y: 12.9), 0.6),
            (CGPoint(x: 14.4, y: 12.9), 0.8),
        ]
        return rounded(pts, in: rect)
    }

    static func dot(in rect: CGRect) -> CGPath {
        let d = rect.width * 0.62
        return CGPath(ellipseIn: CGRect(x: rect.minX + rect.width * 0.08, y: rect.maxY - rect.height * 0.08 - d, width: d, height: d), transform: nil)
    }

    static func path(_ shape: CursorShape, in rect: CGRect) -> CGPath {
        switch shape {
        case .wisp: wisp(in: rect)
        case .pointer: pointer(in: rect)
        case .dot: dot(in: rect)
        }
    }

    /// Polygon with per-vertex corner radii, mapped from the y-down grid into `rect` (y up).
    static func rounded(_ pts: [(CGPoint, CGFloat)], in rect: CGRect) -> CGPath {
        let s = rect.width / grid
        let map = { (p: CGPoint) in CGPoint(x: rect.minX + p.x * s, y: rect.maxY - p.y * s) }
        let n = pts.count
        let path = CGMutablePath()
        let mid = { (a: CGPoint, b: CGPoint) in CGPoint(x: (a.x + b.x) / 2, y: (a.y + b.y) / 2) }
        path.move(to: mid(map(pts[n - 1].0), map(pts[0].0)))
        for i in 0..<n {
            let p = map(pts[i].0), next = map(pts[(i + 1) % n].0)
            path.addArc(tangent1End: p, tangent2End: mid(p, next), radius: pts[i].1 * s)
        }
        path.closeSubpath()
        return path
    }
}
