import SwiftUI

public enum TohsenoTheme {
    public static let void = Color(red: 246 / 255, green: 243 / 255, blue: 234 / 255)
    public static let carbon = Color(red: 255 / 255, green: 253 / 255, blue: 247 / 255)
    public static let graphite = Color(red: 225 / 255, green: 232 / 255, blue: 216 / 255)
    public static let iron = Color(red: 214 / 255, green: 213 / 255, blue: 201 / 255)
    public static let ash = Color(red: 97 / 255, green: 100 / 255, blue: 87 / 255)
    public static let silver = Color(red: 97 / 255, green: 100 / 255, blue: 87 / 255)
    public static let bone = Color(red: 36 / 255, green: 40 / 255, blue: 32 / 255)
    public static let amber = Color(red: 49 / 255, green: 91 / 255, blue: 59 / 255)
    public static let ember = Color(red: 225 / 255, green: 232 / 255, blue: 216 / 255)
}

public struct TohsenoMark: View {
    private let stroke: Color
    private let gap: Color

    public init(stroke: Color = TohsenoTheme.amber, gap: Color = TohsenoTheme.void) {
        self.stroke = stroke
        self.gap = gap
    }

    public var body: some View {
        GeometryReader { geometry in
            Path { path in
                let scale = min(geometry.size.width, geometry.size.height) / 48
                path.addRect(CGRect(x: 4 * scale, y: 4 * scale, width: 28 * scale, height: 28 * scale))
                path.addRect(CGRect(x: 10 * scale, y: 10 * scale, width: 16 * scale, height: 16 * scale))
            }
            .fill(stroke, style: FillStyle(eoFill: true))
            Rectangle().fill(stroke)
                .frame(width: geometry.size.width / 2, height: geometry.size.height / 2)
                .offset(x: geometry.size.width * 20 / 48, y: geometry.size.height * 20 / 48)
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
