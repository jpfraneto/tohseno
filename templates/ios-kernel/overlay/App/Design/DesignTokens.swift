import SwiftUI

/// A deliberately small, app-owned seam. It carries no TOHSENO visual brand.
enum DesignTokens {
    enum Spacing {
        static let small: CGFloat = 8
        static let medium: CGFloat = 16
        static let large: CGFloat = 24
    }

    enum Typography {
        static let title: Font = .system(.largeTitle, design: .rounded, weight: .bold)
        static let body: Font = .system(.body, design: .default)
        static let metric: Font = .system(.title2, design: .rounded, weight: .semibold)
    }
}
