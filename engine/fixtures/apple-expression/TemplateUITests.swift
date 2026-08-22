import XCTest

final class FixtureBirthUITests: XCTestCase {
    func testPrimaryContinuityJourney() {
        let app = XCUIApplication()
        app.launch()

        XCTAssertTrue(
            app.staticTexts["A TOHSENO expression"].waitForExistence(timeout: 10),
            "The target user must reach the primary expression."
        )
        let counter = app.buttons["tap-counter"]
        XCTAssertTrue(counter.waitForExistence(timeout: 10))
        let before = counter.label
        if app.buttons["reset-counter"].exists {
            XCTAssertNotEqual(
                before, "Count: 0",
                "Installing the evolution must preserve the count from the accepted app."
            )
        }
        counter.tap()
        let afterTap = counter.label
        XCTAssertNotEqual(before, afterTap)

        app.terminate()
        app.launch()
        let persisted = app.buttons["tap-counter"]
        XCTAssertTrue(persisted.waitForExistence(timeout: 10))
        XCTAssertEqual(persisted.label, afterTap, "The count must survive launch.")

        let reset = app.buttons["reset-counter"]
        if reset.exists {
            reset.tap()
            XCTAssertEqual(persisted.label, "Count: 0")
        }

        let evidence = XCTAttachment(screenshot: app.screenshot())
        evidence.name = "primary-continuity-journey"
        evidence.lifetime = .keepAlways
        add(evidence)
    }
}
