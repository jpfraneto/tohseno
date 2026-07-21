# Local development

## The site

Requires [Bun](https://bun.sh/). No accounts, no secrets, no database.

```sh
bun install
bun run dev        # http://localhost:3000
bun run check      # the before-commit gate
```

`.env.example` lists the only variables: `NODE_ENV`, `PORT`, `BASE_URL`,
`TRUST_PROXY`.

## The base app

Requires a Mac with Xcode.

```sh
open templates/continuity-app/Writing.xcodeproj    # ⌘R on any iPhone simulator
```

Run its invariant tests with ⌘U or:

```sh
cd templates/continuity-app
xcodebuild -project Writing.xcodeproj -scheme Writing \
  -destination 'platform=iOS Simulator,name=iPhone 16 Pro' test
```

The project file is generated: edit `project.yml`, then `xcodegen generate`
(`brew install xcodegen`).

## The oneshot, end to end

```sh
TOHSENO_REPO="$(pwd)" bash apps/site/public/oneshot.sh --target /tmp/test-app
```

Note the pin: a local run creates workspaces from the *pinned* commit, not
your working tree.
