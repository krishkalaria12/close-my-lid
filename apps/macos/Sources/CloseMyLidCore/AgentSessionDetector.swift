#if canImport(Darwin)
import Darwin

/// Counts running agent harness sessions by snapshotting the current user's
/// processes with `sysctl`, avoiding the cost of spawning `ps` or `pgrep`.
///
/// Process arguments are fetched only for JavaScript runtime processes,
/// since native harness binaries are recognized by executable name alone.
public enum AgentSessionDetector {
    public static func sessionCounts() -> [AgentHarness: Int] {
        AgentSessionClassifier.sessionCounts(in: currentUserProcesses())
    }

    static func currentUserProcesses() -> [RunningProcess] {
        var argumentBuffer: [UInt8] = []
        var processes: [RunningProcess] = []

        for info in listProcesses() {
            // 5 is SZOMB; exited-but-unreaped processes are not sessions.
            guard info.kp_proc.p_stat != 5 else {
                continue
            }

            let name = withUnsafeBytes(of: info.kp_proc.p_comm) { raw in
                String(decoding: raw.prefix(while: { $0 != 0 }), as: UTF8.self)
            }
            let pid = info.kp_proc.p_pid
            var arguments: [String] = []
            if AgentSessionClassifier.scriptRuntimes.contains(name) {
                if argumentBuffer.isEmpty {
                    argumentBuffer = [UInt8](repeating: 0, count: argumentBufferSize)
                }
                arguments = Self.arguments(ofPID: pid, reusing: &argumentBuffer)
            }

            processes.append(RunningProcess(
                id: pid,
                parentID: info.kp_eproc.e_ppid,
                executableName: name,
                arguments: arguments
            ))
        }
        return processes
    }

    private static func listProcesses() -> [kinfo_proc] {
        var request: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_UID, Int32(bitPattern: getuid())]
        let stride = MemoryLayout<kinfo_proc>.stride

        // The process table can grow between the size probe and the fetch,
        // so pad the buffer and retry a few times if it still comes up short.
        for _ in 0..<3 {
            var size = 0
            guard sysctl(&request, UInt32(request.count), nil, &size, nil, 0) == 0, size > 0 else {
                return []
            }

            let capacity = (size + size / 4) / stride + 1
            var buffer = [kinfo_proc](repeating: kinfo_proc(), count: capacity)
            var bufferSize = capacity * stride
            if sysctl(&request, UInt32(request.count), &buffer, &bufferSize, nil, 0) == 0 {
                return Array(buffer.prefix(bufferSize / stride))
            }
            if errno != ENOMEM {
                return []
            }
        }
        return []
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

    private static func arguments(ofPID pid: Int32, reusing buffer: inout [UInt8]) -> [String] {
        var request: [Int32] = [CTL_KERN, KERN_PROCARGS2, pid]
        var size = buffer.count
        guard
            sysctl(&request, UInt32(request.count), &buffer, &size, nil, 0) == 0,
            size > MemoryLayout<Int32>.size
        else {
            return []
        }

        // The buffer holds: argc, the executable path, NUL padding, then the
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
