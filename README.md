# TOHSENO

TOHSENO is a printing press for iOS apps: describe one app, watch your coding agent build it, and find it running on the iPhone connected to your Mac.

```sh
curl -fsSL https://tohseno.com/oneshot.sh | bash
tohseno create replyguy-trencher
```

The loop is deliberately small: create shot 1, use the app, then run `tohseno evolve <app-name>` for shot 2. `tohseno refresh` makes free Apple ID expiry routine, `tohseno list` shows the local ledger, and `tohseno studio` opens the same press on localhost.

Every shot is a complete, append-only Xcode project. Its integer shot number is its `CFBundleVersion`; the filesystem under `~/.tohseno/` is the database; generated apps are iOS 17+ SwiftUI with no third-party dependencies; installation is USB-only through Apple’s tools; and the coding harness is chosen in `~/.tohseno/config.toml`.

TOHSENO is free Apache-2.0 software. A free Apple ID is the default path; Apple’s paid developer membership is only for permanence or App Store publishing.

