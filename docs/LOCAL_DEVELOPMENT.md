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
UDID=$(xcrun simctl list devices available | grep -E '^[[:space:]]+iPhone' | grep -oE '[0-9A-F-]{36}' | head -1)
if [ -z "$UDID" ]; then
  echo "No available iPhone simulator; install one in Xcode → Settings → Platforms." >&2
  exit 1
fi
xcodebuild -project Writing.xcodeproj -scheme Writing \
  -destination "platform=iOS Simulator,id=$UDID" test
```

The project file is generated, not file-system-synced. Run `xcodegen generate`
after editing `project.yml` or adding, removing, or moving a Swift file
(`brew install xcodegen`).

## The oneshot, end to end

```sh
TOHSENO_REPO="$(pwd)" bash apps/site/public/oneshot.sh --target /tmp/test-app
```

Note the pin: a local run creates workspaces from the *pinned* commit, not
your working tree.
