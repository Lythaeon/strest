# Release Process

## Versioning

- This project follows SemVer.
- Update `Cargo.toml` version and `CHANGELOG.md` before tagging a release.

## Release Checklist

1. Update `Cargo.toml` version.
2. Update `CHANGELOG.md` with the release changes (ensure a `## X.Y.Z` heading exists for the tag).
3. Run tests, lint, and format check:
   ```bash
   cargo make test
   cargo make clippy
   cargo make format-check
   ```
4. Build release artifacts from a clean tree:
   ```bash
   cargo build --release --locked
   ```
5. Tag the release and push the tag to trigger the workflow:
   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```
6. Verify the GitHub Release artifacts and crates.io publish status.

## Artifact Notes

- Use `--locked` to ensure dependency versions match `Cargo.lock`.
- Build artifacts from a clean working tree for reproducibility.

## Workflows

- Pull requests run CI checks via `cargo make test`, `cargo make clippy`, and `cargo make format-check`.
- The release workflow runs on pushes to `main` and tag pushes (`v*`). Publish and binary builds only run on tags.
- The publish job only runs when the repo is `Lythaeon/strest`. If you fork, update the `if:` condition in `.github/workflows/release.yml`.
- The changelog section used for release notes is extracted by exact `## X.Y.Z` headings (no date suffix).
- Ensure `CARGO_REGISTRY_TOKEN` is set in repo secrets.
