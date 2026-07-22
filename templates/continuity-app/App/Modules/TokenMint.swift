/// TokenMint — RESERVED. Declared, not implemented.
///
/// The pattern this name reserves: a tiny owner-deployed service keeps the
/// long-lived provider secret and exchanges a non-secret client request for a
/// short-lived provider credential. The mint receives no user content and is
/// not an application backend; generated apps remain independent of TOHSENO.
///
/// This release ships only the `AppConfig.tokenMintEnabled` flag (permanently
/// false) and the manifest's reserved `tokenMint` module field. Enabling it is
/// unsupported. A local prototype may use `AppConfig.developmentSecret` on an
/// owner-controlled Debug device, but must replace that exception with this
/// pattern before TestFlight or any other distribution.
enum TokenMint {}
