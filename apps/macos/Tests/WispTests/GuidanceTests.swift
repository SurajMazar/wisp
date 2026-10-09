import AppKit
import Foundation
import Testing
@testable import WispIPC
@testable import WispUI

@Suite struct GuidanceTests {
    /// The exact shape `wisp-screen::guide::show` produces.
    @Test @MainActor func decodesEngineGuidanceAndMeasuresAnnotations() throws {
        let json = """
        {"id":"g1","durationMs":10000,"buddyTarget":{"x":600,"y":454},
         "annotations":[
           {"type":"highlight","data":{"rect":{"x":556,"y":436,"width":88,"height":36},"label":"Save"}},
           {"type":"pointer","data":{"at":{"x":400,"y":300},"text":""}},
           {"type":"arrow","data":{"from":{"x":10,"y":10},"to":{"x":110,"y":60}}}
         ]}
        """
        let g = try JSONDecoder().decode(GuidanceShow.self, from: Data(json.utf8))
        #expect(g.annotations.count == 3)
        guard case .highlight(let h) = g.annotations[0] else { Issue.record("not a highlight"); return }
        #expect(h.label == "Save")
        #expect(GuidanceOverlayController.bounds(of: g.annotations[0]) == CGRect(x: 556, y: 436, width: 88, height: 36))
        #expect(GuidanceOverlayController.bounds(of: g.annotations[2]) == CGRect(x: 10, y: 10, width: 101, height: 51))
        // Round-trips back to the same JSON shape.
        let again = try JSONDecoder().decode(GuidanceShow.self, from: JSONEncoder().encode(g))
        #expect(again == g)
    }
}

@Suite struct GuidanceRenderingTests {
    /// Renders every annotation type; writes a PNG when WISP_RENDER_DIR is set (for visual review).
    @Test @MainActor func rendersAllAnnotationKinds() throws {
        let view = GuidanceView(frame: NSRect(x: 0, y: 0, width: 800, height: 500))
        view.tint = NSColor(srgbHex: 0x7C6CF2)
        view.map = { CGPoint(x: $0.x, y: 500 - $0.y) }  // top-left → bottom-left
        let bg = CALayer()
        bg.frame = view.bounds
        bg.backgroundColor = NSColor(white: 0.95, alpha: 1).cgColor
        view.layer?.addSublayer(bg)
        view.draw(annotations: [
            .highlight(RectAnnotation(label: "Save", rect: Rect(height: 32, width: 90, x: 600, y: 400))),
            .rectangle(RectAnnotation(label: nil, rect: Rect(height: 60, width: 160, x: 40, y: 40))),
            .circle(CircleAnnotation(center: Point(x: 400, y: 120), label: "Here", radius: 40)),
            .arrow(ArrowAnnotation(from: Point(x: 250, y: 300), label: nil, to: Point(x: 590, y: 410))),
            .pointer(LabelAnnotation(at: Point(x: 120, y: 380), text: "")),
            .label(LabelAnnotation(at: Point(x: 400, y: 260), text: "Step 2: open the menu")),
        ])
        let layer = try #require(view.layer)
        #expect((layer.sublayers?.count ?? 0) >= 8)
        let rep = try #require(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        let ctx = try #require(NSGraphicsContext(bitmapImageRep: rep))
        layer.render(in: ctx.cgContext)
        if let dir = ProcessInfo.processInfo.environment["WISP_RENDER_DIR"] {
            try rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: dir).appendingPathComponent("guidance.png"))
        }
    }
}
