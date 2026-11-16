# Release SBOM — CycloneDX

**Scope:** Rust application dependency inventory  
**Format:** CycloneDX JSON 1.7  
**Generator:** `cargo-cyclonedx` 0.5.10  
**Artifact:** `sbom.json` plus `sbom.json.sha256`

## What this adds

The `.github/workflows/rust-sbom.yml` workflow generates a Software Bill of Materials for the Rust application and stores the result as a downloadable GitHub Actions artifact.

The inventory records the application component and its resolved Cargo dependency components, including version, package identifiers, dependency relationships, and available package license/source metadata. The workflow checks that the result identifies this application and includes its direct `gtk4` and `rusqlite` dependencies.

The generated artifact is a dependency inventory. It is not a vulnerability scan, a signed build attestation, a complete inventory of host GTK/GLib system libraries, or evidence that all third-party licenses have been legally approved. PR #19's RustSec/CodeQL/Dependabot work remains separate.

## How the workflow runs

- Pushes to `work/release-sbom` and the integration foundation branch generate an SBOM.
- A maintainer can manually dispatch the workflow against a selected branch.
- The weekly schedule runs from the default branch once this workflow is merged there.
- The workflow uses read-only repository permissions and SHA-pinned GitHub Actions.
- It generates a temporary Cargo lockfile, installs the pinned CycloneDX generator, validates the JSON and uploads the SBOM/checksum with a 90-day artifact retention period.

## Why it is not triggered by pull requests

The CycloneDX Cargo plugin calls Cargo internally to inspect a project. Cargo may run build scripts, so the plugin's maintainers advise against running it against untrusted projects.

This workflow therefore avoids automatic execution on arbitrary pull-request code. The feature branch is scanned by its push event; a maintainer may also manually dispatch it after reviewing a trusted branch. It does not receive repository write permissions or deployment secrets.

## Current reproducibility limitation

The foundation branch does not contain a committed `Cargo.lock`. The workflow creates a temporary lockfile with `cargo generate-lockfile` and uses it for the SBOM generation run; it is not committed or uploaded.

Consequently, the SBOM describes the dependency graph resolved for that run. It must not be presented as the immutable dependency set of a release binary, and separate runs can resolve different transitive versions.

Before a release candidate:
1. Generate and review `Cargo.lock` using the pinned project toolchain.
2. Commit the reviewed lockfile for this binary application.
3. Change the SBOM workflow to require locked resolution and fail if the lockfile would change.
4. Generate the release SBOM from the exact commit and lockfile used to build the artifact.
5. Retain the SBOM alongside the package, checksums and release provenance.

The repository's project license is still unselected (`UNLICENSED` in Cargo metadata). The SBOM records metadata available from package manifests; it does not choose a project license or replace a legal review.

## Local validation

On a trusted checkout, with the pinned Rust toolchain installed:

```bash
cargo generate-lockfile
cargo install cargo-cyclonedx --version 0.5.10 --locked
cargo cyclonedx --format json --spec-version 1.7 --override-filename sbom -qq
python3 scripts/validate_sbom.py sbom.json
sha256sum sbom.json > sbom.json.sha256
python3 -m unittest discover -s tests -p 'test_*.py' -v
```

The SBOM validator is standard-library-only and checks the CycloneDX format/version, application identity/version, direct dependency presence and dependency relationships.

## References

- CycloneDX Rust Cargo generator: https://github.com/CycloneDX/cyclonedx-rust-cargo
- CycloneDX specification: https://cyclonedx.org/specification/overview/
- GitHub Actions artifact upload: https://github.com/actions/upload-artifact
- Cargo lockfile guidance: https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html
