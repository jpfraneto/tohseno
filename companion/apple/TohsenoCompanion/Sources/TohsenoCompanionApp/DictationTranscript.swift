import Foundation

/// A recognizer may finish an utterance without finishing its audio task, then
/// return only the next utterance. Keep that boundary separate from revisions
/// to the words currently being recognized.
struct DictationTranscript: Sendable {
    private var prefix: String
    private var utterance = ""
    private var completed = false
    private var endTime: TimeInterval?

    init(initialText: String) { prefix = initialText }

    mutating func receive(
        _ text: String, completed: Bool, startTime: TimeInterval?, endTime: TimeInterval?
    ) -> String {
        guard !text.isEmpty else { return value }
        let followsPrevious = if let startTime, let previousEnd = self.endTime {
            startTime > 0 && startTime >= previousEnd && previousEnd > 0
        } else { false }
        // Unstable segments can report zero timestamps. A completed -> partial
        // transition is also an utterance boundary on on-device recognition.
        let beginsNewUtterance = followsPrevious
            || (self.completed && !completed && !text.hasPrefix(utterance))
        if beginsNewUtterance && !utterance.isEmpty {
            prefix = joined(prefix, utterance)
            utterance = ""
        }
        utterance = text
        self.completed = completed
        self.endTime = endTime
        return value
    }

    var value: String { joined(prefix, utterance) }

    private func joined(_ first: String, _ second: String) -> String {
        guard !first.isEmpty else { return second }
        guard !second.isEmpty else { return first }
        return first + (first.last?.isWhitespace == true ? "" : " ") + second
    }
}
