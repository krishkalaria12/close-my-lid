import Foundation
import IOKit.ps

struct BatteryStatus: Equatable {
    let percentage: Int
    let isCharging: Bool
}

enum BatteryStatusReader {
    static func read() -> BatteryStatus? {
        guard
            let snapshot = IOPSCopyPowerSourcesInfo()?.takeRetainedValue(),
            let sources = IOPSCopyPowerSourcesList(snapshot)?.takeRetainedValue() as? [CFTypeRef]
        else {
            return nil
        }

        for source in sources {
            guard
                let description = IOPSGetPowerSourceDescription(snapshot, source)?
                    .takeUnretainedValue() as? [String: Any],
                let current = description[kIOPSCurrentCapacityKey] as? Int,
                let max = description[kIOPSMaxCapacityKey] as? Int,
                max > 0
            else {
                continue
            }

            let percentage = Int((Double(current) / Double(max) * 100).rounded())
            let isCharging = description[kIOPSIsChargingKey] as? Bool ?? false
            return BatteryStatus(percentage: min(percentage, 100), isCharging: isCharging)
        }

        return nil
    }
}
