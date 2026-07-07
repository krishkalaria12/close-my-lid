/// A coding agent CLI whose running sessions Close My Lid can detect.
///
/// The raw value is the name the harness executable carries in the process
/// table, so `AgentHarness(rawValue: executableName)` doubles as the match.
public enum AgentHarness: String, CaseIterable, Sendable {
    case claudeCode = "claude"
    case codex
    case openCode = "opencode"

    public var displayName: String {
        switch self {
        case .claudeCode:
            "Claude Code"
        case .codex:
            "OpenAI Codex CLI"
        case .openCode:
            "OpenCode"
        }
    }

    /// Path fragment that identifies the harness when it runs as a script
    /// under a JavaScript runtime instead of as a native binary, as with
    /// npm installs such as `node .../node_modules/@anthropic-ai/claude-code/cli.js`.
    /// Anchored to `node_modules/` so similarly named project directories
    /// do not match.
    var scriptPathMarker: String {
        switch self {
        case .claudeCode:
            "node_modules/@anthropic-ai/claude-code"
        case .codex:
            "node_modules/@openai/codex"
        case .openCode:
            "node_modules/opencode-ai"
        }
    }
}
