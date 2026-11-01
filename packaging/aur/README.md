# AUR publication

`aur.yml` prepares `.SRCINFO` with Arch `makepkg` in a separate job without
publication credentials. The publish job starts on a fresh runner, downloads only
PKGBUILD and `.SRCINFO` as data and never executes them. Updater scripts always
come from the repository default branch. The git recipe is read from the exact
successful CI commit, so packaging edits in `dev` are included. Successful Linux CI push runs on `dev` publish `osu-radio-git`;
published GitHub Releases (including pre-releases) publish `osu-radio-bin` from
the exact versioned Arch release asset. The workflow does not build packages.

Configure repository secrets `AUR_SSH_PRIVATE_KEY` and `AUR_KNOWN_HOSTS`.
The latter must contain independently verified AUR SSH host keys; the workflow
does not trust a live `ssh-keyscan`. Register the public key with the AUR account
maintaining both packages. A dedicated key still has the account's package rights.

Git package metadata uses epoch 1 and the publication workflow run number so
force-pushed source histories cannot reduce the package version. `-git` still
builds the current `dev` checkout when users rebuild it. Re-running an old CI
result skips publication if its source is no longer the head of `dev`.
GitHub concurrency serializes publication separately for git and bin, so a dev
update cannot cancel a pending release publication. Pending runs can be superseded by
newer runs. There is no guarantee of a separate AUR commit for every source push.

Tests: `python3 -B -m unittest discover -s packaging/aur -v`.
Live AUR pushes require the secrets and are not part of these local tests.

Before pushing, dev freshness is checked again and package versions are compared
with the live AUR `.SRCINFO` through `vercmp`. Unknown version metadata fails;
equal or older versions are skipped. Concurrent AUR edits reject the regular
(non-force) push. The final dev check and AUR push cannot be atomic across hosts.

Release tags use SemVer. Arch `pkgver` maps pre-releases to a `pre` suffix,
replaces identifier hyphens with underscores and omits build metadata:
`1.2.3-alpha-1+build.001` becomes `1.2.3prealpha_1`. The complete original
version remains in release download URLs. This preserves the existing alpha
upgrade and pre-release-to-stable transitions. Arbitrary SemVer pre-release
identifier ordering is not identical to Arch `vercmp`; metadata-only releases
have equal package versions and are intentionally skipped by the updater.
