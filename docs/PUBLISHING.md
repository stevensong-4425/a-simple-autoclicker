# Publishing A Simple Autoclicker

GitHub Actions builds the Windows application on Microsoft's hosted Windows
runner. Every build produces two downloads:

- `A-Simple-Autoclicker-Windows-x64.zip` — portable application
- `A-Simple-Autoclicker-Setup-VERSION-x64.exe` — normal Windows installer

## Test a Windows build without making a release

1. Push the repository to GitHub.
2. Open the repository's **Actions** tab.
3. Select **Build Windows release**.
4. Select **Run workflow**, then **Run workflow** again.
5. When the run finishes, open it and download the artifact named
   **A-Simple-Autoclicker-Windows-x64**.
6. Extract the artifact and test both the portable ZIP and Setup executable on a
   Windows 10 or Windows 11 computer.

Manual workflow runs do not create a public GitHub release.

## Publish a public release

Before publishing, update the `version` in `Cargo.toml`, test the application,
and commit the changes. Then create and push a matching version tag:

```bash
git add .
git commit -m "Release version 0.1.2"
git tag v0.1.2
git push origin main
git push origin v0.1.2
```

Replace `0.1.2` with the version being released. Pushing a tag beginning with
`v` starts the Windows workflow. When it succeeds, the workflow creates the
GitHub release, generates release notes, and attaches both Windows downloads.

If the release already exists, the workflow replaces its Windows files with the
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
