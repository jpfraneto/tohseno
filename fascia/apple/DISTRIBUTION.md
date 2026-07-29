# Distribution

Distribution is metadata about a Shot, not its identity.

Every app records:

- bundle identifier;
- integer `CFBundleVersion`, exactly equal to the Evolution sequence;
- Apple surfaces derived from the Xcode target;
- `local`, `published`, or `app_store` state;
- optional App Store identifier only after an explicit signed attestation.

Apple signing, provisioning, and Apple ID may be required to install or
publish an Apple app. They do not determine BuilderID, Shot ownership,
continuity, or TOHSENO membership.

Publication is an explicit signed protocol action. Installing an app locally
does not publish its prompt, source, metadata, or usage. App Store graduation
does not replace ShotID or signed lineage.
