# TOHSENO npm bootstrap

`tohseno@1.1.0` is the dependency-free Node 20+ front door for the native
TOHSENO 1.1.0 Mac product. It is not the factory: it locates or securely
installs the authorized native release in the existing no-sudo
`~/.tohseno/` layout and delegates to `~/.tohseno/bin/tohseno`.

On a fresh global Mac installation, npm immediately runs that verified front
door. It installs the native release, starts the Local Workspace Service, and
opens the first-run cable guide. Local dependency installs and global updates
that already have a native installation do not open first run.

The npm and native product versions are exact peers. The 1.1.0 package refuses
an older public native manifest, so npm cannot claim the release before the
signed native artifact and its exact manifest are public.

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
