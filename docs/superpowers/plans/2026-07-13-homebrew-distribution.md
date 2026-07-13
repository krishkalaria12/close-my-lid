# Homebrew Distribution Implementation Plan

## Goal

Create a conventional public Homebrew tap, migrate installation guidance, automate version/checksum update pull requests, and validate the complete release path without storing a broad GitHub credential.

## Task 1: Bootstrap the dedicated tap

1. Verify v0.3.0 source and app-archive checksums and artifact architecture.
2. Create a temporary local tap checkout containing the formula, cask, README, license, and CI workflow.
3. Remove the redundant formula `version` declaration and encode any verified architecture/update metadata.
4. Run Ruby syntax, Homebrew style/audit, URL/checksum, formula build/test, and cask validation checks.
5. Create the public `krishkalaria12/homebrew-close-my-lid` repository, push the validated initial commit, and configure repository settings.

## Task 2: Add tested release-update tooling

1. Add fixture-driven tests in the application repository for version validation, checksum substitution, idempotency, and malformed/missing inputs.
2. Run the tests and confirm they fail before implementation.
3. Implement a release updater that validates a tag and asset, computes checksums, updates a tap checkout, and reports whether files changed.
4. Run the focused tests and existing repository checks.
5. Add a release/workflow-dispatch GitHub Actions workflow that uses the updater, pushes a deterministic branch, opens or reuses a PR, and enables auto-merge using a restricted secret.

## Task 3: Migrate documentation and release validation

1. Update release-instruction tests to require the conventional one-line cask command and migration guidance.
2. Confirm the existing validator fails the new tests.
3. Update the validator, README, release guide, and product structure notes.
4. Update the published v0.3.0 release body with conventional installation and old-remote migration instructions.
5. Keep the in-repository formula and cask for one release cycle, clearly marked as migration-only.

## Task 4: Configure GitHub safeguards

1. Enable squash merge, delete-branch-on-merge, and auto-merge in the tap repository.
2. Run tap CI on the initial default branch and identify its exact check name.
3. Protect `main` with required CI, no force pushes, and no deletions while allowing the configured automation path.
4. Document and request the single fine-grained secret needed by the application repository: tap-only Contents and Pull Requests write access plus Administration read access for branch-protection verification.

## Task 5: End-to-end verification and publication

1. Verify clean automatic tap resolution without relying on the existing custom remote.
2. Verify cask metadata/download and formula install/test.
3. Verify replacing an existing custom tap clone with the conventional repository while preserving installed packages.
4. Run all application-side updater, instruction, syntax, style, and diff checks.
5. Commit and push application-repository changes.
6. Report the tap URL, commits, workflows, test evidence, and the exact remaining secret step if credentials block live updater execution.
