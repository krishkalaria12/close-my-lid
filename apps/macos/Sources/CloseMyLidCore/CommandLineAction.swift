public enum CommandLineAction: Equatable, Sendable {
    case launchMenu
    case enable
    case disable
    case status
    case help
    case version
    /// Headless dead-man check run by the watchdog LaunchAgent.
    case watchdog
}

public enum CommandLineActionParser {
    public static func parse(_ arguments: [String]) -> CommandLineAction? {
        guard let first = arguments.first else {
            return .launchMenu
        }

        switch first {
        case "enable", "start", "--enable":
            return .enable
        case "disable", "stop", "--disable":
            return .disable
        case "status", "--status":
            return .status
        case "help", "--help", "-h":
            return .help
        case "version", "--version", "-v":
            return .version
        // Hidden flag: invoked by the watchdog LaunchAgent only.
        case "--watchdog":
            return .watchdog
        default:
            return nil
        }
    }
}
