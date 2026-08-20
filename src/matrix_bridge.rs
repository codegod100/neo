//! Async matrix-sdk bridge: background tokio runtime ↔ egui UI thread.
//!
//! Mirrors sleek's `net.rs` shape (freeq-sdk ↔ egui bridge): the UI thread
//! sends [`MatrixCmd`]s down an `mpsc` channel, this thread drives a
//! `matrix-sdk` `Client` on a Tokio runtime and reports back [`MatrixEvent`]s.
//! There is no long-running sync loop; the UI polls via `MatrixCmd::Poll` on
//! a timer so the whole bridge fits in one straight-line async task with
//! local `client` / `active_room` state instead of shared mutexes.
//!
//! SSO login is the one exception to "straight-line": it can sit waiting on
//! the user finishing a browser flow for an arbitrarily long time, so it runs
//! in its own spawned task and reports back over an internal channel instead
//! of being `.await`ed inline — otherwise it would stall every other command
//! (including `Poll` for an already-connected session) until the browser
//! round-trip finished.
//!
//! A successful login (when "remember me" is on) persists a [`MatrixSession`]
//! to a single JSON file so the *next* launch can call `restore_session`
//! instead of logging in again. This matters beyond convenience: every fresh
//! login mints a brand-new device ID, and matrix-sdk's crypto store is bound
//! to one device — reusing the same on-disk store across logins (as neo did
//! before this existed) eventually throws `CryptoStoreError::MismatchedAccount`.
//! `run()` tries a restore once at startup, before entering its command loop.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::thread;

use matrix_sdk::authentication::matrix::MatrixSession;
use matrix_sdk::config::SyncSettings;
use matrix_sdk::deserialized_responses::SyncOrStrippedState;
use matrix_sdk::media::{MediaFormat, MediaThumbnailSettings};
use matrix_sdk::room::MessagesOptions;
use matrix_sdk::ruma::events::room::message::MessageType;
use matrix_sdk::ruma::events::space::child::SpaceChildEventContent;
use matrix_sdk::ruma::events::{AnySyncMessageLikeEvent, AnySyncTimelineEvent, SyncStateEvent};
use matrix_sdk::ruma::{uint, OwnedRoomId, RoomId, UInt};
use matrix_sdk::{Client, Room};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::state::{ChatMessage, RoomSummary};

/// How many historical messages to pull per room per fetch.
const MESSAGE_PAGE: u32 = 40;

fn message_page_limit() -> UInt {
    UInt::new(MESSAGE_PAGE as u64).unwrap_or_default()
}

/// Commands from the UI into the network thread.
#[derive(Debug, Clone)]
pub enum MatrixCmd {
    /// Log into `homeserver` with a username + password and start a session.
    /// If `remember` is set, the session is persisted so the next launch can
    /// restore it instead of logging in fresh (see the module doc).
    Login { homeserver: String, username: String, password: String, remember: bool },
    /// Log into `homeserver` via SSO (`m.login.sso`). The bridge reports the
    /// URL to open (see [`MatrixEvent::SsoUrl`]) and waits for the browser
    /// redirect carrying the login token. `remember` — see [`MatrixCmd::Login`].
    LoginSso { homeserver: String, remember: bool },
    /// Re-fetch the room list and (if any) the active room's latest messages.
    Poll,
    /// Switch the "watched" room — fetches its most recent messages.
    OpenRoom(String),
    /// Fetch older messages for `room_id`, prepending them.
    LoadOlder(String),
    /// Send a plain-text message to `room_id`.
    Send { room_id: String, text: String },
    /// Drop the session.
    Logout,
}

/// Events from the network thread back to the UI.
#[derive(Debug, Clone)]
pub enum MatrixEvent {
    Connecting,
    /// The homeserver's SSO URL is ready — the UI should open it in the
    /// system browser (and can offer it again as a fallback link/button).
    SsoUrl(String),
    LoggedIn { user_id: String, homeserver: String },
    LoginFailed(String),
    /// The joined-room list, plus which room IDs each joined space claims as
    /// children (via `m.space.child` state, one level deep — nested
    /// subspaces aren't flattened). Keyed by space room ID.
    Rooms { rooms: Vec<RoomSummary>, space_children: HashMap<String, Vec<String>> },
    Timeline { room_id: String, messages: Vec<ChatMessage>, prepend: bool },
    SendFailed { room_id: String, error: String },
    Error(String),
    LoggedOut,
}

