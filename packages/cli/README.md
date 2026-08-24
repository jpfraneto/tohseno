# TOHSENO npm bootstrap

`tohseno@1.0.0` is the dependency-free Node 20+ front door for the native
TOHSENO 1.0.0 Mac product. It is not the factory: it locates or securely
installs the authorized native release in the existing no-sudo
`~/.tohseno/` layout and delegates to `~/.tohseno/bin/tohseno`.

The production manifest is fixed at
`https://tohseno.com/releases/native-v1.json`. Publication remains
fail-closed until the native release, exact sizes/digests, compatibility, and
signing policy are authorized there. No environment variable can replace the
production manifest URL.

Development:

```sh
cd packages/cli
npm test
node test/pack-install.test.js
npm pack --dry-run
```

Publishing is a manual owner action documented in
`../../docs/runbooks/NPM_1_0_0.md`. This repository does not publish or change
the npm dist-tag automatically.
