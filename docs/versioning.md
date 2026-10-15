# Versioning

notez follows [Semantic Versioning](https://semver.org/). Every change worth
noting goes into [CHANGELOG.md](../CHANGELOG.md), under `[Unreleased]` until
the owner cuts a release.

- **One version.** `notez-core` and `notez-cli` share the workspace version,
  set once in `[workspace.package]` in the root `Cargo.toml`.
- **0.x.** While the major version is 0, a minor bump (0.1 to 0.2) may
  break things. A patch bump (0.1.0 to 0.1.1) is for fixes only.
- **notez-core is a contract.** epoz pins `notez-core` by git rev and reads
  the changelog to decide when to move its pin. So any change to the
  `notez-core` public API, or to an on-disk format, is at least a minor bump
  and gets an entry under a `notez-core` subsection. On-disk formats are
  the note layout, `.tags`, the `TODO.md` conventions, the config and
  registry TOML, and `.notez-config.toml`.
- **Tags and releases are the owner's.** A release is an annotated tag,
  `git tag -a vX.Y.Z`, made by Andreas after the version bump and the
  changelog entry are on `main`. No agent tags or releases.
- **Which build is installed.** `notez --version` prints the version and the
  commit it was built from, for example `notez 0.1.0 (7cfeb27)`, with
  `-dirty` when the build had uncommitted changes. A build outside a git
  checkout prints the plain version.
