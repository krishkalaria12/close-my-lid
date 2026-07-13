# Homebrew Distribution Design

## Objective

Make Close My Lid installable through a conventional public Homebrew tap with a single command, keep the macOS cask and CLI formula synchronized with GitHub releases, and prevent invalid package updates from reaching users.

## Repository Architecture

Create the public repository `krishkalaria12/homebrew-close-my-lid`. It becomes the canonical source for:

- `Casks/close-my-lid.rb`, which installs the released macOS application archive.
- `Formula/close-my-lid.rb`, which builds and installs the CLI from the matching source tag.
- Tap-specific CI and contribution documentation.

The application repository remains canonical for source code, release archives, tags, appcast metadata, and release automation. Its existing Homebrew files remain temporarily as a migration fallback but stop being the source updated for new releases.

## User Experience

New users install with Homebrew's conventional automatic tap resolution:

```sh
brew install --cask krishkalaria12/close-my-lid/close-my-lid
```

CLI-only users run:

```sh
brew install krishkalaria12/close-my-lid/close-my-lid
```

Existing users whose same-named tap points to the application repository switch its remote in place:

```sh
brew tap --custom-remote krishkalaria12/close-my-lid \
  https://github.com/krishkalaria12/homebrew-close-my-lid
```

The README, release guide, current release notes, and release-note validation are updated for these commands. Migration instructions remain visible through at least the next release.

## Release Automation

When an application release is published, a workflow in `close-my-lid` performs these steps:

1. Validate the semantic version and required source tag.
2. Locate the expected macOS ZIP asset.
3. Download the source archive and macOS ZIP, then compute SHA-256 checksums.
4. Clone the tap repository using a restricted token.
5. Update the formula and cask version, URLs, and checksums on a release-specific branch.
6. Push the branch, open or update one pull request, and enable auto-merge.

The updater must be idempotent: rerunning it for the same release updates the existing branch and pull request instead of creating duplicates. Missing assets, mismatched versions, download failures, or checksum failures stop the workflow without changing the tap's default branch.

## Tap Validation and Publishing

Pull requests in the tap repository run:

- Ruby syntax checks.
- `brew style` for the formula and cask.
- Strict Homebrew audits.
- Formula installation and `brew test` on supported macOS.
- Cask metadata and artifact checksum validation.
- A clean automatic-tap resolution smoke test.

The tap repository protects `main`, requires the validation workflow, uses squash merges, and permits auto-merge. A release update reaches `main` only after required checks pass. The initial package definitions receive the same checks before automation is enabled.

Artifact architecture is verified during implementation. If the released application is arm64-only, the cask declares that restriction rather than advertising unsupported Intel installation. The cask declares Sparkle-driven automatic updates only if that matches Homebrew's current cask policy.

## Authentication and Security

Cross-repository writes use a fine-grained GitHub personal access token restricted to `krishkalaria12/homebrew-close-my-lid`, with only Contents and Pull Requests write permissions. It is stored as an Actions secret in the application repository. The existing broad GitHub CLI login token is never copied into repository secrets.

The token is used only to push updater branches, manage the corresponding pull request, and enable auto-merge. Tap CI runs with the repository's normal read-only token except for GitHub's merge operation.

Creating the restricted token is the only required manual security step. All other repository creation, files, settings, workflows, documentation, validation, and migration changes are automated or performed through authenticated GitHub tooling.

## Rollout

1. Create and populate the dedicated tap repository.
2. Validate both packages against v0.3.0.
3. Configure tap repository settings and required checks.
4. Update application documentation and the published v0.3.0 release notes.
5. Add the release updater, initially runnable through `workflow_dispatch` as well as release publication.
6. Add the restricted token and run the updater against v0.3.0 to prove idempotency without creating a version change.
7. Verify fresh cask installation, fresh CLI installation, installed-package upgrade metadata, and migration from the old custom remote.
8. Keep the old package files for one release cycle, then remove them in a separate cleanup change.

## Failure Handling

- The updater fails closed when a tag, asset, or checksum cannot be verified.
- A failed tap check leaves the update pull request open and does not modify `main`.
- Repeated release events reuse the same branch and pull request.
- The migration command is verified against an existing custom-remote tap before publication.
- Manual workflow dispatch remains available to recover from transient GitHub or download failures.

## Non-Goals

- Publishing to Homebrew Core or the official cask repository.
- Solving Developer ID signing or notarization; those remain application release concerns.
- Removing the old in-repository Homebrew files during initial rollout.
- Automatically storing or broadening credentials without explicit user action.
