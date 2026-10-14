# Windows code signing

Windows builds use a free self-signed RSA-3072 Code Signing certificate and Windows
SDK SignTool. The Authenticode file digest and RFC 3161 timestamp digest both use
SHA-256. The timestamp service is `http://timestamp.digicert.com`; Windows verifies
the signed timestamp response.

The non-exportable private key stays in `Cert:\CurrentUser\My`. The public identity
is recorded in `%LOCALAPPDATA%\Lumen\CodeSigning\identity.json`, alongside
`Lumen-SelfSigned.cer`. Preserve the Windows user profile and its key storage to
retain this identity. A fresh machine/user creates a new identity; an existing
identity with a missing/expired key fails rather than silently rotating it.

The public certificate is trusted in **CurrentUser** Root and TrustedPublisher
only. LocalMachine is unchanged. Other computers do not automatically trust it.
Self-signing does not remove SmartScreen warnings or guarantee that Defender will
accept the application. Security protections remain enabled.

## Automatic builds

```powershell
npm run build:release
npm run build:installer
npm run build:portable
npm run tauri:build
```

The existing batch entry points also sign their outputs. Tauri automatically
loads `src-tauri/tauri.windows.conf.json` on Windows:

1. Before compilation, `prepare-windows-build.ps1` signs Xray, updates its generated
   manifest hash, and builds the existing frontend.
2. Rust embeds that manifest so runtime integrity checks still match.
3. Tauri signs the main EXE, NSIS uninstaller and installer before updater/minisign
   signatures are generated. Updater keys are independent of Authenticode.
4. `build-release.ps1` explicitly signs the main EXE for portable `--no-bundle`
   builds before packaging, and copies the public CER into `release`.

Use the release scripts for portable builds: bare `tauri build --no-bundle` skips
Tauri's signing phase. `cargo build` is not the release entry point.
Do not sign Xray after compiling the main EXE: its new hash would disagree with
the embedded manifest. `prepare-core.ps1` can restore the pinned upstream runtime;
the next Windows build signs it and embeds the new hash again.

Signing or verification failures stop the build, including an unavailable SDK,
key, timestamp service or invalid runtime manifest. Release scripts discover the
newest installed SignTool and set `TAURI_WINDOWS_SIGNTOOL_PATH` for Tauri's internal
NSIS resource verification. `-Mode Initialize` also saves this SDK path in the user
environment for direct Tauri commands in newly opened terminals. On CI, a pinned
release certificate is imported from repository secrets. The build refuses to
generate a new identity when CI signing is not configured.

## GitHub Actions

The local development key is non-exportable. GitHub-hosted Windows runners use a
separate, persistent RSA-3072 release certificate, shared across all CI releases.
Its public certificate is embedded in the EXE signatures; users do not need a
separate CER file to run the application.

The repository requires these Actions settings:

| Kind     | Name                             | Purpose                        |
| -------- | -------------------------------- | ------------------------------ |
| Secret   | `WINDOWS_CERTIFICATE_PFX_BASE64` | Password-protected release PFX |
| Secret   | `WINDOWS_CERTIFICATE_PASSWORD`   | PFX password                   |
| Variable | `WINDOWS_CERTIFICATE_THUMBPRINT` | Expected certificate identity  |

Provision or restore these settings without printing secret values:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/setup-github-signing.ps1 -Repository Sincopa/lumen-ray
```

The script reuses the encrypted PFX and DPAPI-protected password stored outside
the checkout in `%LOCALAPPDATA%\Lumen\CodeSigningCI\Sincopa-lumen-ray`. Preserve
that directory and the Windows profile. It does not replace the local development
certificate or the existing Tauri updater/minisign secrets.

`ci-windows-signing.ps1` checks the PFX thumbprint before import, imports the runner
key without export permission, and supplies SignTool to Tauri. Xray is signed
before Rust checks/builds so the embedded integrity hash matches. The final
installer, uninstaller, main EXE and all portable EXEs must pass verification
before the draft release is published. Signature reports are uploaded separately
as Actions artifacts. The runner key and temporary PFX are cleaned up even after
failure. PFX/password files are never uploaded as build artifacts.

The existing `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
secrets continue to protect automatic updates independently of Authenticode.

## Verification and backups

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/sign-windows.ps1 -Mode Verify -ReportPath artifacts/code-signing/signatures.json
```

Automatic discovery covers the current source runtime, release EXE/runtime,
NSIS bundles and current unpacked files under `release`. Build/dependency tools,
debug outputs, snapshots under `artifacts`, `*-old`/`*-backup` directories and
`*.before-fix.exe` are excluded. Verify ZIP contents after extraction by passing
their EXE paths explicitly with `-Path`.

Verification requires `signtool verify /pa /all /v`, Windows Authenticode status
`Valid`, the expected signer, a timestamp, and the SHA-256 digest OID in the embedded
CMS signature. Repeated signing leaves valid timestamped files intact. Originals
changed by the signer are saved by SHA-256 under `artifacts/code-signing/backups`.
The initial release is also preserved under `artifacts/code-signing/original-release`.

The Tauri hook preserves a verified copy of the temporary PE uninstaller as
`artifacts/code-signing/nsis-uninstaller.exe`. Its previous proof is cleared when
bundling starts. The final installer refuses signing without a newly verified
uninstaller, because NSIS itself may ignore `!uninstfinalize` command failures.

References: [Microsoft SignTool](https://learn.microsoft.com/en-us/dotnet/framework/tools/signtool-exe),
[New-SelfSignedCertificate](https://learn.microsoft.com/en-us/powershell/module/pki/new-selfsignedcertificate),
[Tauri Windows signing](https://v2.tauri.app/distribute/sign/windows/).
