# Releasing Sentinel

Releases are built by `.github/workflows/release.yml` when a tag named `v<version>` is pushed.
The website (`.github/workflows/deploy-site.yml`) deploys separately and never waits for a release.

## Cutting a release

1. **Bump the version in all three places.** The workflow refuses to build if they disagree
   with the tag.

   | File | Field |
   |---|---|
   | `src-tauri/tauri.conf.json` | `"version"` |
   | `Cargo.toml` | `[workspace.package] version` |
   | `package.json` | `"version"` |

   Run `cargo check` afterwards so `Cargo.lock` picks up the new version, and commit everything:
   `git commit -am "chore: release 0.2.0"`.

2. **Make sure CI is green on that commit** (`ci.yml`: Rust on all three OSes, bindings drift,
   no `not_wired` stubs, frontend, website).

3. **Tag and push.**

   ```sh
   git tag v0.2.0
   git push origin main v0.2.0
   ```

4. **Watch the run** under Actions > Release. It takes roughly 20–40 minutes; the first notarized
   macOS build can take longer while Apple processes a new Developer ID.

5. **Check the release.** By default the workflow publishes it as soon as every asset is uploaded.
   Set the repository variable `RELEASE_DRAFT=true` (Settings > Secrets and variables > Actions >
   Variables) to keep it as a draft and publish manually after testing the installers.

### What the workflow does

| Job | What happens |
|---|---|
| Prepare release | Checks tag vs. versions, checks the Apple secrets are all-or-nothing, creates a draft release (reused if you re-run a failed release). Versions with a pre-release suffix such as `0.2.0-1` are marked as pre-releases. |
| Build (4 jobs) | `tauri-apps/tauri-action@v1` builds and uploads to the draft: macOS arm64 and x64 (both on the Apple Silicon runner), Windows x64, Linux x64 (Ubuntu 22.04 baseline). |
| Publish release | Fails if any expected asset is missing, uploads `SHA256SUMS.txt`, publishes the release. |

### Release assets

| Asset | Platform |
|---|---|
| `Sentinel-<version>-macos-arm64.dmg` | macOS 11+, Apple Silicon |
| `Sentinel-<version>-macos-x64.dmg` | macOS 11+, Intel |
| `Sentinel-<version>-windows-x64-setup.exe` | Windows 10/11 x64, NSIS installer |
| `Sentinel-<version>-windows-x64.msi` | Windows 10/11 x64, MSI package |
| `Sentinel-<version>-linux-x64.AppImage` | Linux x64, portable |
| `Sentinel-<version>-linux-x64.deb` | Debian, Ubuntu and derivatives |
| `Sentinel-<version>-macos-{arm64,x64}.app.tar.gz` | Compressed `.app` bundles (uploaded by tauri-action) |
| `SHA256SUMS.txt` | Checksums of every asset above |

The website's download section looks up the latest release through the GitHub API and matches
these exact names, so do not rename them by hand. If you change the naming scheme, update
`releaseAssetNamePattern` in `release.yml`, the expected list in its publish job, and
`website/src/lib/releases.ts` together.

### Version constraints

- The Windows MSI (WiX) only accepts `major.minor.patch` with an optional **numeric** pre-release
  (`0.2.0-1`). A tag like `v0.2.0-beta.1` builds on macOS and Linux but fails the Windows job.
- A tag that doesn't match the three version fields fails in "Prepare release" before anything is
  built.

### Fixing a failed release

- **A build job failed:** fix the cause on `main`, then either re-run the failed jobs (same commit)
  or delete the tag and re-tag the fixed commit:

  ```sh
  git tag -d v0.2.0 && git push origin :refs/tags/v0.2.0
  git tag v0.2.0 && git push origin v0.2.0
  ```

  The draft release is reused and existing assets with the same name are replaced.
- **A bad release was published:** mark it as a draft again or delete it on the Releases page,
  delete the tag, and ship a new patch version. Don't re-use a version number users may already
  have installed.

## Code signing secrets

Add secrets under **Settings > Secrets and variables > Actions > Secrets**, variables under the
**Variables** tab. Nobody but the account holder can obtain these; the workflow carries
`TODO(secrets)` markers where they are read.

### macOS: Developer ID signing and notarization

