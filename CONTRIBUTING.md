# Contributing to CreativeCraft

Thank you for wanting to help. This project is one person's work so far, so
anything that saves time is welcome.

## Setting up

You need [Rust](https://rustup.rs) (stable, MSVC toolchain) and the Microsoft C++
build tools, on Windows 10 or 11, 64-bit.

```powershell
git clone https://github.com/its-cybru/creativecraft
cd creativecraft\creativecraft\studio-shell
cargo build
cargo run
```

The first build takes a long time and roughly 25 GB of disk in
`creativecraft\studio-shell\target`. That folder is build scratch space, is
ignored by git, and can be deleted at any time.

## Before you open a pull request

```powershell
cd creativecraft\studio-shell
cargo test
```

All 63 tests pass today; please keep it that way, and add a test for anything you
fix.

## What the code looks like

Read [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) first. The short version:
`studio-shell` is the window; every tab hosts a real application through a small
adapter in `src/*_host.rs`; the tabs share state through a handful of explicit
hand-offs; and the shell links the hosted apps' own crates directly, which is the
part most likely to break when those apps change.

## Rules that are easy to break by accident

- **Never commit** `target/`, `dist/`, `.exe`, `.pdb` or `creativecraft.ccproj`.
  `.gitignore` covers them, but check `git status` before committing.
- **Do not restore the `docs/brand/` folders** inside the bundled apps. They hold
  the ArtCraft trademarks, their licence forbids shipping them in a modified
  product, and this repository is a modified product. Leave them excluded.
- **Keep the bundled apps' own licence files** (`LICENSE-MIT`, `LICENSE-APACHE`,
  `NOTICE`, `ATTRIBUTION`) exactly where they are.
- **Do not claim endorsement** from the ArtCraft team, DaVinci, Blackmagic or
  Affinity. Comparing models in plain text is fine; implying a relationship is
  not.

## Reporting problems

Open a GitHub issue with:

- what you clicked and what happened,
- the CreativeCraft version (Help → About, or the release tag),
- whether it happened in the portable build, a source build, or both.

Screenshots help. Crash logs are in `%LOCALAPPDATA%\CreativeCraft` if any were
written.

## Good first issues

- Documentation and wording fixes. The README and `docs/` are read more than the
  code is.
- Reproducing reported limits on your hardware, especially which OpenFX plug-ins
  render and which refuse.
- Small, contained fixes to the hand-offs. Large refactors of the hosted apps'
  code are hard to review and tend to conflict with the upstream projects.
