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
    private let stroke: Color
    private let gap: Color

    public init(stroke: Color = TohsenoTheme.amber, gap: Color = TohsenoTheme.void) {
        self.stroke = stroke
        self.gap = gap
    }

    public var body: some View {
        Circle()
            .stroke(stroke, style: StrokeStyle(lineWidth: 3, lineCap: .round))
            .overlay(alignment: .topTrailing) {
                gap
                    .frame(width: 7, height: 8)
                    .rotationEffect(.degrees(18))
                    .offset(x: 1, y: -1)
            }
            .accessibilityHidden(true)
    }
}

public struct TohsenoSpinner: View {
    private let size: CGFloat
    private let stroke: Color
    private let gap: Color
    @State private var isSpinning = false

    public init(
        size: CGFloat = 28,
        stroke: Color = TohsenoTheme.amber,
        gap: Color = TohsenoTheme.void
    ) {
        self.size = size
        self.stroke = stroke
        self.gap = gap
    }

    public var body: some View {
        TohsenoMark(stroke: stroke, gap: gap)
            .frame(width: size, height: size)
            .rotationEffect(.degrees(isSpinning ? 360 : 0))
            .animation(.linear(duration: 0.9).repeatForever(autoreverses: false), value: isSpinning)
            .onAppear { isSpinning = true }
            .accessibilityHidden(true)
    }
}

/// The mark at the product's front door. It breathes instead of spinning so
/// first contact feels alive without implying that a technical task is stuck.
public struct TohsenoLivingMark: View {
    private let size: CGFloat
    private let animated: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var isBreathing = false

    public init(size: CGFloat = 96, animated: Bool = true) {
        self.size = size
        self.animated = animated
    }

    public var body: some View {
        ZStack {
            Circle()
                .fill(TohsenoTheme.amber.opacity(0.08))
                .frame(width: size * 1.34, height: size * 1.34)
                .scaleEffect(isBreathing && animated && !reduceMotion ? 1.08 : 0.94)
                .opacity(isBreathing && animated && !reduceMotion ? 0.3 : 0.72)

            Circle()
                .stroke(TohsenoTheme.amber.opacity(0.2), lineWidth: 1)
                .frame(width: size * 1.16, height: size * 1.16)
                .scaleEffect(isBreathing && animated && !reduceMotion ? 1.16 : 0.9)
                .opacity(isBreathing && animated && !reduceMotion ? 0.04 : 0.52)

            TohsenoMark()
                .frame(width: size, height: size)
                .rotationEffect(.degrees(isBreathing && animated && !reduceMotion ? 7 : -3))
                .scaleEffect(isBreathing && animated && !reduceMotion ? 1.025 : 0.985)
                .shadow(color: TohsenoTheme.amber.opacity(0.2), radius: 18)
        }
        .frame(width: size * 1.4, height: size * 1.4)
        .animation(
            reduceMotion || !animated
                ? nil
                : .easeInOut(duration: 2.8).repeatForever(autoreverses: true),
            value: isBreathing
        )
        .onAppear { isBreathing = true }
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
