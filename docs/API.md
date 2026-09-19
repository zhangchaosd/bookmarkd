# HTTP API quick reference

The frontend (`web/src/api.ts`, `web/src/App.svelte`) is the executable integration
example. All data routes are under `/api/v1`; no token-to-API login shortcut exists.
Use a normal Passkey session. Bodies are JSON, errors use
`{"error":{"code","message","request_id","details"?}}` with actual HTTP status.
All responses are `Cache-Control: no-store`. CORS is not enabled.

Every write requires the exact configured `Origin`, JSON Content-Type and a valid
`X-CSRF-Token` from `GET /api/v1/session`. Authentication ceremonies instead use
origin checks and a browser binding cookie; registration additionally needs a
terminal-issued one-time authorization. Session token values are never returned in
JSON. Export, permanent trash purge and session-management writes need Passkey
authentication within five minutes; call `/auth/reauth/begin` and `finish` when
`RECENT_AUTH_REQUIRED` is returned.

| Endpoint | Contract |
|---|---|
| `GET /api/v1/session` | Anonymous: only `authenticated:false`; authenticated: CSRF and limited session metadata |
| `POST /auth/setup/verify` | `{token}`; binds setup grant to browser |
| `POST /auth/register/begin` | `{}`; returns standard `publicKey`, ceremony ID and expiration |
| `POST /auth/register/finish` | `{ceremony_id, credential}`; consumes grant; does not log in |
| `POST /auth/login/begin` | `{remember_30d:false}`; standard WebAuthn options |
| `POST /auth/login/finish` | `{ceremony_id, credential}`; sets HttpOnly cookie |
| `POST /auth/reauth/begin`, `finish` | Same format, bound to existing session; does not extend expiry |
| `POST /logout` | Revoke current session and expire its cookie |
| `GET /api/v1/bookmarks` | `q`, `folder_id` (empty = inbox), `tag`, `pinned`, `offset`, `limit` (1–500); returns items/total/revision |
| `GET /api/v1/bookmarks/{id}` | Full entity including version |
| `POST /api/v1/bookmarks` | `{url_raw,title?,notes?,tags?,folder_id?,pinned?}`; optional Idempotency-Key |
| `PATCH /api/v1/bookmarks/{id}` | Partial fields and expected `version` |
| `DELETE /api/v1/bookmarks/{id}` | `{version}`; soft delete |
| `POST /api/v1/bookmarks/{id}/restore` | `{version,folder_id?}`; unavailable original folder must be resolved |
| `POST /api/v1/bookmarks/batch` | `{action,items:[{id,version}],folder_id?,tags?}`; actions move/delete/add-tags/remove-tags, ≤500, all-or-nothing |
| `POST /api/v1/bookmarks/reorder` | `{scope:"pinned"|"folder",items:[{id,version}]}` in desired order |
| `GET/POST /api/v1/folders` | List/create; creation `{name,parent_id?}` |
| `PATCH /api/v1/folders/{id}` | `{version,name?,parent_id?}`; rejects cycles, depth >32 and sibling collisions |
| `DELETE /api/v1/folders/{id}` | `{version,strategy:"trash"|"move",folder_id?}`; descendants handled transactionally |
| `POST /api/v1/folders/{id}/restore` | `{version,parent_id?}`; restore same deletion batch, reject name conflicts |
| `POST /api/v1/folders/reorder` | `{items:[{id,version}]}` |
| `GET/POST /api/v1/tags` | List display names / create `{name}` |
| `PATCH/DELETE /api/v1/tags/{urlencoded-name}` | `{revision,name?}`; tag rename/remove across bookmarks; revision protects bulk changes |
| `GET /api/v1/library/revision` | Lightweight change counter |
| `GET /api/v1/trash` | Deleted bookmarks and folders |
| `POST /api/v1/trash/purge` | `{confirm:true}`; permanently remove all trash, recent auth required |
| `POST /api/v1/imports` | `{format:"html"|"json",content}`; preview only; returns ID, warnings, counts and digest |
| `GET/DELETE /api/v1/imports/{id}` | Read/cancel in-memory preview; expires in one hour or at restart |
| `POST /api/v1/imports/{id}/commit` | `{mode:"merge"|"restore",folder_id?,keep_duplicates?}`; idempotent per preview and request body; restore requires empty library |
| `POST /api/v1/exports` | `{format:"html"|"json",includes_trash?}`; attachment response; recent auth required |
| `GET/PATCH /api/v1/preferences` | JSON preferences; PATCH requires expected version |
| `GET /api/v1/auth/credentials` | Names and IDs only; never serialized key material |
| `PATCH /api/v1/auth/credentials/{id}` | `{name}`; recent auth |
| `GET /api/v1/auth/sessions` | Current host only, no session hashes or CSRF values |
| `DELETE /api/v1/auth/sessions/{id}` | Current host revocation; recent auth |
| `POST /api/v1/auth/sessions/revoke-others` | Keep current session, revoke other current-host sessions |
| `GET /healthz` | Minimal public liveness |

Entity timestamps are UTC Unix seconds; versions and library revisions are
monotonic integers. URLs preserve user spelling; comparison uses conservative URL
parser normalization without stripping queries, fragments or tracking parameters.
New entities have random UUID IDs. Merge import remaps IDs; full JSON restore keeps
them. Tags are normalized display-name sets and are serialized inline in bookmarks,
with separately created names in the top-level `tags` list. This differs from the
design's relational tag-ID sketch; exports roundtrip this implementation's format.

Business storage is a SQLite transaction containing a complete typed library
snapshot and a separate idempotency table. This favors simple all-or-nothing
validation; large-library performance needs measurement. AuthStore is independent
and has normalized identity, credential, grant and session tables.
