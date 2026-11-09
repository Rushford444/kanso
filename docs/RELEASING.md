# Preparing a release

Work from an explicitly reviewed, committed candidate. The script builds a sanitized snapshot and advances the named local branch with a compare-and-swap update. An existing branch becomes the single parent of the new snapshot; if absent, the snapshot has no parent. It never checks out or pushes a branch.

```sh
scripts/prepare-release.sh COMMIT release
```

The candidate's `Cargo.toml` supplies the version for `Release vVERSION`. Author and committer are fixed to Kodawari <237834781+Rushford444@users.noreply.github.com>. The script excludes designated private artifacts and checks the entire resulting tree before updating the ref. A concurrent ref change causes an error rather than overwriting it. This does not sanitize the original development history; do not publish the original branches.

The local development `main` and sanitized `release` histories are distinct. After review, publish the sanitized `release` as GitHub's `main`, not the local development `main`. First fetch the remote and verify that the existing remote `main` is an ancestor of `release`. If it is not, stop and investigate; do not force-push. If `main` does not exist, verify that this is the intended repository before creating it. After explicit authorization:

```sh
git fetch origin main
git merge-base --is-ancestor FETCH_HEAD release
git push origin release:main
git tag -a v0.1.0 release -m "Release v0.1.0"
git push origin refs/tags/v0.1.0
```

Stop on any failed command. The push to main must succeed before pushing the tag; the workflow also checks that the tagged commit is reachable from remote main. These commands publish and must be run only with explicit authorization. The tag triggers the release workflow, which validates the manifest version and changelog, then builds and publishes x86_64 archives for Linux, macOS, and Windows, plus an aarch64 macOS archive, all with SHA-256 checksums and a combined `SHA256SUMS` file. Each archive also contains the documentation pages linked from the READMEs, so those links resolve inside the archive. GitHub-hosted builds and publication are not proven by a local dry run.

Before moving `release`, test a separate candidate ref initialized to the current release snapshot, inspect its new commit and parent, and independently review the resulting tree and package with `scripts/check_package.py --tree` in a separate checkout. Review binary files and secrets separately: pattern checks are not a substitute for human review. After approval, rerun against the reviewed commit and `release`; verify the parent and tree again. Only publish with explicit authorization.
