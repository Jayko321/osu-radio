# Manual release builds

Before merging the release, set the same intended version in the Qt and server
`Cargo.toml` files and update `Cargo.lock`. The release tag must be `v` followed
by this exact version: archive filenames and AUR publication depend on it.

Run **Release builds** (`release-staging.yml`) with the full source commit SHA
from `main`. Windows and Arch build independently from that commit using the
existing builders and tool pins in `inputs.json`. Download `windows-handoff`
(Setup and build metadata) and `arch-handoff` (the Arch archive) from Actions.
A successful platform artifact remains available if the other build fails.
Build logs are uploaded separately. This workflow never publishes a release.

After checking the outputs, create a new SemVer tag and GitHub pre-release on
that source commit and upload the Setup and Arch archive. Write release notes
manually. AUR binary packaging consumes the archive from that release.

The build workflow does not require a separate Windows materials URL or assemble
a combined staging directory. It includes the repository's existing license
files; this does not establish a complete Windows dependency license inventory.

The older `stage.py` assembly tooling remains available for historical task
cards but is not invoked by this workflow. Its materials checks are not release
build prerequisites.
