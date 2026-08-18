import SwiftUI

/// Black, chemical orange, and space — the same identity as the Mac, sized for
/// a thumb. There is no chrome to theme because there is no dashboard.
public enum Tohseno {
    public static let void = Color(red: 0.035, green: 0.035, blue: 0.031)
    public static let carbon = Color(red: 0.063, green: 0.063, blue: 0.059)
    public static let iron = Color(red: 0.188, green: 0.188, blue: 0.169)
    public static let bone = Color(red: 0.957, green: 0.941, blue: 0.902)
    public static let ash = Color(red: 0.443, green: 0.435, blue: 0.408)
    public static let orange = Color(red: 1.0, green: 0.392, blue: 0.118)
}

struct PrimaryButtonStyle: ButtonStyle {
    var enabled = true

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 17, weight: .semibold))
            .foregroundStyle(enabled ? Tohseno.void : Tohseno.ash)
            .padding(.vertical, 15)
            .padding(.horizontal, 28)
            .background(enabled ? Tohseno.orange : Tohseno.iron, in: Capsule())
            .opacity(configuration.isPressed ? 0.8 : 1)
    }
}

struct WordmarkView: View {
    var body: some View {
        HStack(spacing: 10) {
            Circle().fill(Tohseno.orange).frame(width: 8, height: 8)
            Text("TOHSENO")
                .font(.system(size: 13, weight: .bold))
                .kerning(4)
                .foregroundStyle(Tohseno.bone)
        }
    }
}
