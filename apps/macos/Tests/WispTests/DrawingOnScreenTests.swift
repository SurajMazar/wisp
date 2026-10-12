import AppKit
import Testing
@testable import WispIPC
@testable import WispUI

/// Drawing on the real display (opt-in: WISP_ONSCREEN=1 — it shows the overlay for a moment).
/// Physical input can't be simulated here, so it checks what decides where a real click goes —
/// the window server's own view of which window is on top at the pointer — then draws through the
/// real window with events and photographs the screen.
@MainActor
@Suite(.enabled(if: ProcessInfo.processInfo.environment["WISP_ONSCREEN"] == "1"))
struct DrawingOnScreenTests {
    @Test func drawingWhileHoldingWorksOnTheRealScreen() async throws {
        let overlay = FocusOverlayController()
        var result: FocusOverlayController.Result?
        overlay.begin(style: .lasso, dim: false, accent: NSColor(srgbHex: 0x7C6CF2), showHint: true, ink: true) { result = $0 }
        defer { overlay.cancel() }
        try await Task.sleep(for: .milliseconds(400))  // let it appear

        let screen = try #require(NSScreen.main)
        let panel = try #require(overlay.windows.first { $0.screen == screen || $0.frame == screen.frame })
        #expect(panel.isVisible)
        #expect(!panel.ignoresMouseEvents, "claims clicks, even over transparent pixels")
        #expect(!panel.isKeyWindow && !NSApp.isActive, "never takes focus (the earlier jumps-back-to-Wisp bug)")

        // Where a real click would go: the front-most window at the screen's centre, per the
        // window server (CG coordinates, top-left origin).
        let center = CGPoint(x: screen.frame.midX, y: screen.frame.maxY - screen.frame.midY + screen.frame.minY)
        let me = ProcessInfo.processInfo.processIdentifier
        let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
        let front = list.first { w in
            guard let b = w[kCGWindowBounds as String] as? NSDictionary,
                  let r = CGRect(dictionaryRepresentation: b) else { return false }
            return r.contains(CGPoint(x: screen.frame.midX, y: center.y)) && (w[kCGWindowAlpha as String] as? Double ?? 1) > 0
        }
        #expect(front?[kCGWindowOwnerPID as String] as? pid_t == me, "the drawing layer is on top of every app at the pointer")
        #expect(front?[kCGWindowNumber as String] as? Int == panel.windowNumber)

        // Draw a circle through the real window.
        let view = try #require(panel.contentView as? FocusOverlayView)
        let c = CGPoint(x: view.bounds.midX, y: view.bounds.midY)
        func send(_ type: NSEvent.EventType, _ p: CGPoint) {
            let e = NSEvent.mouseEvent(with: type, location: p, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                       windowNumber: panel.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
            panel.sendEvent(e)
        }
        let points = (0...36).map { i -> CGPoint in
            let a = Double(i) / 36 * 2 * .pi
            return CGPoint(x: c.x + 140 * cos(a), y: c.y + 90 * sin(a))
        }
        send(.leftMouseDown, points[0])
        for p in points.dropFirst() {
            send(.leftMouseDragged, p)
            try await Task.sleep(for: .milliseconds(8))
        }
        send(.leftMouseUp, points.last!)
        try await Task.sleep(for: .milliseconds(300))
        #expect(view.strokes.count == 1, "the stroke stays after lifting, ready for more")

        // Photograph only the drawing layer (never the rest of the screen). It's excluded from
        // captures in normal use, so allow capture of this one window for the test.
        panel.sharingType = .readOnly
        try await Task.sleep(for: .milliseconds(150))
        let shot = FileManager.default.temporaryDirectory.appendingPathComponent("wisp-drawing-onscreen.png")
        defer { try? FileManager.default.removeItem(at: shot) }
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
        p.arguments = ["-x", "-o", "-l", String(panel.windowNumber), shot.path]
        try p.run()
        p.waitUntilExit()
        if let dir = ProcessInfo.processInfo.environment["WISP_ONSCREEN_SHOTS"] {
            try? FileManager.default.removeItem(atPath: dir + "/drawing-onscreen.png")
            try FileManager.default.copyItem(atPath: shot.path, toPath: dir + "/drawing-onscreen.png")
        }
        let image = try #require(NSImage(contentsOf: shot)?.cgImage(forProposedRect: nil, context: nil, hints: nil))
        // The ink is drawn in the accent colour where the circle's right edge is.
        let scale = CGFloat(image.width) / view.bounds.width
        let px = Int((c.x + 140) * scale), py = Int((view.bounds.height - c.y) * scale)
        let data = CFDataGetBytePtr(image.dataProvider!.data!)!
        let bpp = image.bitsPerPixel / 8
        var purple = false
        for dy in -6...6 {
            for dx in -6...6 {
                let i = (py + dy) * image.bytesPerRow + (px + dx) * bpp
                let (r, g, b) = (Int(data[i]), Int(data[i + 1]), Int(data[i + 2]))
                // Accent #7C6CF2 (screenshots may be BGRA: accept either order).
                if (b > 180 && r > 90 && r < 160 && g < 140) || (r > 180 && b > 90 && b < 160 && g < 140) { purple = true }
            }
        }
        #expect(purple, "the ink is visible on the real screen")

        // Releasing the keys sends the drawing.
        overlay.finishInk()
        let r = try #require(result)
        #expect(r.kind == .lasso)
        #expect(r.selection.width > 250 && r.selection.height > 150)
        #expect(r.strokes.count == 1)
    }
}
