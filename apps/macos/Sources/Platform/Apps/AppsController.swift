@preconcurrency import EventKit
import AppKit
import Foundation
import WispIPC

/// Background actions through apps (spec Step 3): Reminders and Calendar via EventKit, Notes and
/// Music via Apple Events, files via the Trash. Each call has already passed the tool pipeline's
/// policy and approval; this only performs it and reports what happened.
public enum AppsController {
    private static let store = EKEventStore()

    public static func perform(_ call: ToolCall) async throws -> ToolExecution {
        switch call {
        case .createReminder(let a): return try await createReminder(a)
        case .listReminders: return try await listReminders()
        case .createEvent(let a): return try await createEvent(a)
        case .listEvents(let a): return try await listEvents(a)
        case .createNote(let a): return try await createNote(a)
        case .music(let a): return try await music(a)
        case .trashFile(let a): return try trash(a)
        default: throw IpcError(code: .invalidParams, message: "That isn't a background action.")
        }
    }

    // MARK: Helpers

    private static func done(_ detail: String, _ output: String? = nil) -> ToolExecution {
        ToolExecution(output: output, verification: Verification(detail: detail, status: .verified))
    }

    private static func denied(_ what: String) -> IpcError {
        IpcError(code: .permissionDenied,
                 message: "Wisp isn't allowed to use \(what). Turn it on in System Settings → Privacy & Security → \(what).")
    }

    /// "YYYY-MM-DD HH:MM" / "YYYY-MM-DD" in local time.
    static func date(_ s: String) throws -> Date {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US_POSIX")
        f.timeZone = .current
        f.dateFormat = s.count > 10 ? "yyyy-MM-dd HH:mm" : "yyyy-MM-dd"
        guard let d = f.date(from: s) else { throw IpcError(code: .invalidParams, message: "\(s) isn't a date Wisp understands.") }
        return d
    }

    private static func show(_ d: Date, time: Bool = true) -> String {
        let f = DateFormatter()
        f.dateStyle = .medium
        f.timeStyle = time ? .short : .none
        f.doesRelativeDateFormatting = true
        return f.string(from: d)
    }

    // MARK: Reminders

    private static func reminders() async throws {
        guard (try? await store.requestFullAccessToReminders()) == true else { throw denied("Reminders") }
    }