/// Spawns the bridge thread and returns the command sender + event receiver
/// the UI thread should hold onto.
pub fn spawn() -> (mpsc::UnboundedSender<MatrixCmd>, mpsc::UnboundedReceiver<MatrixEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<MatrixCmd>();
    let (evt_tx, evt_rx) = mpsc::unbounded_channel::<MatrixEvent>();

    thread::Builder::new()
        .name("neo-matrix".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("build tokio runtime");
            rt.block_on(run(cmd_rx, evt_tx));
        })
        .expect("spawn matrix bridge thread");

    (cmd_tx, evt_rx)
}

/// What a completed (successful or not) SSO login attempt reports back to
/// the main loop over the internal `sso_rx` channel.
type SsoLoginResult = anyhow::Result<(Client, String, String)>;

async fn run(mut cmd_rx: mpsc::UnboundedReceiver<MatrixCmd>, evt_tx: mpsc::UnboundedSender<MatrixEvent>) {
    let mut client: Option<Client> = None;
    let mut sync_token: Option<String> = None;
    let mut active_room: Option<String> = None;

    // Try to pick up where the last "remember me" login left off, before
    // servicing any commands. Silent no-op if there's nothing remembered.
    if let Some((restored, user_id, hs)) = try_restore_session().await {
        let _ = evt_tx.send(MatrixEvent::Connecting);
        sync_token = initial_sync(&restored, &evt_tx).await;
        client = Some(restored);
        let _ = evt_tx.send(MatrixEvent::LoggedIn { user_id, homeserver: hs });
    }

    // See the module doc: SSO logins run on a spawned task and report back
    // here instead of being awaited inline, so this loop can keep servicing
    // `Poll`/etc. while one is in flight.
    let (sso_tx, mut sso_rx) = mpsc::unbounded_channel::<SsoLoginResult>();

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else { break };
                match cmd {
                    MatrixCmd::Login { homeserver, username, password, remember } => {
                        let _ = evt_tx.send(MatrixEvent::Connecting);
                        match login(&homeserver, &username, &password, remember).await {
                            Ok((new_client, user_id, hs)) => {
                                sync_token = initial_sync(&new_client, &evt_tx).await;
                                client = Some(new_client);
                                let _ = evt_tx.send(MatrixEvent::LoggedIn { user_id, homeserver: hs });
                            }
                            Err(err) => {
                                let _ = evt_tx.send(MatrixEvent::LoginFailed(err.to_string()));
                            }
                        }
                    }

                    MatrixCmd::LoginSso { homeserver, remember } => {
                        let _ = evt_tx.send(MatrixEvent::Connecting);
                        let evt_tx_task = evt_tx.clone();
                        let sso_tx_task = sso_tx.clone();
                        tokio::spawn(async move {
                            let result = login_sso(&homeserver, &evt_tx_task, remember).await;
                            let _ = sso_tx_task.send(result);
                        });
                    }

                    MatrixCmd::Poll => {
                        let Some(c) = client.as_ref() else { continue };
                        match sync_step(c, sync_token.clone()).await {
                            Ok(token) => sync_token = Some(token),
                            Err(err) => {
                                let _ = evt_tx.send(MatrixEvent::Error(format!("sync: {err}")));
                                continue;
                            }
                        }
                        send_room_list(c, &evt_tx).await;
                        if let Some(room_id) = active_room.clone() {
                            refresh_room(c, &room_id, &evt_tx, true).await;
                        }
                    }

                    MatrixCmd::OpenRoom(room_id) => {
                        active_room = Some(room_id.clone());
                        if let Some(c) = client.as_ref() {
                            refresh_room(c, &room_id, &evt_tx, true).await;
                        }
                    }

                    MatrixCmd::LoadOlder(room_id) => {
                        if let Some(c) = client.as_ref() {
                            load_older(c, &room_id, &evt_tx).await;
                        }
                    }

                    MatrixCmd::Send { room_id, text } => {
                        let Some(c) = client.as_ref() else { continue };
                        if let Err(err) = send_message(c, &room_id, &text).await {
                            let _ = evt_tx.send(MatrixEvent::SendFailed { room_id, error: err.to_string() });
                        } else if let Some(active) = active_room.clone() {
                            refresh_room(c, &active, &evt_tx, true).await;
                        }
                    }

                    MatrixCmd::Logout => {
                        client = None;
                        sync_token = None;
                        active_room = None;
                        clear_saved_session();
                        let _ = evt_tx.send(MatrixEvent::LoggedOut);
                    }
                }
            }

            Some(result) = sso_rx.recv() => {
                match result {
                    Ok((new_client, user_id, hs)) => {
                        sync_token = initial_sync(&new_client, &evt_tx).await;
                        client = Some(new_client);
                        let _ = evt_tx.send(MatrixEvent::LoggedIn { user_id, homeserver: hs });
                    }
                    Err(err) => {
                        let _ = evt_tx.send(MatrixEvent::LoginFailed(err.to_string()));
                    }
                }
            }
        }
    }
}

