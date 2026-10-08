import Foundation
import os

/// Subsystem loggers. Never log secrets, screenshots, audio, clipboard or AX trees.
public enum Log {
    public static let subsystem = "com.wisp.assistant"
    public static let app = Logger(subsystem: subsystem, category: "app")
    public static let ipc = Logger(subsystem: subsystem, category: "ipc")
    public static let core = Logger(subsystem: subsystem, category: "core")
    public static let ui = Logger(subsystem: subsystem, category: "ui")
}
