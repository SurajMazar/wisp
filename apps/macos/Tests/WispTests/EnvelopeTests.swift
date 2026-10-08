import Foundation
import Testing
@testable import WispIPC

@Suite struct EnvelopeTests {
    private func decode(_ s: String) throws -> Envelope {
        try JSONDecoder().decode(Envelope.self, from: Data(s.utf8))
    }

    @Test func decodesEachKind() throws {
        #expect(try decode(#"{"kind":"request","v":1,"id":"1","method":"app.info"}"#).method == "app.info")
        #expect(try decode(#"{"kind":"response","v":1,"id":"1","result":{"a":1}}"#).result == .object(["a": .int(1)]))
        #expect(try decode(#"{"kind":"event","v":1,"event":"core.ready","payload":null}"#).event == "core.ready")
    }

    @Test func rejectsUnknownFieldsWrongVersionAndIncompleteEnvelopes() {
        #expect(throws: (any Error).self) { try decode(#"{"kind":"event","v":1,"event":"x","evil":true}"#) }
        #expect(throws: (any Error).self) { try decode(#"{"kind":"event","v":2,"event":"x"}"#) }
        #expect(throws: (any Error).self) { try decode(#"{"kind":"request","v":1,"id":"1"}"#) }
        #expect(throws: (any Error).self) { try decode(#"{"kind":"response","v":1}"#) }
    }

    @Test func encodesSingleLineWithoutNilFields() throws {
        let line = try Envelope.request(id: "h1", method: "settings.get", params: .object([:])).encodedLine()
        let text = String(decoding: line, as: UTF8.self)
        #expect(text.hasSuffix("\n"))
        #expect(!text.dropLast().contains("\n"))
        #expect(!text.contains("result"))
    }

    @Test func jsonValuePreservesLargeIntegers() throws {
        let v = try JSONDecoder().decode(JSONValue.self, from: Data("1760000000123".utf8))
        #expect(v == .int(1_760_000_000_123))
        #expect(String(decoding: try JSONEncoder().encode(v), as: UTF8.self) == "1760000000123")
    }

    @Test func generatedTaggedUnionRoundTrips() throws {
        let section = SettingsSection.privacy(PrivacySettings(mode: .cloudAllowed))
        let json = String(decoding: try JSONEncoder().encode(section), as: UTF8.self)
        #expect(json.contains(#""section":"privacy""#))
        #expect(try JSONDecoder().decode(SettingsSection.self, from: Data(json.utf8)) == section)
    }
}

@Suite struct LineBufferTests {
    @Test func splitsAcrossChunks() {
        var b = LineBuffer()
        #expect(b.append(Data("{\"a\":1}\n{\"b\"".utf8)).count == 1)
        let rest = b.append(Data(":2}\n\n".utf8))
        #expect(rest.map { String(decoding: $0, as: UTF8.self) } == [#"{"b":2}"#])
    }
}
