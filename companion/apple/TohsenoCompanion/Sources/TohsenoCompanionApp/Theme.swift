import SwiftUI
#if canImport(UIKit)
import UIKit
#elseif canImport(AppKit)
import AppKit
#endif

/// Menlo's paper, ink, and forest-green identity, shared with the Mac.
public enum Tohseno {
    public static let void = Color(red: 246 / 255, green: 243 / 255, blue: 234 / 255)
    public static let carbon = Color(red: 255 / 255, green: 253 / 255, blue: 247 / 255)
    public static let iron = Color(red: 214 / 255, green: 213 / 255, blue: 201 / 255)
    public static let bone = Color(red: 36 / 255, green: 40 / 255, blue: 32 / 255)
    public static let ash = Color(red: 97 / 255, green: 100 / 255, blue: 87 / 255)
    public static let orange = Color(red: 49 / 255, green: 91 / 255, blue: 59 / 255)
    public static let connected = Color(red: 49 / 255, green: 91 / 255, blue: 59 / 255)
    public static let warning = Color(red: 136 / 255, green: 92 / 255, blue: 19 / 255)
    public static let failed = Color(red: 165 / 255, green: 50 / 255, blue: 39 / 255)
}

struct PrimaryButtonStyle: ButtonStyle {
    var enabled = true

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 17, weight: .semibold))
            .foregroundStyle(enabled ? Tohseno.void : Tohseno.ash)
            .padding(.vertical, 15)
            .padding(.horizontal, 28)
            .frame(maxWidth: .infinity)
            .background(enabled ? Tohseno.orange : Tohseno.iron, in: RoundedRectangle(cornerRadius: 6))
            .opacity(configuration.isPressed ? 0.8 : 1)
    }
}

struct TohsenoMark: View {
    var size: CGFloat

    var body: some View {
        ZStack(alignment: .topLeading) {
            Path { path in
                let scale = size / 48
                path.addRect(CGRect(x: 4 * scale, y: 4 * scale, width: 28 * scale, height: 28 * scale))
                path.addRect(CGRect(x: 10 * scale, y: 10 * scale, width: 16 * scale, height: 16 * scale))
            }.fill(Tohseno.bone, style: FillStyle(eoFill: true))
            Rectangle().fill(Tohseno.bone)
                .frame(width: size / 2, height: size / 2)
                .offset(x: size * 20 / 48, y: size * 20 / 48)
        }.frame(width: size, height: size).accessibilityHidden(true)
    }
}

struct WordmarkView: View {
    var body: some View {
        HStack(spacing: 10) {
            TohsenoMark(size: 28)
            Text("menlo")
                .font(.system(size: 32, weight: .semibold, design: .serif))
                .kerning(-1)
                .foregroundStyle(Tohseno.bone)
        }
    }
}
