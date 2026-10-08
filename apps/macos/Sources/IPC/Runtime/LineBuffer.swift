import Foundation

/// Splits a byte stream into newline-terminated frames, enforcing the frame size cap.
public struct LineBuffer: Sendable {
    public static let maxFrameBytes = 16 * 1024 * 1024
    private var buffer = Data()
    private(set) public var droppedOversizeFrames = 0

    public init() {}

    /// Append bytes and return all complete lines (without the trailing newline).
    public mutating func append(_ data: Data) -> [Data] {
        buffer.append(data)
        var lines: [Data] = []
        while let nl = buffer.firstIndex(of: 0x0A) {
            let line = buffer[buffer.startIndex..<nl]
            buffer.removeSubrange(buffer.startIndex...nl)
            if !line.isEmpty { lines.append(Data(line)) }
        }
        if buffer.count > Self.maxFrameBytes {
            // A frame this large is a protocol violation; discard until the next newline.
            buffer.removeAll(keepingCapacity: false)
            droppedOversizeFrames += 1
        }
        return lines
    }
}
