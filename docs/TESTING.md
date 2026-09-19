# Validation report / 验证报告

Initial implementation validation: 2026-09-19. This report distinguishes executed
automation from hardware/browser requirements that need real devices.

## Executed locally

Environment: Fedora Linux 44 x86_64, kernel 7.0.14, glibc 2.43; Rust 1.94.0;
Node.js 24.18.0; Playwright 1.60.0 / Chromium 148.0.7778.96.

- Svelte/TypeScript checks: zero errors and zero warnings.
- Frontend production build: approximately 70 KiB JavaScript, 27.3 KiB gzip;
  CSS approximately 9 KiB. No CDN or runtime network asset dependencies.
- `cargo fmt --all -- --check` and strict workspace Clippy: passed.
- Nine Rust unit tests: URL/scheme and credential-in-URL checks; short Chinese,
  mixed-case and literal wildcard search; folder cycles; HTML hierarchy roundtrip
  with rejected JavaScript URLs; transaction rollback and idempotency; exact
  32-byte WebAuthn User Handle and UV/resident policy; setup mode/binding/one-time
  consumption; cross-thread first-credential race through independent SQLite
  connections; session expiry/scope/revocation/stale credential versions; identity
  and session persistence across reopening.
- Chromium integration scenario: **passed**. Real WebAuthn library validation
  against a CDP virtual authenticator, not a server login stub. It exercises:
  login page control count; anonymous API denial; token-authorized registration;
  explicit login after setup; auto-setup closure; short and 30-day cookies;
  bookmark creation/search/edit conflict/trash/restore; duplicate request keys;
  malicious URLs; folder-cycle rejection; failed batch rollback; HTML import
  warnings and repeat commit; JSON full-field transfer into an empty second host;
  separate business libraries; shared Passkey login; copied session rejection;
  logout; cross-host credential revocation; 32-byte authenticator user handle;
  desktop/mobile screenshots; browser uncaught-error check; offline restoration
  of a populated business/auth backup, invalidation of pre-backup sessions, and
  a fresh Passkey login to the restored three-entry library.
- Both debug and release-optimized binaries passed the browser integration scenario.
- Native executable smoke test: init, config validate, server start, health,
  embedded UI, anonymous session endpoint, online backup including auth, stopped
  server auth restore, and doctor all passed. Dynamic dependencies were libc,
  libm and libgcc_s; no system OpenSSL or SQLite runtime required.

The local Chromium package required NSS/NSPR libraries and Chinese fonts in the
build environment. These are browser-test dependencies, not bookmarkd runtime
requirements. CI installs browser dependencies on its Ubuntu runner.

## Release evidence

The Release workflow builds Linux x86_64/aarch64, macOS arm64/x86_64 and Windows
x86_64 on native runners. Each artifact's `.manifest.json` captures the actual OS,
architecture, program version, startup/backup/restore outcome and link evidence.
The initial [v0.1.0 release workflow](https://github.com/zhangchaosd/bookmarkd/actions/runs/35440914018) **passed all five native jobs and publication**.
The [latest recovery regression CI](https://github.com/zhangchaosd/bookmarkd/actions/runs/35441731909) also passed.
All release attachments were downloaded and every entry in `SHA256SUMS` verified.

| Artifact | Actual native test environment | Result |
|---|---|---|
| Linux x86_64 | Ubuntu runner, kernel 6.17.0, glibc 2.39 | Build, tests, startup and restore passed |
| Linux aarch64 | Ubuntu ARM runner, kernel 6.17.0, glibc 2.39 | Build, tests, startup and restore passed |
| macOS arm64 | macOS 14.8.9 | Build, tests, startup and restore passed |
| macOS x86_64 | macOS 15.7.9 | Build, tests, startup and restore passed |
| Windows x86_64 | Windows Server 2022, build 20348 | Build, tests, startup and restore passed |

These are tested environments, not claims about the minimum compatible OS version.
The downloaded Linux x86_64 artifact also ran directly on the local Fedora host.

## Not claimed / 尚未验收

- Safari/macOS/iOS, Android, Windows Hello, Firefox, physical FIDO2 security keys,
  native phone cross-device flows and 30-day real-time aging are not hardware
  tested in this environment. A Chromium virtual authenticator does not establish
  those results. No generic “all browsers/devices supported” claim is made.
- No independent security review of the WebAuthn core policy wrapper has occurred.
  In particular, negative cryptographic test vectors and full hardware behavior
  require additional review. This is why releases are explicitly prereleases.
- The design's 1-vCPU/1-GiB, 10,000-bookmark p95 and memory targets have not been
  measured on that specified hardware. Storage currently uses a transactional
  JSON business snapshot inside SQLite, rather than the design's per-entity SQL
  indexes. Large-library cost needs profiling before a scale claim.
- HTML supports UTF-8 browser exports. The import UI does not currently offer a
  legacy encoding picker; decoding replacement characters are rejected rather
  than silently importing corrupt text.
- Version 0.1 only has schema 1. Unknown schemas fail closed; there is no previous
  released schema to migrate. Auth `migrate` is a compatibility check.
- No macOS notarization, Windows signing or Linux musl artifact is claimed.

The design document is the acceptance baseline, not an assertion that every item
in its full V1 hardware/operational matrix has been certified.

## Compact library UI follow-up

The library now uses name / tags / URL columns with a sticky table header.
Browser inspection with 28 demo entries confirmed 32px default rows and 28px
compact rows, including long titles, multiple tags and organize-mode controls.
At a 1440×1000 viewport, 23 full rows are visible. Mobile prioritizes name and
tags; organize mode allows horizontal scrolling. Copy/edit controls use inline
SVG icons, and notes/folder metadata remain available through the title tooltip
and editor. Frontend type checks/build and the existing end-to-end flow passed
after the change. README screenshots were refreshed from the inspected UI.

## Drag-and-drop follow-up

Desktop mouse dragging passed in Chromium: bookmark before-row ordering, moving
into a sidebar folder and out to Inbox, nesting folders, moving them to root,
and reordering sibling folders. The full authentication/import/export/recovery
scenario also passed (4.3 seconds overall) after these operations. A fixed root
drop target avoids moving the source element during drag startup. Touch dragging
is not implemented; editing and batch-move controls remain available.

All 12 Rust tests passed, including new coverage for 150-item ordering across
pagination, independent pinned positions, preserved bookmark metadata, stale
revision rejection and transactional rollback of invalid folder/anchor moves.
Frontend checks reported zero errors/warnings; production build, Rust formatting
and strict Clippy passed. Current frontend assets are approximately 79 KiB JS
(30.7 KiB gzip) and 11 KiB CSS.
