## Creating a release

1. Run `./scripts/release.sh` to prepare the release with [rooster].
1. Review the changes and editorialize the changelog.
1. Create a pull request with the changelog and version updates.
1. Merge the PR.
1. Run the Release workflow with the new version number (without the starting `v`).
1. Verify the GitHub release to confirm the changelog matches `CHANGELOG.md`.

[rooster]: https://github.com/zanieb/rooster
