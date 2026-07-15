import Foundation

/// Decides when a closed-lid sleep hold should be released to protect the
/// battery. Keeping a Mac awake in a bag drains the battery and generates
/// heat, so an unplugged machine that drops below the threshold should return
/// to normal sleep behavior automatically.
public struct BatterySafetyPolicy: Equatable, Sendable {
    public static let defaultThreshold = 5

    /// Battery percentage at or below which an unplugged hold is released.
    public let threshold: Int

    public init(threshold: Int = BatterySafetyPolicy.defaultThreshold) {
        self.threshold = threshold
    }

    /// A hold is only released on battery power. A plugged-in Mac carries no
    /// battery risk, so charging machines are always left alone.
    public func shouldReleaseHold(percentage: Int, isCharging: Bool) -> Bool {
        guard !isCharging else {
            return false
        }

        return percentage <= threshold
    }
}
