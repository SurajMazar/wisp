import AppKit
import CoreGraphics
import Foundation
import ScreenCaptureKit
import Testing
@testable import WispIPC
@testable import WispPlatform
@testable import WispUI

@Suite struct FocusGeometryTests {
    @Test func appKitAndCoreGraphicsRoundTrip() {
        let h: CGFloat = 982
        let r = CGRect(x: 100, y: 700, width: 300, height: 200)  // AppKit, y up
        let cg = FocusGeometry.cgRect(fromAppKit: r, primaryHeight: h)
        #expect(cg == CGRect(x: 100, y: 82, width: 300, height: 200))
        #expect(FocusGeometry.appKitRect(fromCG: cg, primaryHeight: h) == r)
        // A display above the primary has negative CG y.
        let above = FocusGeometry.cgRect(fromAppKit: CGRect(x: 0, y: 982, width: 2560, height: 1440), primaryHeight: h)
        #expect(above.minY == -1440)
    }

    @Test func dragRectAnyDirection() {
        #expect(FocusGeometry.rect(from: CGPoint(x: 50, y: 40), to: CGPoint(x: 10, y: 90)) == CGRect(x: 10, y: 40, width: 40, height: 50))
    }

    @Test func cropAddsMarginAndClipsToDisplay() {
        let display = CGRect(x: 1512, y: -200, width: 2560, height: 1440)
        let crop = FocusGeometry.crop(selection: CGRect(x: 1520, y: -195, width: 100, height: 100), margin: 24, display: display)
        #expect(crop == CGRect(x: 1512, y: -200, width: 132, height: 129))
    }

    @Test func pointRegionShiftsInsideDisplay() {
        let display = CGRect(x: 0, y: 0, width: 1512, height: 982)
        let r = FocusGeometry.pointRegion(center: CGPoint(x: 10, y: 970), side: 480, display: display)
        #expect(r.size == CGSize(width: 480, height: 480))
        #expect(display.contains(r))
    }

    @Test func localRectAndRetinaPixels() {
        let display = CGRect(x: -1920, y: 0, width: 1920, height: 1080)
        let local = FocusGeometry.local(CGRect(x: -1900, y: 100, width: 333, height: 211), in: display)
        #expect(local == CGRect(x: 20, y: 100, width: 333, height: 211))
        let px = FocusGeometry.pixelSize(local, scale: 2)
        #expect(px.width == 666 && px.height == 422)
    }

    @Test func clickVersusDrag() {
        #expect(!FocusGeometry.isDrag(from: .zero, to: CGPoint(x: 3, y: 3)))
        #expect(FocusGeometry.isDrag(from: .zero, to: CGPoint(x: 10, y: 0)))
    }
}

@Suite @MainActor struct FocusSchemeTests {
    @Test func focusHostServesOnlyPNGsInsideCache() throws {
        let fm = FileManager.default
        let dir = fm.temporaryDirectory.appendingPathComponent("wisp-focus-\(UUID().uuidString)")
        try fm.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: dir) }
        try Data([0x89]).write(to: dir.appendingPathComponent("a.png"))
        try Data("x".utf8).write(to: dir.appendingPathComponent("notes.txt"))
        let h = WispSchemeHandler(roots: ["focus": dir])
        #expect(h.resolve("/a.png", host: "focus") != nil)
        #expect(h.resolve("/notes.txt", host: "focus") == nil, "non-PNG must not be served")
        #expect(h.resolve("/../a.png", host: "focus") == nil)
        #expect(h.resolve("/a.png", host: "app") == nil, "unknown host root")
    }
}

/// Real capture through ScreenCaptureKit. Skipped when this process lacks Screen Recording access.
@Suite(.serialized) @MainActor struct ScreenCaptureIntegrationTests {
    @Test func capturesExactlyTheSelectedRectAtNativeScale() async throws {
        guard CGPreflightScreenCaptureAccess() else {
            print("skipping: no Screen Recording permission for the test runner")
            return
        }
        // On the lock screen nothing the test draws is visible to capture.
        let session = CGSessionCopyCurrentDictionary() as? [String: Any]
        if session?["CGSSessionScreenIsLocked"] as? Bool == true {
            print("skipping: the screen is locked")
            return
        }
        _ = NSApplication.shared
        guard let screen = NSScreen.main else { return }
        let display = Displays.info(for: screen)
        let h = Displays.primaryHeight

        // A magenta target with a 20 pt cyan border; we capture the target plus 10 pt of border.
        let targetCG = CGRect(x: display.frame.x + 300, y: display.frame.y + 260, width: 201, height: 133)
        let windowCG = targetCG.insetBy(dx: -20, dy: -20)
        let window = NSWindow(contentRect: FocusGeometry.appKitRect(fromCG: windowCG, primaryHeight: h),
                              styleMask: .borderless, backing: .buffered, defer: false)
        window.level = .screenSaver
        window.backgroundColor = .cyan
        let inner = NSView(frame: NSRect(x: 20, y: 20, width: targetCG.width, height: targetCG.height))
        inner.wantsLayer = true
        inner.layer?.backgroundColor = NSColor(srgbRed: 1, green: 0, blue: 1, alpha: 1).cgColor
        window.contentView?.addSubview(inner)
        window.orderFrontRegardless()
        defer { window.orderOut(nil) }
        try await Task.sleep(for: .milliseconds(400))

        let crop = targetCG.insetBy(dx: -10, dy: -10)
        let image = try await ScreenCapturer.capture(rect: crop, display: display, excludingSelf: false)
        let expected = FocusGeometry.pixelSize(crop, scale: display.scale)
        #expect(image.width == expected.width && image.height == expected.height, "Retina pixel size")

        // Sample: center is magenta; 5 pt inside the crop edge is cyan border (no shift).
        let scale = display.scale
        func color(atPoint x: CGFloat, _ y: CGFloat) -> (Int, Int, Int)? {
            guard let data = image.dataProvider?.data, let ptr = CFDataGetBytePtr(data) else { return nil }
            let px = Int(x * scale), py = Int(y * scale)
            let bpr = image.bytesPerRow, bpp = image.bitsPerPixel / 8
            let o = py * bpr + px * bpp
            // BGRA (little-endian) from ScreenCaptureKit
            let le = image.bitmapInfo.contains(.byteOrder32Little)
            return le ? (Int(ptr[o + 2]), Int(ptr[o + 1]), Int(ptr[o])) : (Int(ptr[o]), Int(ptr[o + 1]), Int(ptr[o + 2]))
        }
        let center = try #require(color(atPoint: crop.width / 2, crop.height / 2))
        #expect(center.0 > 200 && center.1 < 60 && center.2 > 200, "center should be magenta, got \(center)")
        let edge = try #require(color(atPoint: 5, 5))
        #expect(edge.0 < 60 && edge.1 > 200 && edge.2 > 200, "border should be cyan, got \(edge)")
        let justInside = try #require(color(atPoint: 12, crop.height / 2))
        #expect(justInside.0 > 200 && justInside.2 > 200 && justInside.1 < 60, "target begins 10 pt in, got \(justInside)")
    }
}
