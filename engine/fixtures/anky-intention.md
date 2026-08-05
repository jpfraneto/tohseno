Build a production-quality native iPhone app called Anky.

The target experience has two actors:
1. a parent who handles setup, consent, permissions, privacy, and diagnostics;
2. a young child who experiences the app.

The phone is a magical window into the child’s real home.

The rear camera is open during the child experience.

Use ARKit and RealityKit to place a small creature at a plausible spatial hiding location. Use Core Motion, spatial or reactive audio, Core Haptics, particles, light, and visual traces to communicate warmer and colder as the child physically searches.

Anky begins without language. After discovery it listens through the microphone, reacts to the child’s voice, attempts one constrained imperfect act of mimicry, stores one sparse relationship memory locally, and reflects that memory in a later encounter.

Use SwiftData for local persistence.

LiDAR-capable phones receive scene reconstruction and stronger occlusion. Standard ARKit phones receive plane/world-tracking behavior. A motion-relative Tier C experience exists only as a runtime fallback when spatial tracking genuinely fails; it is not the primary product merely because Simulator lacks AR input.

The experience is finite, safe for child movement, private by default, and complete from parent setup through the second memory-bearing encounter.
