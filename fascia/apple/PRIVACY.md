# Privacy

The default is no telemetry, no tracking, no account, no silent linkage, and
no network use.

Every network endpoint and purpose must be declared in the per-app
`fascia.json`. Every protected API and entitlement must map to the finite
capability vocabulary in `FASCIA.json`. An undeclared sensitive capability is
a conformance failure.

Private by default:

- prompts and reference images;
- unpublished source and Shot sidecars;
- application content and usage;
- continuity relationships;
- installation private material;
- Builder device inventory and recovery material.

Public only after an explicit Builder-signed action:

- Shot commitment and public lineage head;
- controller and publication state;
- deliberately selected source or manifest URI;
- handle, appcoin relation, or App Store attestation.

InstallationIdentity never creates a cross-app tracking key. Continuity is
pairwise or audience-scoped, expires, and is disclosed only through a
user-chosen transport.
