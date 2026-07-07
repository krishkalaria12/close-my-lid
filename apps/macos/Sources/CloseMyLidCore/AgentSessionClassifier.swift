/// A snapshot of one running process, reduced to the fields needed to
/// recognize agent harness sessions.
public struct RunningProcess: Equatable, Sendable {
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
/// binary, OpenCode's TUI spawning its server) are not counted twice.
public enum AgentSessionClassifier {
    /// JavaScript runtimes that npm-installed harnesses run under.
    static let scriptRuntimes: Set<String> = ["node", "bun", "deno"]

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

        guard
            scriptRuntimes.contains(process.executableName),
            let script = process.arguments.dropFirst().first(where: { !$0.hasPrefix("-") })
        else {
            return nil
        }

        let scriptName = script.split(separator: "/").last.map(String.init) ?? script
        if let harness = AgentHarness(rawValue: scriptName) {
            return harness
        }
        return AgentHarness.allCases.first { script.contains($0.scriptPathMarker) }
    }

    private static func hasAncestor(
        of pid: Int32,
        matching harness: AgentHarness,
        matches: [Int32: AgentHarness],
        parents: [Int32: Int32]
    ) -> Bool {
        var visited: Set<Int32> = [pid]
        var current = parents[pid]
        while let parent = current, parent > 0, visited.insert(parent).inserted {
            if matches[parent] == harness {
                return true
            }
            current = parents[parent]
        }
        return false
    }
}