Requires a paid [Apple Developer Program](https://developer.apple.com/programs/) membership
(individual or organization).

| Secret | Required | Where to get it |
|---|---|---|
| `APPLE_CERTIFICATE` | yes | In [Certificates, IDs & Profiles](https://developer.apple.com/account/resources/certificates/list) create a **Developer ID Application** certificate (needs a CSR from Keychain Access > Certificate Assistant > Request a Certificate From a Certificate Authority). Install it, then in Keychain Access > My Certificates export the certificate **with its private key** as `.p12`. Encode: `openssl base64 -A -in certificate.p12 -out certificate-base64.txt` and paste the contents. |
| `APPLE_CERTIFICATE_PASSWORD` | yes | The password you set when exporting the `.p12`. |
| `APPLE_ID` | yes | The email address of the Apple Account that belongs to the developer team. |
| `APPLE_PASSWORD` | yes | An **app-specific password**, not your account password: [account.apple.com](https://account.apple.com) > Sign-In and Security > App-Specific Passwords. |
| `APPLE_TEAM_ID` | yes | 10-character Team ID shown at [developer.apple.com/account](https://developer.apple.com/account) > Membership details. |
| `APPLE_SIGNING_IDENTITY` | no | The certificate's full name, e.g. `Developer ID Application: Rayan Jain (AB12CD34EF)`. Only needed if the `.p12` contains more than one identity; otherwise Tauri reads it from the certificate. |

**If absent:** the macOS builds are ad-hoc signed and not notarized. They still install, but
Gatekeeper blocks the first launch until the user clicks **Open Anyway** in System Settings >
Privacy & Security (the website's install guide documents this). The run shows a warning.

**If only some are set:** the release fails in "Prepare release" and lists what is missing, so a
typo never silently ships an unsigned build.

### Windows: Authenticode signing

Choose one option. Without either, the release still succeeds.

**Option A — code signing certificate (`.pfx`)** from a CA such as DigiCert, Sectigo or SSL.com.
Since June 2023 CAs issue new certificates only on hardware tokens or cloud HSMs, which cannot be
exported to a `.pfx`; use this option only if you have an exportable certificate (for example one
issued before that date) and prefer Option B otherwise.

| Secret | Where to get it |
|---|---|
| `WINDOWS_CERTIFICATE` | Base64 of the `.pfx`: `certutil -encode certificate.pfx cert.txt` (strip the BEGIN/END lines) or `openssl base64 -A -in certificate.pfx`. |
| `WINDOWS_CERTIFICATE_PASSWORD` | The `.pfx` export password. |

The workflow imports the certificate, reads its thumbprint and signs with SHA-256 and the DigiCert
timestamp server.

**Option B — Azure Artifact Signing** (formerly Trusted Signing), a pay-monthly Microsoft service
that needs identity validation for individuals or organizations.

1. Create an Artifact Signing account and a certificate profile in the Azure portal.
2. Register an app in Microsoft Entra ID, create a client secret, and give the app the
   *Artifact Signing Certificate Profile Signer* role on the account.

| Name | Kind | Value |
|---|---|---|
| `AZURE_CLIENT_ID` | secret | App registration's Application (client) ID |
| `AZURE_CLIENT_SECRET` | secret | Client secret value |
| `AZURE_TENANT_ID` | secret | Directory (tenant) ID |
| `AZURE_SIGNING_ENDPOINT` | variable | Account endpoint, e.g. `https://eus.codesigning.azure.net` |
| `AZURE_SIGNING_ACCOUNT` | variable | Artifact Signing account name |
| `AZURE_SIGNING_PROFILE` | variable | Certificate profile name |

The workflow installs `artifact-signing-cli` and passes it to Tauri as `bundle.windows.signCommand`.

**If absent:** installers are published unsigned and the run shows an "Unsigned Windows build"
warning. Users see SmartScreen's "Windows protected your PC" and must choose **More info > Run
anyway**; reputation for unsigned files does not build up over time.

### Linux

Not signed. `SHA256SUMS.txt` on each release lets users verify downloads.

### Other permissions

`GITHUB_TOKEN` is provided automatically. The workflows request only what they need
(`contents: write` for releases, `pages: write` + `id-token: write` for the site), so the
repository's default workflow permissions can stay read-only.

## Website deploys

`deploy-site.yml` builds `website/` and publishes it to GitHub Pages at
<https://rayanjainn.github.io/sentinel/> on every push to `main` that touches `website/**` (or the
shared `src/styles/tokens.css`), and on manual dispatch.

One-time setup after creating the repository: **Settings > Pages > Build and deployment > Source:
GitHub Actions**. If you later move the site to a custom domain, update `site` and `base` in
`website/astro.config.mjs`.