async fn login(
    homeserver: &str,
    username: &str,
    password: &str,
    remember: bool,
) -> anyhow::Result<(Client, String, String)> {
    let raw = homeserver.trim();
    let store_path = store_dir(raw, username);

    let attempt = || async { login_once(raw, username, password, &store_path).await };
    let result = match attempt().await {
        Err(err) if is_stale_crypto_store(&err) => {
            wipe_store(&store_path);
            attempt().await
        }
        other => other,
    }?;

    if remember {
        save_session(raw, &store_path, &result.0);
    }
    Ok(result)
}

async fn login_once(
    homeserver: &str,
    username: &str,
    password: &str,
    store_path: &Path,
) -> anyhow::Result<(Client, String, String)> {
    std::fs::create_dir_all(store_path).ok();

    let client = Client::builder()
        .server_name_or_homeserver_url(homeserver)
        .sqlite_store(store_path, None)
        .build()
        .await?;

    let response = client
        .matrix_auth()
        .login_username(username, password)
        .initial_device_display_name("neo (vidya)")
        .send()
        .await?;

    let user_id = response.user_id.to_string();
    let hs = client.homeserver().to_string();
    Ok((client, user_id, hs))
}

/// Log into `homeserver` via SSO (`m.login.sso`). matrix-sdk's `sso-login`
/// feature spins up a short-lived localhost server for the redirect and
/// hands us the login URL to open; we only report it back to the UI thread
/// (over `evt_tx`) rather than opening a browser ourselves, since that's an
/// OS-integration concern the UI thread already owns via the `open` crate.
async fn login_sso(
    homeserver: &str,
    evt_tx: &mpsc::UnboundedSender<MatrixEvent>,
    remember: bool,
) -> anyhow::Result<(Client, String, String)> {
    let raw = homeserver.trim();
    // Keyed by homeserver only (no username yet at this point) — see the
    // `store_dir` doc comment for the SSO caveat this implies.
    let store_path = store_dir(raw, "sso");

    let attempt = || async { login_sso_once(raw, evt_tx, &store_path).await };
    let result = match attempt().await {
        Err(err) if is_stale_crypto_store(&err) => {
            wipe_store(&store_path);
            attempt().await
        }
        other => other,
    }?;

    if remember {
        save_session(raw, &store_path, &result.0);
    }
    Ok(result)
}

async fn login_sso_once(
    homeserver: &str,
    evt_tx: &mpsc::UnboundedSender<MatrixEvent>,
    store_path: &Path,
) -> anyhow::Result<(Client, String, String)> {
    std::fs::create_dir_all(store_path).ok();

    let client = Client::builder()
        .server_name_or_homeserver_url(homeserver)
        .sqlite_store(store_path, None)
        .build()
        .await?;

    let sso_evt_tx = evt_tx.clone();
    let response = client
        .matrix_auth()
        .login_sso(move |sso_url| {
            let sso_evt_tx = sso_evt_tx.clone();
            async move {
                let _ = sso_evt_tx.send(MatrixEvent::SsoUrl(sso_url));
                Ok(())
            }
        })
        .initial_device_display_name("neo (vidya)")
        .send()
        .await?;

    let user_id = response.user_id.to_string();
    let hs = client.homeserver().to_string();
    Ok((client, user_id, hs))
}

