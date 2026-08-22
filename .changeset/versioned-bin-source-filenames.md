---
"git-stats": patch
---

**fix**: download AUR `git-stats-bin` release tarballs to version-suffixed filenames.

The release assets are named without a version, so makepkg's source cache collided across releases: a tarball cached from an older install shadowed the new download and failed checksum validation, breaking `paru -S git-stats-bin` on upgrade.
