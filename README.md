# neo

A [matrix.org](https://matrix.org) chat client built with
[Relm4](https://relm4.org) (GTK4 + libadwaita), following a GNOME HIG look.

## Stack

- [`relm4`](https://relm4.org) 0.11 — Elm-architecture GUI framework on top of `gtk4-rs`
  (GTK4 + libadwaita widgets, native dark/light theming via `AdwStyleManager`)
- [`matrix-sdk`](https://github.com/matrix-org/matrix-rust-sdk) 0.13 — official Rust Matrix SDK
  (sqlite store, rustls, e2e-encryption + automatic room-key forwarding)
- `tokio` — background runtime for the network bridge

Building requires GTK4 and libadwaita development files (`.pc` files discoverable by
`pkg-config`) — on distros without a system package for these,
[Homebrew](https://brew.sh) (`brew install gtk4 libadwaita`) works too; point
`PKG_CONFIG_PATH` at its `lib/pkgconfig` (see `.cargo/config.toml`).

## Architecture

- `src/state.rs` — plain domain data shared between the bridge and the UI: `Screen`,
  `ConnectionState`, `RoomSummary`, `ChatMessage`. No `matrix-sdk` or GTK types leak in
  here; room/message IDs are plain `String`s so this module stays decoupled from both.
- `src/matrix_bridge.rs` — the entire network layer, unchanged by the UI rewrite below.
  A dedicated OS thread runs its own Tokio runtime driving a `matrix_sdk::Client`. The UI
  talks to it over an `mpsc` channel: `MatrixCmd` in (`Login`, `LoginSso`, `Poll`,
  `OpenRoom`, `LoadOlder`, `Send`, `Logout`), `MatrixEvent` out (`LoggedIn`, `SsoUrl`,
  `Rooms { rooms, space_children }`, `Timeline`, ...).
  There's no continuous sync loop — the UI drives a `Poll` command every 3s while
  connected, keeping all bridge state local to one async task instead of behind shared
  mutexes. `LoginSso` is the one command that doesn't run inline: it can sit waiting on
  a browser round-trip indefinitely, so it runs in a spawned task and reports back over
  an internal channel that the main loop also `select!`s on — otherwise it would stall
  every other command until the browser flow finished.
- `src/app.rs` — the single `AppModel` (a Relm4 `SimpleComponent`) that owns every
  screen's state. All four screens (connect, rooms, room, settings) live in one
  `gtk::Stack`, built once and toggled via `set_visible_child_name` rather than
  torn down and rebuilt on navigation — that keeps the room list and message list's
  factory-owned widgets alive across screen switches. `init()` spawns the bridge thread
  and a `relm4::spawn`ed task that forwards `MatrixEvent`s in as `AppMsg::Bridge`, plus
  the timer driving `MatrixCmd::Poll`.
- `src/factory/*.rs` — `relm4::factory` components for the dynamic lists: `RoomRow`
  (an `adw::ActionRow` per joined room), `MessageRow` (a chat bubble per timeline
  event), `SpaceChip` (a pill button per joined Space, plus "Home").

Message history is fetched via `Room::messages()` and parsed by hand (not through
`matrix-sdk-ui`'s `Timeline` widget) — this keeps the dependency surface and the
event-parsing logic small and auditable, at the cost of not getting edits/reactions/
read-receipts for free.

## Running

```sh
cargo run
```

Log in with a homeserver (defaults to `matrix.org`) either with a username and
password, or via **Sign in with SSO** for homeservers that authenticate through a
browser (Mozilla accounts, GitHub, a workplace IdP, …). SSO uses matrix-sdk's
`sso-login` feature: it spins up a short-lived localhost server, hands neo the
homeserver's SSO URL, which neo opens in your default browser via the `open` crate;
once you finish signing in there, the homeserver redirects back to that local server
with a login token neo exchanges for a session. If the browser doesn't open on its own
(sandboxed environment, no default browser configured), the connect screen shows the
URL as a fallback with an "Open again" button.

Session data (including the E2EE sqlite store) is kept under
`$XDG_DATA_HOME/neo/<homeserver>-<username>/` (`<homeserver>-sso` for SSO logins, since
the username isn't known until after the login completes).

Checking **Remember me on this device** (applies to both password and SSO login) saves
the logged-in session — homeserver, user ID, device ID, and access/refresh tokens — to
`$XDG_DATA_HOME/neo/session.json`. On the next launch neo restores it automatically and
skips the connect screen entirely. Logging out deletes this file. This also matters for
end-to-end encryption: matrix-sdk's sqlite crypto store is bound to a single device ID,
so without a remembered session, every launch would log in as a *new* device against the
same store and eventually corrupt it (`CryptoStoreError::MismatchedAccount`). As a
safety net, if neo ever detects that mismatch on login (e.g. from a store that predates
this feature), it automatically wipes and rebuilds the store for that account and retries
once, rather than leaving you stuck.

Joined [Matrix Spaces](https://spec.matrix.org/latest/client-server-api/#spaces) show up
as filter chips ("Home" + one per space) above the room list — space rooms are never
listed as chats themselves. Membership comes from each space's local `m.space.child`
state (no server hierarchy query), one level deep: sub-spaces aren't flattened into
their parent's chip, and children the space admin hasn't invited/added locally won't
show until they do.

## Known limitations

- Polling, not live sync — new messages appear up to ~3s late.
- No message edits, reactions, redactions, read receipts, or typing indicators.
- No room creation/invites from the client yet.
- SSO login can't be cancelled server-side once started — "Cancel" just stops the UI
  from waiting; if you finish the browser flow anyway, you'll still end up signed in.
- SSO store paths are keyed by homeserver only, so multiple SSO accounts on the same
  homeserver currently share one local session store.
- Space membership is one level deep and read from local state only — no
  `/hierarchy` query, so nested subspaces and not-yet-synced children aren't shown.
