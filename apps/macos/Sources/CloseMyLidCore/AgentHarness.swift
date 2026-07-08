/// A coding agent CLI whose running sessions Close My Lid can detect.
///
/// The raw value is the name the harness executable carries in the process
/// table, so `AgentHarness(rawValue: executableName)` doubles as the match.
public enum AgentHarness: String, CaseIterable, Sendable {
    case claudeCode = "claude"
    case codex
    case openCode = "opencode"
    case gemini
    case copilot
    case cursor = "cursor-agent"
    case pi

    public var displayName: String {
        switch self {
        case .claudeCode:
            "Claude Code"
        case .codex:
            "OpenAI Codex CLI"
        case .openCode:
            "OpenCode"
        case .gemini:
            "Gemini CLI"
        case .copilot:
            "GitHub Copilot CLI"
        case .cursor:
            "Cursor CLI"
        case .pi:
            "Pi"
        }
    }

    /// Path fragments that identify the harness when it runs as a script under
    /// a JavaScript runtime instead of as a native binary. Each fragment is
    /// anchored to the harness's install directory — `node_modules/<package>`
    /// for npm installs, or Cursor's versioned install layout — so similarly
    /// named project directories do not match.
    private var scriptPathMarkers: [String] {
        switch self {
        case .claudeCode:
            ["node_modules/@anthropic-ai/claude-code"]
        case .codex:
            ["node_modules/@openai/codex"]
        case .openCode:
            ["node_modules/opencode-ai"]
        case .gemini:
            ["node_modules/@google/gemini-cli"]
        case .copilot:
            // Also matches the @github/copilot-<platform> packages that carry
            // the native binary the npm loader script launches.
            ["node_modules/@github/copilot"]
        case .cursor:
            // Cursor CLI has no npm package; its `cursor-agent` launcher
            // script execs a bundled Node runtime on the installer's
            // auto-update layout, e.g.
            // `~/.local/share/cursor-agent/versions/<v>/index.js`.
            ["cursor-agent/versions/"]
        case .pi:
            // The coding agent moved npm scopes; both remain installable.
            ["node_modules/@earendil-works/pi-coding-agent", "node_modules/@mariozechner/pi-coding-agent"]
        }
    }

    /// The harness whose install-path marker appears in a script path passed
    /// to a JavaScript runtime, or `nil` for paths that belong to no harness.
    static func matching(scriptPath: String) -> AgentHarness? {
        allCases.first { harness in
            harness.scriptPathMarkers.contains { scriptPath.contains($0) }
        }
    }
}
