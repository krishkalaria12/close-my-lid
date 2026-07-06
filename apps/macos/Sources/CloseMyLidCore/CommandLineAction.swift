public enum CommandLineAction: Equatable, Sendable {
    case launchMenu
    case enable
    case disable
    case status
    case help
    case version
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
        default:
            return nil
        }
    }
}
