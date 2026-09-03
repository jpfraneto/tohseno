# TOHSENO CLI for npm

`tohseno@1.2.1` is the dependency-free Node 20+ CLI installer for TOHSENO
1.2.1 on macOS.

```sh
npm install --global tohseno
cd /path/to/YourApp
tohseno init
tohseno deploy
```

npm installs only the small command launcher. It does not download a runtime,
start a service, set up Companion, or open a GUI during install. Running
`tohseno` with no arguments shows the commands above. Interactive `init` then
explains the real path one line at a time and waits for Enter between steps.
Before touching the Xcode project, it verifies that the intended iPhone's real
installed-app inventory contains `com.tohseno.companion` and that its one-use
private pairing completed. If not, it directs the user to
`tohseno companion install` and stops.

When a real command first needs the compiled runtime, the launcher downloads
the exact architecture from a fixed HTTPS manifest, verifies byte length,
SHA-256, the closed release tree and checksums, and the declared Apple
Developer ID requirement, then activates it in the existing no-sudo
`~/.tohseno/` layout. It does not install `Tohseno.app`.

The production manifest is fixed at
`https://tohseno.com/releases/cli-v1.json`. Publication remains fail-closed
until the command runtime, exact sizes/digests, compatibility, and signing
policy are authorized there. No environment variable can replace the
production manifest URL.

Development:

```sh
cd packages/cli
npm test
node test/pack-install.test.js
npm pack --dry-run
```

Publishing remains an owner-authorized npm action. This repository does not
change the npm dist-tag automatically.