    private static func createReminder(_ a: ReminderArgs) async throws -> ToolExecution {
        try await reminders()
        let r = EKReminder(eventStore: store)
        r.title = a.title
        r.calendar = store.defaultCalendarForNewReminders()
        var when = ""
        if let due = a.due {
            let d = try date(due)
            r.dueDateComponents = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute], from: d)
            r.addAlarm(EKAlarm(absoluteDate: d))
            when = " for \(show(d))"
        }
        try store.save(r, commit: true)
        return done("Added “\(a.title)” to Reminders\(when)")
    }

    private static func listReminders() async throws -> ToolExecution {
        try await reminders()
        let predicate = store.predicateForIncompleteReminders(withDueDateStarting: nil, ending: nil, calendars: nil)
        // Reminders aren't Sendable: describe them inside the callback.
        let (count, lines): (Int, [String]) = await withCheckedContinuation { c in
            store.fetchReminders(matching: predicate) { found in
                let found = found ?? []
                let lines = found.prefix(50).map { r -> String in
                    let due = r.dueDateComponents.flatMap { Calendar.current.date(from: $0) }.map { " — due \(show($0))" } ?? ""
                    return "\(r.title ?? "(untitled)")\(due)"
                }
                c.resume(returning: (found.count, lines))
            }
        }
        return done("\(count) open reminders", lines.isEmpty ? "(none)" : lines.joined(separator: "\n"))
    }

    // MARK: Calendar

    private static func calendar() async throws {
        guard (try? await store.requestFullAccessToEvents()) == true else { throw denied("Calendars") }
    }

    private static func createEvent(_ a: EventArgs) async throws -> ToolExecution {
        try await calendar()
        let start = try date(a.start)
        let e = EKEvent(eventStore: store)
        e.title = a.title
        e.startDate = start
        e.endDate = start.addingTimeInterval(TimeInterval(a.minutes) * 60)
        e.calendar = store.defaultCalendarForNewEvents
        try store.save(e, span: .thisEvent, commit: true)
        return done("Added “\(a.title)” to Calendar on \(show(start)) (\(a.minutes) min)")
    }

    private static func listEvents(_ a: DayArgs) async throws -> ToolExecution {
        try await calendar()
        let start = try date(a.day)
        let end = Calendar.current.date(byAdding: .day, value: 1, to: start) ?? start
        let events = store.events(matching: store.predicateForEvents(withStart: start, end: end, calendars: nil))
            .sorted { $0.startDate < $1.startDate }
        let t = DateFormatter()
        t.timeStyle = .short
        let lines = events.prefix(50).map { e in
            e.isAllDay ? "All day: \(e.title ?? "(untitled)")"
                : "\(t.string(from: e.startDate))–\(t.string(from: e.endDate)): \(e.title ?? "(untitled)")"
        }
        return done("\(events.count) events on \(show(start, time: false))", lines.isEmpty ? "(nothing scheduled)" : lines.joined(separator: "\n"))
    }

    // MARK: Notes & Music (Apple Events; macOS asks once per app)

    /// AppleScript string literal.
    static func literal(_ s: String) -> String {
        "\"" + s.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") + "\""
    }

    @MainActor
    private static func run(_ source: String, app: String) throws {
        var error: NSDictionary?
        guard let script = NSAppleScript(source: source) else { throw IpcError(code: .internal, message: "Couldn't prepare the \(app) command.") }
        script.executeAndReturnError(&error)
        if let error {
            let code = error[NSAppleScript.errorNumber] as? Int ?? 0
            if code == -1743 {  // errAEEventNotPermitted
                throw IpcError(code: .permissionDenied,
                               message: "Wisp isn't allowed to control \(app). Turn it on in System Settings → Privacy & Security → Automation → Wisp.")
            }
            throw IpcError(code: .unavailable, message: "\(app) didn't do it: \(error[NSAppleScript.errorMessage] as? String ?? "error \(code)")")
        }
    }

    private static func createNote(_ a: NoteArgs) async throws -> ToolExecution {
        // Notes bodies are HTML: escape, keep line breaks, title first.
        let esc = { (s: String) in
            s.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "<", with: "&lt;").replacingOccurrences(of: ">", with: "&gt;")
        }
        let html = "<h1>\(esc(a.title))</h1>" + esc(a.body).split(separator: "\n", omittingEmptySubsequences: false)
            .map { "<div>\($0.isEmpty ? "<br>" : String($0))</div>" }.joined()
        try await MainActor.run {
            try run("tell application \"Notes\" to make new note with properties {body:\(literal(html))}", app: "Notes")
        }
        return done("Wrote the note “\(a.title)” in Notes")
    }

    private static func music(_ a: MusicArgs) async throws -> ToolExecution {
        let spotifyRunning = NSWorkspace.shared.runningApplications.contains { $0.bundleIdentifier == "com.spotify.client" }
        let musicRunning = NSWorkspace.shared.runningApplications.contains { $0.bundleIdentifier == "com.apple.Music" }
        let app = spotifyRunning && !musicRunning ? "Spotify" : "Music"
        let verb: String = switch a.command {
        case .play: "play"
        case .pause: "pause"
        case .next: "next track"
        case .previous: "previous track"
        }
        try await MainActor.run { try run("tell application \"\(app)\" to \(verb)", app: app) }
        return done("\(app): \(verb)")
    }

    // MARK: Files

    /// `~/…` resolved (symlinks followed) and confined to the home folder, outside ~/Library and
    /// hidden folders — the same rule the engine applies.
    static func homeURL(_ p: String) throws -> URL {
        let home = FileManager.default.homeDirectoryForCurrentUser.resolvingSymlinksInPath().standardizedFileURL
        guard p == "~" || p.hasPrefix("~/") else { throw IpcError(code: .invalidParams, message: "Paths start with ~/.") }
        let rel = p.dropFirst(min(2, p.count))
        let url = home.appendingPathComponent(String(rel)).resolvingSymlinksInPath().standardizedFileURL
        let parts = url.path.dropFirst(home.path.count).split(separator: "/")
        guard url.path.hasPrefix(home.path + "/"), parts.first != "Library", !parts.contains(where: { $0.hasPrefix(".") }) else {
            throw IpcError(code: .invalidParams, message: "\(p) isn't somewhere Wisp may touch.")
        }
        return url
    }

    private static func trash(_ a: PathArgs) throws -> ToolExecution {
        let url = try homeURL(a.path)
        guard FileManager.default.fileExists(atPath: url.path) else { throw IpcError(code: .notFound, message: "\(a.path) doesn't exist.") }
        try FileManager.default.trashItem(at: url, resultingItemURL: nil)
        return done("Moved \(a.path) to the Trash (you can put it back from there)")
    }
}

public extension ToolCall {
    /// Acts through apps and files directly (mirrors `ToolCall::is_background` in the engine).
    var isBackground: Bool {
        switch self {
        case .createReminder, .listReminders, .createEvent, .listEvents, .createNote, .music,
             .listFiles, .moveFile, .trashFile, .fetchPage:
            true
        default:
            false
        }
    }
}
