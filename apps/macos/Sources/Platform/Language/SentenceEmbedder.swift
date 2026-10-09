import Foundation
import NaturalLanguage
import WispIPC

/// On-device sentence embeddings (Apple NaturalLanguage) for memory and file search.
/// Nothing leaves the Mac; no model download.
public actor SentenceEmbedder {
    public static let shared = SentenceEmbedder()
    public static let modelName = "apple-nl-sentence-en"

    private var embedding: NLEmbedding?
    private var loaded = false

    private func model() -> NLEmbedding? {
        if !loaded {
            embedding = NLEmbedding.sentenceEmbedding(for: .english)
            loaded = true
        }
        return embedding
    }

    public func embed(_ texts: [String]) -> Embeddings {
        guard let m = model() else {
            return Embeddings(dims: 0, model: Self.modelName, vectors: texts.map { _ in [] })
        }
        let vectors: [[Double]] = texts.map { t in
            let clipped = String(t.prefix(2000)).trimmingCharacters(in: .whitespacesAndNewlines)
            guard !clipped.isEmpty, let v = m.vector(for: clipped) else { return [] }
            return v
        }
        return Embeddings(dims: UInt32(m.dimension), model: Self.modelName, vectors: vectors)
    }
}
