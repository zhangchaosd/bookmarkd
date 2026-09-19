# Authentication integration and security policy

`bookmarkd-auth` is an embeddable Rust crate. It contains the `AuthStore` interface,
`SqliteAuthStore`, and a narrow `engine::Engine` WebAuthn policy adapter. It does not
start a server. The host supplies exact origins, app identity, storage, cookie
management, rate limits, request binding, and HTTP responses.

The verifier is `webauthn-rs-core` 0.5.5, locked in Cargo.lock. Its high-level
`webauthn-rs` API uses UUID user identifiers, which cannot preserve the design's
32-byte handle. The wrapper therefore uses the mature core verifier rather than
truncating the handle. This lower-level integration needs independent security
review before a production assurance claim. No CBOR, COSE, attestation or signature
verification is implemented by this project.

Policy enforced by the adapter:

- Preserve the exact 16–64-byte configured handle; init generates 32 random bytes.
- Registration: residentKey required, UV required, attestation none, secure library
  algorithms, no attachment restriction, existing credential exclusion.
- Login: fixed-owner credential allow-list, UV required, exact RP and origin;
  any returned userHandle must match the stored bytes.
- No origin wildcards, subdomain relaxation, arbitrary ports or cross-origin mode.
- Library sign-counter semantics apply, including legitimate zero counters.
  Backup eligibility/state is persisted, and stale concurrent snapshots fail.
- Ceremony state stays in bounded server memory for at most five minutes and is
  removed before verification. It is bound to a random HttpOnly browser cookie;
  reauthentication additionally binds the existing session ID. Restart invalidates
  all ceremonies.
- Setup mode is checked at token bind, registration begin and transaction commit.
  Token hashes expire after ten minutes, bind once to browser/app/origin, and are
  consumed atomically with credential insertion. `auto` checks live credential
  count under SQLite's immediate transaction, including across hosts.
- Session tokens have 256 random bits. Only purpose-separated SHA-256 hashes are
  stored. CSRF values are independent 256-bit synchronizer tokens kept server-side,
  returned only by the same-origin session endpoint, and compared in constant time.
- Short sessions use a session cookie and a 12-hour server deadline. Remembered
  sessions use an absolute 2,592,000-second cookie/server deadline. Neither slides.
- Every session lookup checks app, origin, expiry, and credential revocation.
  Credentials are versioned; final login updates the credential and inserts a
  session in one transaction. Revoking a credential invalidates all its hosts.

To integrate another host, load the same identity and the **same authoritative local
SQLite auth database**, choose a distinct `app_id` and origin, and use the same crate.
Keep business data separate. Copying a database once is not ongoing sharing. Never
share SQLite over NFS/SMB. The browser integration test launches a second host,
logs in with the same authenticator, rejects a copied first-host session token and
verifies cross-host revocation.

Origin checks use configured public URLs. Forwarded headers are deliberately not
trusted. Rate limits use the direct socket peer plus browser binding and a global
ceiling; behind a reverse proxy the proxy address shares a quota. Configure the
proxy's own per-client rate limits for Internet deployments.

No password, TOTP, recovery-code, anonymous bypass or testing login is shipped.
Production uses Secure, HttpOnly, SameSite=Strict host-only cookies. An explicitly
configured `http://localhost` development instance uses an unprefixed, non-Secure
cookie; this exception cannot be used with an arbitrary hostname or LAN IP.
