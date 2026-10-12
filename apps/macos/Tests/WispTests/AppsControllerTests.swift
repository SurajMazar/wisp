import Foundation
import Testing
@testable import WispIPC
@testable import WispPlatform

@Suite struct AppsControllerTests {
    @Test func parsesLocalDates() throws {
        let d = try AppsController.date("2026-10-13 09:05")
        let c = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute], from: d)
        #expect((c.year, c.month, c.day, c.hour, c.minute) == (2026, 10, 13, 9, 5))
        #expect(throws: IpcError.self) { try AppsController.date("next tuesday") }
    }

    @Test func filesStayInTheHomeFolder() throws {
        let home = FileManager.default.homeDirectoryForCurrentUser.resolvingSymlinksInPath().standardizedFileURL.path
        #expect(try AppsController.homeURL("~/Desktop/old.zip").path == home + "/Desktop/old.zip")
        for bad in ["/etc/hosts", "~/../other", "~/Library/Keychains", "~/.ssh/id_rsa", "Desktop/x"] {
            #expect(throws: IpcError.self, "\(bad)") { try AppsController.homeURL(bad) }
        }
    }

    @Test func appleScriptTextCantBreakOut() {
        #expect(AppsController.literal("say \"hi\" \\ bye") == "\"say \\\"hi\\\" \\\\ bye\"")
    }

    @Test func onlyBackgroundActionsGoThroughApps() {
        #expect(ToolCall.createReminder(ReminderArgs(title: "x")).isBackground)
        #expect(ToolCall.music(MusicArgs(command: .play)).isBackground)
        #expect(!ToolCall.click(PointerArgs(x: 1, y: 1)).isBackground)
    }
}
