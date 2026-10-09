import AppKit
import Testing
@testable import WispIPC
@testable import WispUI

@MainActor
@Suite struct InkTests {
    private func drag(_ v: FocusOverlayView, _ pts: [CGPoint]) {
        func ev(_ t: NSEvent.EventType, _ p: CGPoint) -> NSEvent {
            NSEvent.mouseEvent(with: t, location: p, modifierFlags: [], timestamp: 0, windowNumber: 0,
                               context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
        }
        v.mouseDown(with: ev(.leftMouseDown, pts[0]))
        for p in pts.dropFirst() { v.mouseDragged(with: ev(.leftMouseDragged, p)) }
        v.mouseUp(with: ev(.leftMouseUp, pts.last!))
    }

    private func view() -> FocusOverlayView {
        let v = FocusOverlayView(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
        v.ink = true
        v.prepare(showHint: true)
        return v
    }

    @Test func severalMarksSelectTheBoxAroundAllOfThem() throws {
        let v = view()
        drag(v, [CGPoint(x: 100, y: 100), CGPoint(x: 160, y: 100), CGPoint(x: 220, y: 100)])  // underline
        drag(v, [CGPoint(x: 400, y: 300), CGPoint(x: 420, y: 320), CGPoint(x: 440, y: 340)])  // arrow
        #expect(v.strokes.count == 2, "lifting the mouse keeps the mark and stays ready")
        let sel = try #require(v.finishInk())
        #expect(sel.kind == .rectangle)
        #expect(sel.rect == CGRect(x: 88, y: 88, width: 364, height: 264))
    }

    @Test func oneLoopIsALassoAndAClickIsAPoint() throws {
        let loop = (0...24).map { i -> CGPoint in
            let a = Double(i) / 24 * 2 * .pi
            return CGPoint(x: 300 + 80 * cos(a), y: 300 + 50 * sin(a))
        }
        let v = view()
        drag(v, loop)
        let sel = try #require(v.finishInk())
        #expect(sel.kind == .lasso)
        #expect(sel.path.count == loop.count + 1)

        let c = view()
        drag(c, [CGPoint(x: 50, y: 60)])
        let tap = try #require(c.finishInk())
        #expect(tap.kind == .point)
        #expect(tap.path == [CGPoint(x: 50, y: 60)])

        #expect(view().finishInk() == nil, "nothing drawn: nothing to send")
    }

    @Test func inkIsBurnedIntoTheCaptureWhereItWasDrawn() throws {
        // A 200×100 pt crop captured at 2× → 400×200 px, all white.
        let ctx = try #require(CGContext(data: nil, width: 400, height: 200, bitsPerComponent: 8, bytesPerRow: 0,
                                         space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.setFillColor(.white)
        ctx.fill(CGRect(x: 0, y: 0, width: 400, height: 200))
        let image = try #require(ctx.makeImage())
        let crop = CGRect(x: 1000, y: 500, width: 200, height: 100)
        // A horizontal stroke 25 pt below the crop's top edge (CG top-left coordinates).
        let out = try #require(InkRenderer.burn([[CGPoint(x: 1020, y: 525), CGPoint(x: 1180, y: 525)]],
                                                into: image, crop: crop, color: .red))
        func pixel(_ x: Int, _ yFromTop: Int) -> (UInt8, UInt8) {
            let data = CFDataGetBytePtr(out.dataProvider!.data!)!
            let i = yFromTop * out.bytesPerRow + x * 4
            return (data[i], data[i + 1])
        }
        let onLine = pixel(200, 50), away = pixel(200, 150)
        #expect(onLine.0 > 200 && onLine.1 < 80, "red where the stroke was drawn")
        #expect(away.0 > 200 && away.1 > 200, "untouched elsewhere")
    }
}
