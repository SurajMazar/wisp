import AppKit
import SwiftUI
import Testing
@testable import WispIPC
@testable import WispUI

@MainActor
@Suite struct IslandTests {
    /// Renders each state (set WISP_ISLAND_SNAPSHOTS=<dir> to write PNGs for a visual check).
    @Test func rendersEveryState() throws {
        let states: [(AssistantState, String)] = [
            (.idle, ""), (.listening, ""), (.drawing, ""), (.thinking, ""),
            (.speaking, "The highlighted function retries the request three times before giving up."), (.error, ""),
        ]
        let dir = ProcessInfo.processInfo.environment["WISP_ISLAND_SNAPSHOTS"]
        for (state, caption) in states {
            let model = IslandModel()
            model.notch = CGSize(width: 185, height: 32)
            model.state = state
            model.level = 0.6
            model.caption = caption
            let view = IslandView(model: model, onClick: {})
                .frame(width: 640, height: 140)
                .background(Color(white: 0.85))
            let renderer = ImageRenderer(content: view)
            renderer.scale = 2
            let image = try #require(renderer.cgImage)
            #expect(image.width == 1280)
            #expect(model.isActive == (state != .idle))
            if let dir {
                let rep = NSBitmapImageRep(cgImage: image)
                try rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: dir).appendingPathComponent("island-\(state.rawValue).png"))
            }
        }
    }
}
