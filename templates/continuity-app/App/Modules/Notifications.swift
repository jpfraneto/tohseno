import UserNotifications

/// Notifications module — flag-gated, OFF by default
/// (`AppConfig.notificationsEnabled`).
///
/// When enabled, Settings offers a daily reminder. Permission is requested
/// at that moment — never at launch, never before the person has written.
/// When the flag is off this file still compiles and nothing is scheduled.
enum Notifications {
    private static let reminderIdentifier = "daily-writing-reminder"

    /// Requests permission and schedules a daily reminder at the given hour.
    static func enableDailyReminder(hour: Int) async -> Bool {
        guard AppConfig.notificationsEnabled else { return false }
        let center = UNUserNotificationCenter.current()
        let granted = (try? await center.requestAuthorization(options: [.alert, .sound])) ?? false
        guard granted else { return false }

        let content = UNMutableNotificationContent()
        content.title = "Write"
        content.body = "The page is ready."
        var components = DateComponents()
        components.hour = hour
        let trigger = UNCalendarNotificationTrigger(dateMatching: components, repeats: true)
        let request = UNNotificationRequest(identifier: reminderIdentifier, content: content, trigger: trigger)
        try? await center.add(request)
        return true
    }

    static func disableDailyReminder() {
        UNUserNotificationCenter.current()
            .removePendingNotificationRequests(withIdentifiers: [reminderIdentifier])
    }
}
