// From examples/t3-code/mobile: swiftc modules/apple/T3GitCommandMessage.swift tests/GitCommandMessageTests.swift -o /tmp/t3-git-command-message-tests
import Foundation

@main struct GitCommandMessageTests {
    static func main() {
        var checks = 0
        var failures: [String] = []
        func equal(_ actual: String, _ expected: String, _ label: String) {
            checks += 1
            if actual != expected { failures.append("\(label): expected \(expected.debugDescription), got \(actual.debugDescription)") }
        }

        // Literal expected messages follow the pinned getter; its reason schema admits these eight tags.
        let reasons = [
            ("authentication_failed", "Git command failed in fetch (/repo): exit 128 (authentication_failed)"),
            ("branch_already_exists", "Git command failed in fetch (/repo): exit 128 (branch_already_exists)"),
            ("branch_checked_out_in_worktree", "Git command failed in fetch (/repo): exit 128 (branch_checked_out_in_worktree)"),
            ("host_key_unverified", "Git command failed in fetch (/repo): exit 128 (host_key_unverified)"),
            ("not_a_repository", "Git command failed in fetch (/repo): exit 128 (not_a_repository)"),
            ("path_already_exists", "Git command failed in fetch (/repo): exit 128 (path_already_exists)"),
            ("remote_unreachable", "Git command failed in fetch (/repo): exit 128 (remote_unreachable)"),
            ("tag_would_be_clobbered", "Git command failed in fetch (/repo): exit 128 (tag_would_be_clobbered)"),
        ]
        for (reason, expected) in reasons {
            equal(T3GitCommandMessage.message(explicit: "", operation: "fetch", cwd: "/repo", detail: "exit 128", reason: reason), expected, reason)
        }
        equal(T3GitCommandMessage.message(explicit: "", operation: "switchRef", cwd: "/repo", detail: "exit 1"),
              "Git command failed in switchRef (/repo): exit 1", "absent optional reason")
        equal(T3GitCommandMessage.message(explicit: "", operation: "切换", cwd: "/项目/🙂", detail: "分支不可用", reason: "branch_already_exists"),
              "Git command failed in 切换 (/项目/🙂): 分支不可用 (branch_already_exists)", "Unicode fields")
        equal(T3GitCommandMessage.message(explicit: "", operation: "", cwd: "", detail: ""),
              "Git command failed in  (): ", "empty string fields")
        equal(T3GitCommandMessage.message(explicit: " \nUse the server's explanation.\t", operation: "fetch", cwd: "/repo", detail: "exit 128", reason: "authentication_failed"),
              "Use the server's explanation.", "trimmed explicit message wins")
        equal(T3GitCommandMessage.message(explicit: "\n\t ", operation: " fetch\n", cwd: " /repo\t", detail: " exit 128\n", reason: "remote_unreachable"),
              "Git command failed in  fetch\n ( /repo\t):  exit 128\n (remote_unreachable)", "raw getter string fields")
        equal(T3GitCommandMessage.message(explicit: "", operation: "status", cwd: "/repo ", detail: "exit 128"),
              "Git command failed in status (/repo ): exit 128", "trailing space identifies the actual directory")
        equal(T3GitCommandMessage.message(explicit: "", operation: "fetch", cwd: "/repo", detail: "first line\n\nlast line\n", reason: "authentication_failed"),
              "Git command failed in fetch (/repo): first line\n\nlast line\n (authentication_failed)", "detail newlines are preserved")

        // Outside the reference reason schema: retain the existing native diagnostic trimming policy.
        equal(T3GitCommandMessage.message(explicit: "   ", operation: "", cwd: "", detail: "", reason: " \n"),
              "Git command failed in  (): ", "blank reason is absent after native trimming")

        // Outside the 365 reference's reason schema: formatting raw text does not admit it as a decoded reason.
        equal(T3GitCommandMessage.message(explicit: "", operation: "switchRef", cwd: "/repo", detail: "exit 128", reason: "ref_not_found"),
              "Git command failed in switchRef (/repo): exit 128 (ref_not_found)", "unsupported newer reason remains raw display text")

        for failure in failures { print("FAIL: \(failure)") }
        if !failures.isEmpty { exit(1) }
        print("Git command messages: \(checks) checks passed")
    }
}
