# Shared vectors

The monorepo test suite reads the single source of truth at
`companion/test-vectors/companion-v1.json`. The deterministic vendoring helper
copies that exact file into this directory when CompanionKit is embedded in a
Shot, allowing the vendored package to run its conformance tests without a
mutable dependency on `~/.tohseno/current`.
