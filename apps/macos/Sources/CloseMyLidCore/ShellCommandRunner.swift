import Foundation

public struct ShellCommandResult: Equatable, Sendable {
    public let status: Int32
    public let output: String
}

public struct ShellCommandRunner: Sendable {
    public init() {}

    public func run(executablePath: String, arguments: [String]) throws -> ShellCommandResult {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: executablePath)
        process.arguments = arguments

        let outputPipe = Pipe()
        process.standardOutput = outputPipe
        process.standardError = outputPipe

        try process.run()
        process.waitUntilExit()

        let output = String(
            data: outputPipe.fileHandleForReading.readDataToEndOfFile(),
            encoding: .utf8
        )?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""

        return ShellCommandResult(status: process.terminationStatus, output: output)
    }
}