/// True if `err` is matrix-sdk-crypto's `MismatchedAccount`: the on-disk
/// crypto store already holds Olm data for a different device than the one
/// this login just obtained. Happens whenever a store directory is reused
/// across logins that each mint a new device ID — the case before session
/// persistence existed (every relaunch logged in fresh into the same store),
/// or if a remembered session was ever invalidated server-side. The store
/// can't straddle two devices, so the caller wipes it and retries once with
/// a clean slate — this loses that device's E2EE key history, but recovers
/// a client that was otherwise stuck failing every login.
fn is_stale_crypto_store(err: &anyhow::Error) -> bool {
    let matched_type = err.downcast_ref::<matrix_sdk::Error>().is_some_and(|e| {
        matches!(
            e,
            matrix_sdk::Error::CryptoStoreError(inner)
                if matches!(**inner, matrix_sdk::crypto::CryptoStoreError::MismatchedAccount { .. })
        )
    });
    // Belt-and-suspenders: fall back to matching the (stable, hand-written)
    // error text in case a future matrix-sdk version reaches this error
    // through a different wrapper type than the one matched above.
    matched_type || err.to_string().contains("doesn't match the account in the constructor")
}

fn wipe_store(store_path: &Path) {
    let _ = std::fs::remove_dir_all(store_path);
}

/// Sqlite store directory for a given `(homeserver, username)` pair.
/// `homeserver` here is whatever the user typed (a bare server name like
/// `mozilla.org` or a full URL) — it's only ever used as a filesystem-safe
/// cache key, never parsed, so this doesn't need to agree with what
/// `.well-known` discovery resolves it to internally. For SSO logins
/// `username` is unknown until after the login completes (the store path has
/// to be picked before the client is built), so `login_sso` passes a fixed
/// `"sso"` placeholder instead — meaning distinct SSO accounts on the same
/// homeserver currently share one store. Fine for the common case of one
/// account per homeserver; a real multi-account setup would need to build the
/// client without a store, log in, then move the data once the user ID is
/// known.
fn store_dir(homeserver: &str, username: &str) -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("neo");
    let safe_host = homeserver.replace(['/', ':'], "_");
    let safe_user = username.replace(['@', ':', '/'], "_");
    base.join(format!("{safe_host}-{safe_user}"))
}

/// The one remembered session, if any — see the module doc. There's only
/// ever one: logging in as someone else overwrites it, logging out deletes
/// it. `homeserver` is whatever the user originally typed (passed straight
/// through to `server_name_or_homeserver_url` again on restore, same as a
/// fresh login), not the resolved URL — so a homeserver that migrates its
/// `.well-known` target between launches is still picked up correctly.
#[derive(Serialize, Deserialize)]
struct SavedSession {
    homeserver: String,
    store_path: PathBuf,
    session: MatrixSession,
}

fn session_file() -> PathBuf {
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("neo").join("session.json")
}

/// Best-effort: a failure to persist just means the next launch logs in
/// fresh instead of restoring, not a fatal error for *this* session.
fn save_session(homeserver: &str, store_path: &Path, client: &Client) {
    let Some(session) = client.matrix_auth().session() else { return };
    let saved = SavedSession { homeserver: homeserver.to_owned(), store_path: store_path.to_owned(), session };
    let Ok(json) = serde_json::to_vec_pretty(&saved) else { return };
    let path = session_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let _ = std::fs::write(path, json);
}

fn clear_saved_session() {
    let _ = std::fs::remove_file(session_file());
}

