import Foundation
import Testing
@testable import WispIPC
@testable import WispPlatform

@Suite struct SentenceEmbedderTests {
    @Test func similarSentencesAreCloserThanUnrelatedOnes() async throws {
        let e = await SentenceEmbedder.shared.embed([
            "The user prefers dark roast coffee.",
            "Which coffee should I buy?",
            "My dentist appointment is on Friday.",
        ])
        guard e.dims > 0 else {
            print("skipping: sentence embeddings unavailable on this Mac")
            return
        }
        #expect(e.vectors.count == 3 && e.vectors.allSatisfy { $0.count == Int(e.dims) })
        func cos(_ a: [Double], _ b: [Double]) -> Double {
            var dot = 0.0, na = 0.0, nb = 0.0
            for (x, y) in zip(a, b) {
                dot += x * y
                na += x * x
                nb += y * y
            }
            return dot / (na.squareRoot() * nb.squareRoot())
        }
        let related = cos(e.vectors[0], e.vectors[1])
        let unrelated = cos(e.vectors[2], e.vectors[1])
        print("cosine related \(related) unrelated \(unrelated)")
        #expect(related > unrelated)
    }
}
