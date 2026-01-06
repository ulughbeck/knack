# Release Guide (macOS + Linux + Windows)

This repo publishes macOS and Linux binaries via GitHub Releases when you push a `v*` tag.

## Checklist
1) Update version in `Cargo.toml` (and anywhere else you track versions).
2) Run tests:
   ```sh
   cargo test
   ```
3) Commit your changes.

## Create the release
```sh
git tag v0.0.1
git push origin v0.0.1
```

## What happens next
- GitHub Actions runs the `Release` workflow.
- It builds five artifacts (Linux builds are GNU/glibc):
  - `knack-macos-arm64.tar.gz`
  - `knack-macos-x86_64.tar.gz`
  - `knack-linux-arm64.tar.gz`
  - `knack-linux-x86_64.tar.gz`
  - `knack-windows-x86_64.zip`
- The workflow creates/updates the GitHub Release for that tag and uploads the artifacts.

## Verify
- Check the Actions tab for a green run.
- Open the GitHub Release and download an artifact.
- Test install with (auto-selects OS/arch):
  ```sh
  curl -fsSL https://raw.githubusercontent.com/ulughbeck/knack/main/scripts/install.sh | sh
  ```
  ```powershell
  iwr -useb https://raw.githubusercontent.com/ulughbeck/knack/main/scripts/install.ps1 | iex
  ```

## Troubleshooting
- If the release exists but is missing assets, re-run the workflow or delete and re-push the tag.
- If builds fail, fix the issue and force-update the tag:
  ```sh
  git tag -d v0.0.1
  git push --delete origin v0.0.1
  git tag v0.0.1
  git push origin v0.0.1
  ```
