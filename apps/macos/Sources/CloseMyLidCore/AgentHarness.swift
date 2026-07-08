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
    /// a JavaScript runtime instead of as a native binary, as with npm installs
    /// such as `node .../node_modules/@anthropic-ai/claude-code/cli.js`.
    /// Anchored to `node_modules/` so similarly named project directories do
    /// not match. A harness may publish under more than one package scope, and
    /// harnesses distributed only as native binaries (Cursor) have none.
    var scriptPathMarkers: [String] {
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
            ["node_modules/@github/copilot"]
        case .cursor:
            []
        case .pi:
            // The coding agent moved npm scopes; both remain installable.
            ["node_modules/@earendil-works/pi-coding-agent", "node_modules/@mariozechner/pi-coding-agent"]
        }
    }
}
