import SwiftUI
import TohsenoCompanionKit

#if canImport(PhotosUI) && os(iOS)
import PhotosUI
#endif

/// "+ Add screenshots" — optional, secondary, and never in the way of the box.
struct ScreenshotPicker: View {
    @Binding var attachments: [CompanionReferenceBlob]

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            picker
            if !attachments.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(attachments, id: \.blobID) { blob in
                            HStack(spacing: 8) {
                                Text(blob.originName)
                                    .font(.system(size: 13))
                                    .foregroundStyle(Tohseno.bone)
                                    .lineLimit(1)
                                Button {
                                    attachments.removeAll { $0.blobID == blob.blobID }
                                } label: {
                                    Image(systemName: "xmark")
                                        .font(.system(size: 11, weight: .semibold))
                                        .foregroundStyle(Tohseno.ash)
                                }
                            }
                            .padding(.horizontal, 12)
                            .padding(.vertical, 8)
                            .overlay(Capsule().strokeBorder(Tohseno.iron))
                        }
                    }
                }
            }
        }
    }

#if canImport(PhotosUI) && os(iOS)
    @State private var selection: [PhotosPickerItem] = []

    private var picker: some View {
        PhotosPicker(
            selection: $selection,
            maxSelectionCount: CompanionAttachments.maximumCount,
            matching: .images
        ) {
            Text("+ Add screenshots")
                .font(.system(size: 15))
                .foregroundStyle(Tohseno.ash)
        }
        .onChange(of: selection) { _, items in
            Task { await adopt(items) }
        }
    }

    private func adopt(_ items: [PhotosPickerItem]) async {
        var loaded: [CompanionReferenceBlob] = []
        for (index, item) in items.prefix(CompanionAttachments.maximumCount).enumerated() {
            guard let data = try? await item.loadTransferable(type: Data.self),
                  let blob = CompanionAttachments.blob(from: data, index: index)
            else { continue }
            loaded.append(blob)
        }
        attachments = loaded
    }
#else
    private var picker: some View {
        Text("+ Add screenshots")
            .font(.system(size: 15))
            .foregroundStyle(Tohseno.ash)
    }
#endif
}

/// Turning picked bytes into exactly the reference blobs the Mac accepts.
public enum CompanionAttachments {
    public static let maximumCount = 8

    /// PNG and JPEG are recognized by their own leading bytes rather than by a
    /// filename or a picker's claim about them.
    public static func mediaType(of data: Data) -> String? {
        if data.starts(with: [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) { return "image/png" }
        if data.starts(with: [0xFF, 0xD8, 0xFF]) { return "image/jpeg" }
        return nil
    }

    public static func blob(from data: Data, index: Int) -> CompanionReferenceBlob? {
        guard let mediaType = mediaType(of: data) else { return nil }
        let suffix = mediaType == "image/png" ? "png" : "jpg"
        return try? CompanionReferenceBlob(
            originName: "screenshot-\(index + 1).\(suffix)",
            mediaType: mediaType,
            bytes: data
        )
    }
}
