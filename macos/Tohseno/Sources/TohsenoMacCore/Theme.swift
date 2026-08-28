import SwiftUI

public enum TohsenoTheme {
    public static let void = Color(red: 5 / 255, green: 5 / 255, blue: 5 / 255)
    public static let carbon = Color(red: 13 / 255, green: 13 / 255, blue: 14 / 255)
    public static let graphite = Color(red: 25 / 255, green: 25 / 255, blue: 26 / 255)
    public static let iron = Color(red: 43 / 255, green: 43 / 255, blue: 45 / 255)
    public static let ash = Color(red: 104 / 255, green: 105 / 255, blue: 100 / 255)
    public static let silver = Color(red: 167 / 255, green: 168 / 255, blue: 162 / 255)
    public static let bone = Color(red: 241 / 255, green: 237 / 255, blue: 228 / 255)
    public static let amber = Color(red: 1, green: 90 / 255, blue: 0)
    public static let ember = Color(red: 53 / 255, green: 22 / 255, blue: 7 / 255)
}

public struct TohsenoMark: View {
    public init() {}

    public var body: some View {
        Circle()
            .stroke(TohsenoTheme.amber, style: StrokeStyle(lineWidth: 3, lineCap: .round))
            .overlay(alignment: .topTrailing) {
                TohsenoTheme.void
                    .frame(width: 7, height: 8)
                    .rotationEffect(.degrees(18))
                    .offset(x: 1, y: -1)
            }
            .accessibilityHidden(true)
    }
}

struct PrimaryActionStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .fontWeight(.semibold)
            .foregroundStyle(TohsenoTheme.void)
            .padding(.horizontal, 18)
            .padding(.vertical, 9)
            .background(TohsenoTheme.amber.opacity(configuration.isPressed ? 0.78 : 1))
            .clipShape(RoundedRectangle(cornerRadius: 8))
    }
}
