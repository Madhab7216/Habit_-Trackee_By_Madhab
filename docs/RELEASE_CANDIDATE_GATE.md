# Release candidate gate

This is a deliberately separate, manually invoked release pipeline. It does not run on ordinary feature pull requests and therefore cannot block day-to-day development while the project is still being built.

## How to run it

After the feature branches have been reviewed and integrated into a release-candidate commit:

1. Create a candidate tag matching the package version, such as `v0.1.0-rc.1`.
2. Push the tag to trigger the gate automatically. Manual **Actions → Release candidate gate → Run workflow** dispatch is available once the workflow has been merged into the default branch; select the candidate tag as `candidate_ref`.
3. Review the preflight result and every subsequent job.
4. Publish a GitHub Release only after the exact candidate tag passes all jobs.

The workflow checks out the tag itself, not the current default branch. All dependency-sensitive Cargo commands use `--locked`.

## What the gate checks

The preflight fails closed when any of these are missing or inconsistent:

- A non-empty, committed `Cargo.lock` containing the application package and version.
- An owner-selected project license in `Cargo.toml` and the corresponding license text file.
- Security disclosure policy, release readiness plan, pinned toolchain and the existing engineering/data-portability/SBOM workflows.
- Desktop launcher, AppStream metadata, icon and their validation scripts.
- An AppStream release version matching the Cargo package version.
- A candidate tag matching the package version.

If preflight passes, the workflow runs formatting, locked metadata resolution, all-target compile, Rust tests, strict Clippy, Python schema/SBOM/attestation-contract tests, desktop/AppStream validation, and CycloneDX SBOM generation. It then builds both release executables from the exact candidate tag, creates SLSA build-provenance attestations and CycloneDX SBOM attestations for both executables, verifies both predicates with GitHub CLI, and uploads the binaries, SBOM, metadata and SHA-256 checksums as release-candidate evidence.

## Current known blockers

The foundation branch reviewed on 17 November 2026 has no committed `Cargo.lock`, no project license text, and `license = "UNLICENSED"` in `Cargo.toml`. The packaging, SBOM and security files are still in separate draft PRs. The release gate is therefore expected to report blockers until those items are reviewed and integrated.

This PR does not invent or select a project license. The owner must choose one after considering redistribution, modification and contribution policy. The gate only verifies that an explicit choice and license text have been committed.

## Safety and permissions

- Read-only repository permission only.
- No repository writes, tagging, release publication, secret access, telemetry or user-data collection.
- The pipeline runs only on an explicitly selected candidate tag or a pushed `v*` tag.
- Build, tests and SBOM generation run on the trusted candidate tag, not arbitrary pull-request code.
- Both release executables receive signed SLSA build-provenance attestations and signed CycloneDX SBOM attestations. The workflow verifies both attestations before uploading evidence.
- The action is pinned to the immutable `actions/attest` v4.2.2 commit. The signing certificate is short-lived and uses Sigstore.
- GitHub documents artifact attestations as SLSA v1.0 Build Level 2. This workflow does not claim Build Level 3 or a reproducible-build guarantee.
- The uploaded binaries are raw Cargo executables, not a Debian package, AppImage or signed installer. Distribution packaging and real Linux Mint acceptance remain separate release gates.
- A green hosted pipeline does not replace the real Linux Mint installation, accessibility, upgrade and uninstall acceptance described in `docs/RELEASE_READINESS.md`.

## Local preflight

```bash
python3 scripts/check-release-readiness.py --ref v0.1.0-rc.1
```

Run it from a trusted release-candidate checkout. It reads the repository and does not write or modify files.

Official Cargo reference: [Cargo `--locked`](https://doc.rust-lang.org/cargo/commands/cargo-check.html). Cargo documents that `--locked` fails when the lockfile is absent or dependency resolution would change; this is why the release pipeline requires a committed lockfile.


## Signed release evidence

After the preflight and tests pass, the gate builds:

- `target/release/madhab-habit-tracker`
- `target/release/madhab-data-tool`

The two files are the subjects of:
1. a SLSA build-provenance attestation, recording the repository, workflow, triggering event and exact source commit; and
2. a CycloneDX SBOM attestation, binding the generated `sbom.json` to each executable.

The workflow verifies both predicates for both executable files before uploading the evidence bundle. The attestation workflow is restricted to pushed `v*` tags or a maintainer-triggered workflow dispatch; it does not issue attestations for pull-request builds. The signing steps receive only job-scoped permissions for read-only source access, OIDC token issuance and attestation storage.

### Verify after downloading a release evidence artifact

Install a current GitHub CLI and run these commands against the downloaded executable paths:

```bash
gh attestation verify target/release/madhab-habit-tracker \
  --repo Madhab7216/Habit_-Trackee_By_Madhab
gh attestation verify target/release/madhab-habit-tracker \
  --repo Madhab7216/Habit_-Trackee_By_Madhab \
  --predicate-type https://cyclonedx.org/bom

gh attestation verify target/release/madhab-data-tool \
  --repo Madhab7216/Habit_-Trackee_By_Madhab
gh attestation verify target/release/madhab-data-tool \
  --repo Madhab7216/Habit_-Trackee_By_Madhab \
  --predicate-type https://cyclonedx.org/bom
```

The first verification for each file checks build provenance; the second checks the CycloneDX SBOM predicate. A SHA-256 file alone proves only that a downloaded file matches a separately trusted checksum. The attestation adds a cryptographically signed claim about where and how the executable was built. Consumers must still verify the attestation and review the software; signing does not prove that the source code is vulnerability-free.

GitHub currently makes artifact attestations available on public repositories across current plans. This repository is public. Private repositories on GitHub Free, Pro or Team would not have this feature; GitHub Enterprise Cloud is required for private/internal repository attestations.
