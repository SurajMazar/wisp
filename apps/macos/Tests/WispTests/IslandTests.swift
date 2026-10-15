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
            (.executing, ""), (.dictating, ""),
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
        // Talk mode between turns, and a background task's spoken summary.
        for (name, configure) in [
            ("talking", { (m: IslandModel) in m.talking = true; m.state = .listening; m.level = 0.5 }),
            ("done", { (m: IslandModel) in m.state = .speaking; m.caption = "I'll remind you tomorrow at 9 to call Sam." }),
            ("answer", { (m: IslandModel) in m.state = .speaking; m.caption = "Click Save, at the bottom right, to keep your changes." }),
            ("circled", { (m: IslandModel) in m.state = .speaking; m.caption = "That's Skip: it jumps straight to the next video." }),
        ] as [(String, (IslandModel) -> Void)] {
            let model = IslandModel()
            model.notch = CGSize(width: 185, height: 32)
            configure(model)
            #expect(model.isActive)
            let renderer = ImageRenderer(content: IslandView(model: model, onClick: {}).frame(width: 640, height: 140))
            renderer.scale = 2
            let image = try #require(renderer.cgImage)
            if let dir {
                try NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:])?
                    .write(to: URL(fileURLWithPath: dir).appendingPathComponent("island-\(name).png"))
            }
        }
    }

    /// Test Voice (or any later speech) must not bring back the previous answer's text.
    @Test func anOldAnswerIsNotShownAgain() {
        let island = IslandController()
        island.enabled = false  // no window on screen during tests
        island.set(state: .thinking)
        island.appendCaption("The previous answer.")
        island.set(state: .idle)
        island.set(state: .speaking)
        #expect(island.model.caption.isEmpty)
    }

    /// Pointing markers are for the on-screen marks, never the caption, even mid-stream.
    @Test func captionsLeaveOutPointingMarkers() {
        let island = IslandController()
        island.enabled = false
        island.set(state: .speaking)
        island.appendCaption("Click Save [[point: Sa")
        #expect(island.model.caption == "Click Save ")
        island.appendCaption("ve]] to keep it. Or Export [[point: 812,40]].")
        #expect(island.model.caption == "Click Save to keep it. Or Export.")
    }
}
