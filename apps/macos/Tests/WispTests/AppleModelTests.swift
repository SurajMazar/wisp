import Foundation
import Testing
@testable import WispIPC
@testable import WispPlatform

@Suite(.serialized) @MainActor struct AppleModelTests {
    @Test func reportsRealAvailability() {
        let s = AppleModelService().status
        #expect([.available, .unavailable, .notInstalled].contains(s.state))
        if s.state != .available { #expect(s.detail != nil, "unavailable states must explain why") }
    }

    @Test func answersTwoPlusTwoWithStreamedDeltas() async throws {
        let service = AppleModelService()
        guard service.status.state == .available else {
            print("skipping: Apple Foundation Models unavailable — \(service.status.detail ?? "")")
            return
        }
        var deltas: [String] = []
        let text = try await service.generate(
            AppleGenerateParams(instructions: "Answer with just the number.", maxTokens: 16, prompt: "What is 2 + 2?", requestId: "t1", temperature: 0)
        ) { deltas.append($0) }
        #expect(text.contains("4"), "got \(text)")
        #expect(!deltas.isEmpty)
        #expect(deltas.joined() == text, "deltas must reassemble the final text")
    }
}

@Suite struct KeychainTests {
    @Test func rejectsHostileAccountNames() {
        for bad in ["", "../x", "a b", String(repeating: "x", count: 200)] {
            #expect(throws: (any Error).self) { try Keychain.validate(bad) }
        }
        #expect(throws: Never.self) { try Keychain.validate("endpoint:lm-studio") }
    }
}
