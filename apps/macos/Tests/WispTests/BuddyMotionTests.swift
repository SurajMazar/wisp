import CoreGraphics
import Foundation
import AppKit
import Testing
@testable import WispUI

@Suite struct BuddyMotionTests {
    let size = CGSize(width: 48, height: 48)
    let screen = CGRect(x: 0, y: 0, width: 1440, height: 900)

    @Test func restsBelowRightOfPointer() {
        let a = BuddyMotion.anchor(cursor: CGPoint(x: 500, y: 500), size: size, bounds: screen, gap: 14)
        #expect(a.x > 500 && a.y < 500)
    }

    @Test func flipsAtRightAndBottomEdgesAndStaysOnScreen() {
        let right = BuddyMotion.anchor(cursor: CGPoint(x: 1430, y: 500), size: size, bounds: screen, gap: 14)
        #expect(right.x < 1430)
        let bottom = BuddyMotion.anchor(cursor: CGPoint(x: 500, y: 10), size: size, bounds: screen, gap: 14)
        #expect(bottom.y > 10)
        for p in [CGPoint(x: 0, y: 0), CGPoint(x: 1440, y: 900), CGPoint(x: 1439, y: 1)] {
            let a = BuddyMotion.anchor(cursor: p, size: size, bounds: screen, gap: 14)
            #expect(screen.insetBy(dx: size.width / 2 - 0.01, dy: size.height / 2 - 0.01).contains(a))
        }
    }

    @Test func secondaryDisplayWithNegativeOrigin() {
        let left = CGRect(x: -1920, y: 0, width: 1920, height: 1080)
        let a = BuddyMotion.anchor(cursor: CGPoint(x: -5, y: 540), size: size, bounds: left, gap: 14)
        #expect(left.contains(a))
        #expect(a.x < -5, "flips left at the shared edge instead of crossing onto the other display")
    }

    @Test func followsQuickMovementAndConverges() {
        var m = BuddyMotion()
        var t: TimeInterval = 0
        _ = m.step(cursor: CGPoint(x: 100, y: 500), time: t, size: size, bounds: screen, smoothing: 0.3)
        var c = CGPoint.zero
        for i in 1...60 {
            t += 1.0 / 60
            c = m.step(cursor: CGPoint(x: 100 + CGFloat(min(i, 10)) * 40, y: 500), time: t, size: size, bounds: screen, smoothing: 0.3)
        }
        let target = BuddyMotion.anchor(cursor: CGPoint(x: 500, y: 500), size: size, bounds: screen, gap: 14)
        #expect(hypot(c.x - target.x, c.y - target.y) < 0.5)
        #expect(!m.isHeld)
    }

    @Test func holdsStillWhenReachedForSlowlySoItCanBeClicked() {
        var m = BuddyMotion()
        var t: TimeInterval = 0
        let start = m.step(cursor: CGPoint(x: 500, y: 500), time: t, size: size, bounds: screen, smoothing: 0.3)
        var cursor = CGPoint(x: 500, y: 500)
        for _ in 0..<40 {
            t += 1.0 / 60
            // Slow approach (~120 pt/s) toward the buddy.
            let dx = start.x - cursor.x, dy = start.y - cursor.y
            let len = hypot(dx, dy)
            guard len > 1 else { break }
            cursor.x += dx / len * 2
            cursor.y += dy / len * 2
            let c = m.step(cursor: cursor, time: t, size: size, bounds: screen, smoothing: 0.3)
            #expect(c == start, "buddy ran away from a pointer reaching for it")
        }
        #expect(m.isHeld)
    }

    @Test func fastMovementTowardItIsNotAReach() {
        var m = BuddyMotion()
        _ = m.step(cursor: CGPoint(x: 500, y: 500), time: 0, size: size, bounds: screen, smoothing: 0.3)
        // 1,200 pt/s diagonal sweep through the buddy's position.
        _ = m.step(cursor: CGPoint(x: 520, y: 480), time: 1.0 / 60, size: size, bounds: screen, smoothing: 0.3)
        _ = m.step(cursor: CGPoint(x: 560, y: 440), time: 2.0 / 60, size: size, bounds: screen, smoothing: 0.3)
        #expect(!m.isHeld)
    }

    @Test func snapsWithReducedMotion() {
        var m = BuddyMotion()
        _ = m.step(cursor: CGPoint(x: 100, y: 500), time: 0, size: size, bounds: screen, smoothing: 1)
        let c = m.step(cursor: CGPoint(x: 900, y: 300), time: 1.0 / 60, size: size, bounds: screen, smoothing: 1)
        #expect(c == BuddyMotion.anchor(cursor: CGPoint(x: 900, y: 300), size: size, bounds: screen, gap: 14))
    }
}

@Suite @MainActor struct PanelPlacementTests {
    @Test func panelOpensBesideAnchorAndStaysOnScreen() {
        guard let v = NSScreen.main?.visibleFrame else { return }
        let size = NSSize(width: 440, height: 300)
        for anchor in [
            NSRect(x: v.midX, y: v.midY, width: 48, height: 48),
            NSRect(x: v.maxX - 50, y: v.maxY - 50, width: 48, height: 48),
            NSRect(x: v.minX, y: v.minY, width: 48, height: 48),
        ] {
            let f = FloatingAssistantPanel.frame(size: size, beside: anchor)
            #expect(v.contains(f), "panel off screen for anchor \(anchor)")
            #expect(!f.intersects(anchor.insetBy(dx: 8, dy: 8)), "panel covers the buddy")
        }
    }
}