/// Attempt to pick up the last "remember me" session. `None` covers every
/// non-fatal outcome (nothing saved, file unreadable, server rejected the
/// token, ...) — the caller just falls through to the normal connect screen.
/// A stale/corrupt entry is deleted so it doesn't keep failing on every launch.
async fn try_restore_session() -> Option<(Client, String, String)> {
    let path = session_file();
    let bytes = std::fs::read(&path).ok()?;
    let Ok(saved) = serde_json::from_slice::<SavedSession>(&bytes) else {
        let _ = std::fs::remove_file(&path);
        return None;
    };

    let client = Client::builder()
        .server_name_or_homeserver_url(&saved.homeserver)
        .sqlite_store(&saved.store_path, None)
        .build()
        .await
        .ok()?;

    let user_id = saved.session.meta.user_id.clone();
    if client.restore_session(saved.session).await.is_err() {
        // Most likely the server revoked the token (password change, admin
        // action, "sign out all devices", ...) — the store may also be
        // unusable for a future fresh login into the same device slot, so
        // clear both rather than leaving something around that just fails
        // the same way again next launch.
        clear_saved_session();
        wipe_store(&saved.store_path);
        return None;
    }

    let hs = client.homeserver().to_string();
    Some((client, user_id.to_string(), hs))
}

/// First sync after login — establishes the sync token and pushes the room list.
async fn initial_sync(client: &Client, evt_tx: &mpsc::UnboundedSender<MatrixEvent>) -> Option<String> {
    match sync_step(client, None).await {
        Ok(token) => {
            send_room_list(client, evt_tx).await;
            Some(token)
        }
        Err(err) => {
            let _ = evt_tx.send(MatrixEvent::Error(format!("initial sync: {err}")));
            None
        }
    }
}

async fn sync_step(client: &Client, token: Option<String>) -> anyhow::Result<String> {
    let mut settings = SyncSettings::default().timeout(std::time::Duration::from_secs(10));
    if let Some(t) = token {
        settings = settings.token(t);
    }
    let resp = client.sync_once(settings).await?;
    Ok(resp.next_batch)
}

async fn send_room_list(client: &Client, evt_tx: &mpsc::UnboundedSender<MatrixEvent>) {
    let joined_rooms = client.rooms();

    let mut rooms: Vec<RoomSummary> = Vec::new();
    for room in &joined_rooms {
        rooms.push(room_summary(room).await);
    }
    rooms.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let mut space_children: HashMap<String, Vec<String>> = HashMap::new();
    for room in &joined_rooms {
        if !room.is_space() {
            continue;
        }
        let children = space_child_room_ids(room).await;
        if !children.is_empty() {
            space_children.insert(room.room_id().to_string(), children);
        }
    }

    let _ = evt_tx.send(MatrixEvent::Rooms { rooms, space_children });
}

async fn room_summary(room: &Room) -> RoomSummary {
    let name = room.name().unwrap_or_else(|| room.room_id().to_string());
    let is_space = room.is_space();
    // Only spaces need their avatar today (for the filter chips above the
    // room list), so skip the download for every other room.
    let avatar = if is_space { space_avatar(room).await } else { None };
    RoomSummary {
        id: room.room_id().to_string(),
        name,
        preview: String::new(),
        encrypted: room.encryption_state().is_encrypted(),
        direct: room.is_direct().await.unwrap_or(false),
        is_space,
        avatar,
    }
}

/// Small thumbnail of a space's avatar, sized for the filter chip row.
async fn space_avatar(room: &Room) -> Option<Vec<u8>> {
    let format = MediaFormat::Thumbnail(MediaThumbnailSettings::new(uint!(64), uint!(64)));
    room.avatar(format).await.ok().flatten()
}

/// Room IDs a joined space (`space`) advertises as children via `m.space.child`
/// state events, read from local sync state (no server hierarchy query). An
/// empty `via` list means the relationship was removed, so those are skipped;
/// this only reflects what's already in `space`'s state, so children the user
/// hasn't joined won't be discoverable this way — good enough for filtering
/// the room list down to rooms already in `client.rooms()`.
async fn space_child_room_ids(space: &Room) -> Vec<String> {
    let Ok(events) = space.get_state_events_static::<SpaceChildEventContent>().await else {
        return Vec::new();
    };
    events
        .iter()
        .filter_map(|raw| {
            let SyncOrStrippedState::Sync(SyncStateEvent::Original(orig)) = raw.deserialize().ok()?
            else {
                return None;
            };
            if orig.content.via.is_empty() {
                return None;
            }
            Some(orig.state_key.to_string())
        })
        .collect()
}

