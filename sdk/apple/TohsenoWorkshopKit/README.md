# TohsenoWorkshopKit

`TohsenoWorkshopKit` is the small Swift boundary for a Shot that intentionally
uses the owner's live local Workshop. It exposes authenticated Session devices,
truthful capability state, and typed ephemeral events. It does not expose
pairing keys, durable commands, Claim, Ship, Update, installation, publication,
payment, or device revocation.

Focused apps do not need this package. If an app's ordinary iPhone target is
enough, add no declaration and no Workshop dependency.

## Join an installed Session

```swift
import TohsenoWorkshopKit

let session = try await TohsenoWorkshop.current.join()
let devices = await session.devices

let payload = try JSONEncoder().encode(["direction": "left"])
try await session.send(WorkshopEvent(
    type: "my_app.controller_move",
    payload: payload
))

for await envelope in session.events {
    // Decode only event types owned by this app.
}
```

`join()` succeeds only when a trusted host product has installed a real
Session. The package never creates a standalone unpaired network session as a
fallback.

## Read capability truth

```swift
let camera = devices
    .first(where: { $0.platform == .iPhone })?
    .capability(.camera)

if camera?.ready == true {
    // The hardware, permission, reachability, and Session authorization agree.
} else {
    let reason = camera?.explanation ?? "No iPhone is in this Session"
}
```

Identity, hardware, operating-system permission, reachability, and Session
authorization are separate fields. Never infer one from another and never ask
for a permission merely to make a capability appear ready.

## Optional multi-surface declaration

`WorkshopShotDeclaration` uses schema `tohseno.workshop-shot/1`. It is ordinary
source-visible app metadata and is optional. For example:

```swift
let declaration = WorkshopShotDeclaration(
    surfaces: [
        WorkshopSurfaceDeclaration(
            role: "controller",
            platform: .iPhone,
            required: [.touch],
            preferred: [.motion]
        ),
        WorkshopSurfaceDeclaration(
            role: "display",
            platform: .macOS,
            required: [.display, .compute]
        ),
    ],
    session: WorkshopSessionDeclaration(realtime: true)
)

let resolution = WorkshopResolver.resolve(
    declaration: declaration,
    devices: devices
)
guard resolution.runnable else {
    // Explain the missing required capability; do not simulate it.
    return
}
```

A nil declaration resolves to the existing focused mode and remains runnable.
Every declared required surface fails closed until one connected device has all
required capabilities ready. Preferred capabilities never become requirements.

## Event constraints

Events are versioned, limited to 1 MiB, encrypted after the paired-device
handshake, and sequenced against replay. Use a stable lowercase app namespace.
Authority-related namespaces are rejected. Session events are not durable and
may be lost on disconnect; use the existing Tohseno Companion command path for
anything that must survive, reconcile, or receive human authorization.
