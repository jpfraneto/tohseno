import SwiftUI
import TohsenoCompanionKit

#if canImport(PhotosUI) && os(iOS)
import PhotosUI
#endif

/// Optional image references, kept visually secondary to the intention.
struct ScreenshotPicker: View {
    @Binding var attachments: [CompanionReferenceBlob]

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 10) {
                picker
                ForEach(attachments, id: \.blobID) { blob in
                    AttachmentThumbnail(blob: blob) {
                        attachments.removeAll { $0.blobID == blob.blobID }
                    }
                }
            }
        }
    }

#if canImport(PhotosUI) && os(iOS)
    @State private var selection: [PhotosPickerItem] = []

    private var picker: some View {
        let hasAttachments = !attachments.isEmpty
        return PhotosPicker(
            selection: $selection,
            maxSelectionCount: CompanionAttachments.maximumCount,
            matching: .images
        ) {
            ScreenshotPickerLabel(hasAttachments: hasAttachments)
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
        ScreenshotPickerLabel(hasAttachments: !attachments.isEmpty)
    }
#endif

}

private struct ScreenshotPickerLabel: View {
    let hasAttachments: Bool

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "photo.badge.plus")
                .font(.system(size: 17, weight: .medium))
            Text(hasAttachments ? "Add more" : "Add images")
                .font(.system(size: 15, weight: .medium))
        }
        .foregroundStyle(Tohseno.bone)
        .padding(.horizontal, 14)
        .frame(height: 56)
        .background(Tohseno.carbon, in: RoundedRectangle(cornerRadius: 13, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 13, style: .continuous)
                .strokeBorder(Tohseno.iron)
        )
    }
}

private struct AttachmentThumbnail: View {
    let blob: CompanionReferenceBlob
    let remove: () -> Void

    var body: some View {
        ZStack(alignment: .topTrailing) {
            Group {
#if canImport(UIKit)
                if let image = UIImage(data: blob.bytes) {
                    Image(uiImage: image)
                        .resizable()
                        .scaledToFill()
                } else {
                    fallback
                }
#else
                fallback
#endif
            }
            .frame(width: 56, height: 56)
            .clipShape(RoundedRectangle(cornerRadius: 13, style: .continuous))

            Button(action: remove) {
                Image(systemName: "xmark.circle.fill")
                    .symbolRenderingMode(.palette)
                    .foregroundStyle(Tohseno.bone, Color.black.opacity(0.72))
                    .font(.system(size: 18))
            }
            .offset(x: 6, y: -6)
            .accessibilityLabel("Remove \(blob.originName)")
        }
        .padding(.top, 6)
        .padding(.trailing, 6)
    }

    private var fallback: some View {
        RoundedRectangle(cornerRadius: 13, style: .continuous)
            .fill(Tohseno.iron)
            .overlay(Image(systemName: "photo").foregroundStyle(Tohseno.ash))
    }
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
