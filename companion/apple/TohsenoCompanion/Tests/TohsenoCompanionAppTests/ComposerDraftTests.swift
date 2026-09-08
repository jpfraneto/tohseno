import Foundation
import Testing
import TohsenoCompanionKit
@testable import TohsenoCompanionApp

@MainActor
struct ComposerDraftTests {
    @Test("Draft text and exact images survive relaunch and stay in their workspace")
    func fileRoundTrip() throws {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: directory) }
        let image = try CompanionReferenceBlob(originName: "reference.png", mediaType: "image/png",
                                               bytes: Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")!)
        let first = try FileComposerDraftStorage(directory: directory)
        try first.save(key: "workspace_a/create", intention: "A calm timer", name: "", base: nil, images: [image])
        try first.save(key: "workspace_a/app", intention: "Keep my writing", name: "", base: shot(version: 3), images: [image])
        try first.save(key: "workspace_a/create", intention: "A calm breathing timer", name: "", base: nil, images: [image])
        let restored = try FileComposerDraftStorage(directory: directory)
        let (draft, images) = try #require(try restored.draft(for: "workspace_a/create"))
        #expect(draft.intention == "A calm breathing timer")
        #expect(images == [image])
        #expect(try restored.draft(for: "workspace_b/create") == nil)
        try restored.remove(key: "workspace_a/create")
        #expect(try restored.draft(for: "workspace_a/app")?.1 == [image], "A shared attachment belongs to the remaining draft")
    }

    @Test("Creation and app feedback restore independently after navigation and model relaunch")
    func navigationAndRelaunch() async throws {
        let store = MemoryComposerDraftStorage()
        let backend = StubBackend(shots: [shot(version: 3)])
        let subject = await model(backend)
        subject.draftStorage = store
        let app = try #require(subject.apps.first)
        subject.openCreate()
        subject.intent = "Create a timer"
        subject.open(app)
        subject.intent = "Keep the writing on screen"
        subject.openApps()
        subject.openCreate()
        #expect(subject.intent == "Create a timer")
        let relaunched = await model(backend)
        relaunched.draftStorage = store
        relaunched.open(app)
        #expect(relaunched.intent == "Keep the writing on screen")
        relaunched.openCreate()
        #expect(relaunched.intent == "Create a timer")
    }

    @Test("Draft feedback is preserved when its app advances and requires deliberate review")
    func movedBase() async throws {
        let backend = StubBackend(shots: [shot(version: 3)])
        let subject = await model(backend)
        subject.open(try #require(subject.apps.first))
        subject.intent = "Keep the writing on screen"
        await backend.set(shots: [shot(version: 4)])
        await subject.refresh()
        #expect(subject.draftNeedsReview)
        #expect(!subject.canEvolve)
        await subject.evolve()
        #expect(await backend.submissions.isEmpty)
        subject.useCurrentDraftBase()
        #expect(subject.canEvolve)
        #expect(subject.intent == "Keep the writing on screen")
    }

    @Test("Rejected submission retains the draft; a durable submission clears only that draft")
    func submission() async throws {
        let backend = StubBackend(shots: [shot(version: 3)])
        let subject = await model(backend)
        let app = try #require(subject.apps.first)
        subject.open(app)
        subject.intent = "App feedback"
        subject.openCreate()
        subject.intent = "Create a timer"
        await backend.rejectNext(with: .transportUnavailable)
        await subject.create()
        subject.openApps()
        subject.openCreate()
        #expect(subject.intent == "Create a timer")
        await subject.create()
        subject.openCreate()
        #expect(subject.intent.isEmpty)
        subject.open(app)
        #expect(subject.intent == "App feedback")
    }

    @Test("Completing a send after navigation cannot erase the newly active app draft")
    func navigationDuringSend() async throws {
        let backend = StubBackend(shots: [shot(version: 3)])
        await backend.pauseCreation()
        let subject = await model(backend)
        subject.openCreate()
        subject.intent = "Create a timer"
        let task = Task { await subject.create() }
        while await backend.creations.isEmpty { await Task.yield() }
        let app = try #require(subject.apps.first)
        subject.open(app)
        subject.intent = "New app feedback"
        await backend.resumeCreation()
        await task.value
        #expect(subject.screen == .app(app.shotID))
        #expect(subject.intent == "New app feedback")
        subject.openCreate()
        #expect(subject.intent.isEmpty)
        subject.open(app)
        #expect(subject.intent == "New app feedback")
    }
    @Test("Edits made while sending remain as a new unsent draft")
    func editsDuringSend() async {
        let backend = StubBackend()
        await backend.pauseCreation()
        let subject = await model(backend)
        subject.openCreate()
        subject.intent = "Create a timer"
        let task = Task { await subject.create() }
        while await backend.creations.isEmpty { await Task.yield() }
        subject.intent = "Create a different app next"
        await backend.resumeCreation()
        await task.value
        #expect(subject.screen == .create)
        #expect(subject.intent == "Create a different app next")
        subject.openApps()
        subject.openCreate()
        #expect(subject.intent == "Create a different app next")
    }

    @Test("Unread notification count and read decisions survive relaunch")
    func unreadNotificationsPersist() async throws {
        let suite = "tohseno.notifications.test.\(UUID().uuidString)"
        let storage = try #require(UserDefaults(suiteName: suite))
        defer { storage.removePersistentDomain(forName: suite) }
        let backend = StubBackend()
        let subject = CompanionModel(backend: backend, deviceName: "Test iPhone", storage: storage)
        await subject.refresh()
        let update = PrivateUpdateItem(kind: .evolutionFinished, subjectID: "shot_fixture", evidenceID: "execution_fixture",
            title: "App work completed", detail: "Your Mac has a new report.", occurredAt: "2026-09-08T12:00:00Z")
        subject.apply(WorkspaceEvent(eventID: "event_notifications", workspaceID: "workspace_fixture", cursor: 10,
            emittedAt: "2026-09-08T12:00:00Z",
            payload: .privateUpdates(PrivateUpdateProjection(items: [update], updatedAt: "2026-09-08T12:00:00Z"))))
        #expect(subject.unreadNotificationCount == 1)
        let reopened = CompanionModel(backend: backend, deviceName: "Test iPhone", storage: storage)
        await reopened.refresh()
        #expect(reopened.unreadNotificationCount == 1)
        reopened.setPrivateUpdateRead(update)
        #expect(reopened.unreadNotificationCount == 0)
        let again = CompanionModel(backend: backend, deviceName: "Test iPhone", storage: storage)
        await again.refresh()
        #expect(again.unreadNotificationCount == 0)
        #expect(again.privateUpdates.count == 1)
    }

}
