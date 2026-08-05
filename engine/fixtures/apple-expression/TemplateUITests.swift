import XCTest

final class FixtureBirthUITests: XCTestCase {
    func testPrimaryContinuityJourney() {
        let app = XCUIApplication()
        app.launch()

        XCTAssertTrue(
            app.staticTexts["A TOHSENO expression"].waitForExistence(timeout: 10),
            "The target user must reach the primary expression."
        )
        XCTAssertTrue(
            app.staticTexts["This fixture passes the real Apple materialization gates."].exists,
            "The complete bounded promise must be visible without explanation."
        )

        let evidence = XCTAttachment(screenshot: app.screenshot())
        evidence.name = "primary-continuity-journey"
        evidence.lifetime = .keepAlways
        add(evidence)
    }
}
