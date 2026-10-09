# GSET - Generic Syntax Extension Tool

<div align="center">

**Write in any language syntax, compile to any language.**

[![License: Apache-2.0 OR MIT](https://img.shields.io/badge/License-Apache--2.0%20OR%20MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.90+-orange?style=flat&logo=rust)](https://rust-lang.org/)
[![Release](https://img.shields.io/github/v/release/Crazygiscool/GSETLang)](https://github.com/Crazygiscool/GSETLang/releases)
[![Tests](https://img.shields.io/github/actions/workflow/status/Crazygiscool/GSETLang/test.yml)](https://github.com/Crazygiscool/GSETLang/actions)

</div>

## Install

- **Linux / macOS**: `curl -fsSL https://raw.githubusercontent.com/Crazygiscool/GSETLang/main/install.sh | bash`
- **Windows**: `powershell -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/Crazygiscool/GSETLang/main/install.ps1 | iex"`
- **Homebrew**: `brew install crazygiscool/gset/gset`
- **AUR**: `gset` (stable); `gset-git` is deprecated in favor of it
- **Chocolatey**: `choco install gset`
- **winget**: `winget install GSETLang.GSET`

## Publishing

GSET ships to four channels: GitHub Release, the Homebrew tap, AUR, and
Chocolatey (winget via a version PR to `microsoft/winget-pkgs`).

1. **Cut the tag** — releases build from `v*` tags:

   ```bash
   git tag -a vX.Y.Z -m "vX.Y.Z" && git push origin vX.Y.Z
   ```

   `.github/workflows/release.yml` builds the asset matrix and creates the GitHub
   Release.
2. **Publish packages** — the publish workflow does not auto-trigger for
   API-created releases, so dispatch it after the release is up:

   ```bash
   gh workflow run publish-packages.yml -R Crazygiscool/GSETLang -f tag=vX.Y.Z
   ```

   `.github/workflows/publish-packages.yml` then updates the
   [Homebrew tap](https://github.com/Crazygiscool/homebrew-gset), pushes the AUR
   `gset` package (and deprecates `gset-git`), pushes the Chocolatey package, and
   opens a winget version PR.

   Every package step is idempotent, so re-running after a partial failure is
   safe; the winget step skips when a matching PR already exists.
