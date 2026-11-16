# Release candidate gate

This is a deliberately separate, manually invoked release pipeline. It does not run on ordinary feature pull requests and therefore cannot block day-to-day development while the project is still being built.

## How to run it

After the feature branches have been reviewed and integrated into a release-candidate commit:

1. Create a candidate tag matching the package version, such as \`v0.1.0-rc.1\`.
2. Open **Actions → Release candidate gate → Run workflow** and select that tag as \`candidate_ref\`, or push the tag to trigger the gate automatically.
3. Review the preflight result and every subsequent job.
4. Publish a GitHub Release only after the exact candidate tag passes all jobs.

The workflow checks out the tag itself, not the current default branch. All dependency-sensitive Cargo commands use \`--locked\`.

## What the gate checks

The preflight fails closed when any of these are missing or inconsistent:

- A non-empty, committed \`Cargo.lock\` containing the application package and version.
- An owner-selected project license in \`Cargo.toml\` and the corresponding license text file.
- Security disclosure policy, release readiness plan, pinned toolchain and the existing engineering/data-portability/SBOM workflows.
- Desktop launcher, AppStream metadata, icon and their validation scripts.
- An AppStream release version matching the Cargo package version.
- A candidate tag matching the package version.

If preflight passes, the workflow runs formatting, locked metadata resolution, all-target compile, Rust tests, strict Clippy, Python schema/SBOM tests, desktop/AppStream validation, and CycloneDX SBOM generation. It uploads the SBOM and SHA-256 checksum as release-candidate evidence.

## Current known blockers

The foundation branch reviewed on 17 November 2026 has no committed \`Cargo.lock\`, no project license text, and \`license = "UNLICENSED"\` in \`Cargo.toml\`. The packaging, SBOM and security files are still in separate draft PRs. The release gate is therefore expected to report blockers until those items are reviewed and integrated.

This PR does not invent or select a project license. The owner must choose one after considering redistribution, modification and contribution policy. The gate only verifies that an explicit choice and license text have been committed.

## Safety and permissions

- Read-only repository permission only.
- No repository writes, tagging, release publication, secret access, telemetry or user-data collection.
- The pipeline runs only on an explicitly selected candidate tag or a pushed \`v*\` tag.
- Build, tests and SBOM generation run on the trusted candidate tag, not arbitrary pull-request code.
- The SBOM is uploaded with a checksum; it is not described as a signed provenance attestation.
- A green hosted pipeline does not replace the real Linux Mint installation, accessibility, upgrade and uninstall acceptance described in \`docs/RELEASE_READINESS.md\`.

## Local preflight

\`\`\`bash
python3 scripts/check-release-readiness.py --ref v0.1.0-rc.1
\`\`\`

Run it from a trusted release-candidate checkout. It reads the repository and does not write or modify files.

Official Cargo reference: [Cargo \`--locked\`](https://doc.rust-lang.org/cargo/commands/cargo-check.html). Cargo documents that \`--locked\` fails when the lockfile is absent or dependency resolution would change; this is why the release pipeline requires a committed lockfile.