/// Fetch the most recent page of messages for `room_id` (fresh, not incremental).
async fn refresh_room(
    client: &Client,
    room_id: &str,
    evt_tx: &mpsc::UnboundedSender<MatrixEvent>,
    replace: bool,
) {
    let Ok(rid) = RoomId::parse(room_id) else { return };
    let Some(room) = client.get_room(&rid) else { return };

    let mut options = MessagesOptions::backward();
    options.limit = message_page_limit();
    match room.messages(options).await {
        Ok(page) => {
            let own_id = client.user_id().map(|u| u.to_string());
            let mut messages: Vec<ChatMessage> = page
                .chunk
                .iter()
                .filter_map(|te| to_chat_message(te, own_id.as_deref()))
                .collect();
            messages.reverse(); // oldest first for display
            let _ = evt_tx.send(MatrixEvent::Timeline {
                room_id: room_id.to_owned(),
                messages,
                prepend: !replace,
            });
        }
        Err(err) => {
            let _ = evt_tx.send(MatrixEvent::Error(format!("messages: {err}")));
        }
    }
}

async fn load_older(client: &Client, room_id: &str, evt_tx: &mpsc::UnboundedSender<MatrixEvent>) {
    let Ok(rid) = RoomId::parse(room_id) else { return };
    let Some(room) = client.get_room(&rid) else { return };

    let mut options = MessagesOptions::backward();
    options.limit = message_page_limit();
    match room.messages(options).await {
        Ok(page) => {
            let own_id = client.user_id().map(|u| u.to_string());
            let mut messages: Vec<ChatMessage> = page
                .chunk
                .iter()
                .filter_map(|te| to_chat_message(te, own_id.as_deref()))
                .collect();
            messages.reverse();
            let _ = evt_tx.send(MatrixEvent::Timeline {
                room_id: room_id.to_owned(),
                messages,
                prepend: true,
            });
        }
        Err(err) => {
            let _ = evt_tx.send(MatrixEvent::Error(format!("older messages: {err}")));
        }
    }
}

async fn send_message(client: &Client, room_id: &str, text: &str) -> anyhow::Result<()> {
    let rid: OwnedRoomId = RoomId::parse(room_id)?.to_owned();
    let room = client.get_room(&rid).ok_or_else(|| anyhow::anyhow!("room not joined"))?;
    let content = matrix_sdk::ruma::events::room::message::RoomMessageEventContent::text_plain(text);
    room.send(content).await?;
    Ok(())
}

fn to_chat_message(
    te: &matrix_sdk::deserialized_responses::TimelineEvent,
    own_id: Option<&str>,
) -> Option<ChatMessage> {
    let ev = te.raw().deserialize().ok()?;
    let (sender, body, ts) = message_body(&ev)?;
    Some(ChatMessage {
        event_id: None,
        own: own_id.is_some_and(|me| me == sender),
        sender,
        body,
        ts_millis: ts,
        pending: false,
    })
}

/// Extract `(sender, readable body, origin_server_ts millis)` from a
/// `m.room.message` event; `None` for anything else (state events, reactions,
/// unsupported msgtypes are skipped rather than shown as raw JSON).
fn message_body(ev: &AnySyncTimelineEvent) -> Option<(String, String, i64)> {
    let AnySyncTimelineEvent::MessageLike(AnySyncMessageLikeEvent::RoomMessage(msg)) = ev else {
        return None;
    };
    let orig = msg.as_original()?;
    let sender = orig.sender.to_string();
    let ts: i64 = orig.origin_server_ts.0.into();
    let body = match &orig.content.msgtype {
        MessageType::Text(t) => t.body.clone(),
        MessageType::Emote(t) => format!("* {} {}", orig.sender.localpart(), t.body),
        MessageType::Notice(t) => t.body.clone(),
        MessageType::Image(t) => format!("🖼 {}", t.body),
        MessageType::File(t) => format!("📎 {}", t.body),
        MessageType::Audio(t) => format!("🔊 {}", t.body),
        MessageType::Video(t) => format!("🎬 {}", t.body),
        MessageType::Location(t) => format!("📍 {}", t.body),
        _ => "[unsupported message]".to_owned(),
    };
    Some((sender, body, ts))
}
