/// SessionLink — RESERVED. Declared, not implemented.
///
/// The pattern this name reserves: a web page (for example the app's own
/// landing page) shows a QR code; the person scans it with the app and
/// confirms; the app signs a session challenge with its identity key and a
/// signed session now connects the browser to the app identity — login on
/// the web without an account, because the phone holds the keys.
///
/// This release ships the name only: the `AppConfig.sessionLinkEnabled`
/// flag (permanently false), the manifest's reserved `sessionLink` module
/// field, and the QR slot on the packaged landing page. Enabling it is
/// unsupported; there is deliberately no code here to flip on.
enum SessionLink {}
