# TOHSENO doctrine

Five pillars. Everything else is implementation.

## 1. Speed is the product

Prompt to phone, measured in minutes. Every design decision that adds a
question, a config step, or a ceremony must pay for itself in reliability.
That is why the workspace is already a compiling app, why the agent asks at
most three questions and records the rest as `ASSUMED`, and why the manifest
bounds the feature space — a bounded space is why one-shots land.

## 2. Cryptography kills auth

Every generated app ships with a locally generated BIP39 seed phrase as its
identity. First launch = identity exists, silently. Recovery is the phrase.
No accounts, ever — unless the builder explicitly demands them, and then the
agent warns once that this breaks the model, and complies.

## 3. Private by default, ejectable from birth

Data lives on the device as plain files. Anything that leaves the device is
declared in the manifest. Every app builds and runs without TOHSENO
credentials; nobody stays because leaving is hard.

## 4. Ship to share

Every app has a share primitive — a locally rendered share card — because
distribution is part of the app, not an afterthought. The landing page ships
in the same package as the app: one static file, no build step.

## 5. The builder decides the mechanics

Streaks, paywalls, scores, virality loops — tools, not sins. TOHSENO gives
the spine: identity, persistence, modules, ejectability. The builder gives
the personality. Defaults stay private and account-free; they are defaults,
never refusals.
