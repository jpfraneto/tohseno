import Testing
@testable import TohsenoCompanionApp

@Test func dictationKeepsUtterancesAcrossPausesAndRevisesOnlyCurrentWords() {
    var text = DictationTranscript(initialText: "My idea:")
    #expect(text.receive("Build a time", completed: false, startTime: 0, endTime: 0) == "My idea: Build a time")
    #expect(text.receive("Build a timer.", completed: true, startTime: 0.2, endTime: 2) == "My idea: Build a timer.")
    #expect(text.receive("Make", completed: false, startTime: 0, endTime: 0) == "My idea: Build a timer. Make")
    #expect(text.receive("Make it orange.", completed: true, startTime: 4, endTime: 6) == "My idea: Build a timer. Make it orange.")
    #expect(text.receive("And add", completed: false, startTime: 0, endTime: 0) == "My idea: Build a timer. Make it orange. And add")
    #expect(text.receive("And add sound.", completed: true, startTime: 8, endTime: 10) == "My idea: Build a timer. Make it orange. And add sound.")
    #expect(text.receive("", completed: false, startTime: nil, endTime: nil) == text.value)
}

@Test func dictationDoesNotDuplicateCumulativeOrRepeatedResults() {
    var text = DictationTranscript(initialText: "")
    _ = text.receive("A timer", completed: true, startTime: 1, endTime: 2)
    #expect(text.receive("A timer", completed: true, startTime: 1, endTime: 2) == "A timer")
    #expect(text.receive("A timer with sound", completed: false, startTime: 1, endTime: 4) == "A timer with sound")
    #expect(text.receive("A timer with sound.", completed: true, startTime: 1, endTime: 4) == "A timer with sound.")
    #expect(text.receive("Again.", completed: true, startTime: 6, endTime: 7) == "A timer with sound. Again.")
}

@Test func anotherRecordingKeepsExistingDraftFormatting() {
    var text = DictationTranscript(initialText: "Existing notes.\n\n")
    #expect(text.receive("Another idea", completed: false, startTime: 0, endTime: 0) == "Existing notes.\n\nAnother idea")
}
