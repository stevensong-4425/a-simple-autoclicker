# Publishing A Simple Autoclicker

GitHub Actions builds the Windows application and the Linux Debian package on
GitHub-hosted runners. Every build produces three downloads:

- `A-Simple-Autoclicker-Windows-x64.zip` — portable application
- `A-Simple-Autoclicker-Setup-VERSION-x64.exe` — normal Windows installer
- `a-simple-autoclicker_VERSION_amd64.deb` — installable package for 64-bit
  Debian, Ubuntu, Linux Mint, and compatible distributions

## Test a Windows build without making a release

1. Push the repository to GitHub.
2. Open the repository's **Actions** tab.
3. Select **Build release**.
4. Select **Run workflow**, then **Run workflow** again.
5. When the run finishes, open it and download the artifact named
   **release-windows-x64** and **release-linux-amd64**.
6. Extract the Windows artifact and test both the portable ZIP and Setup
   executable on a Windows 10 or Windows 11 computer. Install the Debian package
   on a compatible Linux computer with
   `sudo apt install ./a-simple-autoclicker_VERSION_amd64.deb`.

Manual workflow runs do not create a public GitHub release.

## Publish a public release

Before publishing, update the `version` in `Cargo.toml`, test the application,
and commit the changes. Then create and push a matching version tag:

```bash
git add .
git commit -m "Release version 0.1.13"
git pull --rebase origin main
git push origin main
git tag v0.1.13
git push origin v0.1.13
```

Replace `0.1.13` with the version being released. Pushing a tag beginning with
`v` starts the release workflow. The tag must match the version in `Cargo.toml`.
When both platform builds succeed, the workflow creates the GitHub release,
generates release notes, and attaches the two Windows downloads and Linux `.deb`
package.

If the release already exists, the workflow replaces all three files with the
newly built copies.

## Windows download warnings and code signing

New unsigned Windows applications commonly show a Microsoft Defender SmartScreen
warning. The files remain usable, but public users may be hesitant to continue.
For a widely distributed release, obtain a trusted Windows code-signing
certificate, store it as an encrypted GitHub Actions secret, and sign both the
application and installer before the release step. Never commit a certificate or
its password to the repository.

Code signing is intentionally not enabled in the default workflow because it
requires a certificate owned by the publisher.
