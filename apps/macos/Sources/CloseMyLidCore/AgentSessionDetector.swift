#if canImport(Darwin)
import Darwin

/// Counts running agent harness sessions by snapshotting the current user's
/// processes with `sysctl`, avoiding the cost of spawning `ps` or `pgrep`.
///
/// Process arguments are fetched only for JavaScript runtime processes,
/// since native harness binaries are recognized by executable name alone.
public enum AgentSessionDetector {
    private static let zombie: Int32 = 5 // SZOMB in <sys/proc.h>

    public static func sessionCounts() -> [AgentHarness: Int] {
        AgentSessionClassifier.sessionCounts(in: currentUserProcesses())
    }

    static func currentUserProcesses() -> [RunningProcess] {
        // One scratch buffer, reused across every argument fetch in the scan.
        var argumentBuffer = [UInt8](repeating: 0, count: argumentBufferSize)

        return listProcesses().compactMap { info in
            guard Int32(info.kp_proc.p_stat) != zombie else {
                return nil // Exited-but-unreaped processes are not sessions.
            }

            let name = withUnsafeBytes(of: info.kp_proc.p_comm) { raw in
                String(decoding: raw.prefix(while: { $0 != 0 }), as: UTF8.self)
            }
            let needsArguments = AgentSessionClassifier.scriptRuntimes.contains(name)
            return RunningProcess(
                id: info.kp_proc.p_pid,
                parentID: info.kp_eproc.e_ppid,
                executableName: name,
                arguments: needsArguments ? arguments(ofPID: info.kp_proc.p_pid, into: &argumentBuffer) : []
            )
        }
    }

    private static func listProcesses() -> [kinfo_proc] {
        var request: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_UID, Int32(bitPattern: getuid())]
        let stride = MemoryLayout<kinfo_proc>.stride

        var size = 0
        guard sysctl(&request, UInt32(request.count), nil, &size, nil, 0) == 0, size > 0 else {
            return []
        }

        // Pad the buffer so a table that grows between the size probe and the
        // fetch still fits; sysctl reports the bytes actually written.
        let capacity = (size + size / 4) / stride + 1
        var buffer = [kinfo_proc](repeating: kinfo_proc(), count: capacity)
        var bufferSize = capacity * stride
        guard sysctl(&request, UInt32(request.count), &buffer, &bufferSize, nil, 0) == 0 else {
            return []
        }
        return Array(buffer.prefix(bufferSize / stride))
    }

    private static let argumentBufferSize: Int = {
        var request: [Int32] = [CTL_KERN, KERN_ARGMAX]
        var argumentMax: Int32 = 0
        var size = MemoryLayout<Int32>.size
        guard sysctl(&request, 2, &argumentMax, &size, nil, 0) == 0, argumentMax > 0 else {
            return 262_144
        }
        return Int(argumentMax)
    }()

    private static func arguments(ofPID pid: Int32, into buffer: inout [UInt8]) -> [String] {
        var request: [Int32] = [CTL_KERN, KERN_PROCARGS2, pid]
        var size = buffer.count
        guard
            sysctl(&request, UInt32(request.count), &buffer, &size, nil, 0) == 0,
            size > MemoryLayout<Int32>.size
        else {
            return []
        }

        // Layout: argc, the executable path, NUL padding, then the
        // NUL-separated argv strings (followed by the environment).
        let argumentCount = Int(buffer.withUnsafeBytes { $0.load(as: Int32.self) })
        guard argumentCount > 0 else {
            return []
        }

        var index = MemoryLayout<Int32>.size
        while index < size, buffer[index] != 0 { index += 1 }
        while index < size, buffer[index] == 0 { index += 1 }

        var arguments: [String] = []
        var start = index
        while index < size, arguments.count < argumentCount {
            if buffer[index] == 0 {
                arguments.append(String(decoding: buffer[start..<index], as: UTF8.self))
                start = index + 1
            }
            index += 1
        }
        return arguments
    }
}
#endif
