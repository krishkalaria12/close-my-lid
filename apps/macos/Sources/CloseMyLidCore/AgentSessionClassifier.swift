/// A snapshot of one running process, reduced to the fields needed to
/// recognize agent harness sessions.
public struct RunningProcess: Sendable {
    public let id: Int32
    public let parentID: Int32
    public let executableName: String
    public let arguments: [String]

    public init(id: Int32, parentID: Int32, executableName: String, arguments: [String] = []) {
        self.id = id
        self.parentID = parentID
        self.executableName = executableName
        self.arguments = arguments
    }
}

/// Turns a process snapshot into per-harness session counts.
///
/// A session is a matched process with no ancestor matched to the same
/// harness, so helper children (Codex's npm wrapper spawning the native
/// binary, OpenCode's launcher spawning its server/TUI) are not counted
/// twice.
public enum AgentSessionClassifier {
    /// JavaScript runtimes that npm-installed harnesses run under.
    static let scriptRuntimes: Set<String> = ["node", "bun"]

    public static func sessionCounts(in processes: [RunningProcess]) -> [AgentHarness: Int] {
        var parents: [Int32: Int32] = [:]
        var matches: [Int32: AgentHarness] = [:]
        parents.reserveCapacity(processes.count)

        for process in processes {
            parents[process.id] = process.parentID
            if let harness = harness(for: process) {
                matches[process.id] = harness
            }
        }

        var counts: [AgentHarness: Int] = [:]
        for (pid, harness) in matches where !hasAncestor(of: pid, matching: harness, matches: matches, parents: parents) {
            counts[harness, default: 0] += 1
        }
        return counts
    }

    static func harness(for process: RunningProcess) -> AgentHarness? {
        if let harness = AgentHarness(rawValue: process.executableName) {
            return harness
        }

        guard scriptRuntimes.contains(process.executableName) else {
            return nil
        }

        // Runtime flags and subcommands never contain a path separator, so
        // only path-like arguments are script candidates. A candidate matches
        // when its basename is a harness executable name (bin shims like
        // /usr/local/bin/claude keep the harness name) or when it lives in
        // the harness's npm package directory.
        for argument in process.arguments.dropFirst() where argument.contains("/") {
            if let basename = argument.split(separator: "/").last,
               let harness = AgentHarness(rawValue: String(basename)) {
                return harness
            }
            if let harness = AgentHarness.allCases.first(where: { argument.contains($0.scriptPathMarker) }) {
                return harness
            }
        }
        return nil
    }

    private static func hasAncestor(
        of pid: Int32,
        matching harness: AgentHarness,
        matches: [Int32: AgentHarness],
        parents: [Int32: Int32]
    ) -> Bool {
        // Parent chains in a snapshot are short; the hop bound terminates
        // ppid cycles that pid reuse could introduce mid-snapshot.
        var current = parents[pid]
        var hops = 0
        while let parent = current, parent > 0, hops < 64 {
            if matches[parent] == harness {
                return true
            }
            current = parents[parent]
            hops += 1
        }
        return false
    }
}
