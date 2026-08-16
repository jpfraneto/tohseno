# TOHSENO

TOHSENO records an app beside its filesystem.

The app remains an ordinary folder that any editor, coding agent, or build tool
can change. TOHSENO adds a `.tohseno/` directory inside it and records immutable
Versions of those files. It does not own the development workflow.

```bash
tohseno create my-app
# edit my-app with whatever tools you use
tohseno evolve my-app --note "Describe this version"
```

`create` initializes a new app folder, or adopts an existing folder, without
running inference or building anything. `evolve` records the folder's current
state. `.git/` and `.tohseno/` are metadata and are never copied into a Version.

Use Studio for the same local loop:

```bash
tohseno studio
```

TOHSENO does not publish source, contact a blockchain, deploy a contract, or
launch a token while initializing or recording an app.

## Documentation

- [Current repository state](docs/STATE.md)
- [Architecture decisions](docs/adr/README.md)
- [Protocol specification](protocol/SPECIFICATION.md)
- [Protocol conformance](protocol/CONFORMANCE.md)
- [Frozen history](history/README.md)

Historical protocol records and their exact encodings remain supported for
verification. They are not the current product interface.
