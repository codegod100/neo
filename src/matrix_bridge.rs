//! Async matrix-sdk bridge: background tokio runtime ↔ Relm4/GTK UI thread.
//!
//! Sync is push-based sliding sync (MSC4186), not classic `/sync` polling:
//! [`matrix_sdk_ui::sync_service::SyncService`] drives the joined-room list
//! (via `RoomListService`), a second hand-rolled [`matrix_sdk::sliding_sync`]
//! instance drives Matrix Spaces (see the "spaces gotcha" note on
//! [`spawn_space_sync`]), and each open room gets its own
//! [`matrix_sdk_ui::timeline::Timeline`] subscription. All three keep running
//! in the background for as long as the session is alive — there is no
//! `MatrixCmd::Poll` and nothing in `run()`'s command loop blocks behind a
//! long-poll, so `Send`/`OpenRoom`/`LoadOlder` never queue behind sync.
//!
//! The UI thread sends [`MatrixCmd`]s down an `mpsc` channel; this thread
//! reports back [`MatrixEvent`]s. Every event is a full, already-flattened
//! snapshot (`Vec<RoomSummary>`, `Vec<ChatMessage>`, ...) rebuilt from a
//! diff-stream's local `eyeball_im::Vector` each time it changes — the UI
//! side keeps its current clear+rebuild `FactoryVecDeque` pattern unchanged;
//! incremental `VectorDiff` application into the widgets is a separate
//! follow-up, not part of this migration.
//!
//! SSO login is the one command that can sit waiting on the user finishing a
//! browser flow for an arbitrarily long time, so — like before — it runs in
//! its own spawned task and reports back over an internal channel instead of
//! being `.await`ed inline in the command loop.
//!
//! A successful login (when "remember me" is on) persists a [`MatrixSession`]
//! to a single JSON file so the *next* launch can call `try_restore_session`
//! instead of logging in again. This matters beyond convenience: every fresh
//! login mints a brand-new device ID, and matrix-sdk's crypto store is bound
//! to one device — reusing the same on-disk store across logins eventually
//! throws `CryptoStoreError::MismatchedAccount`. `run()` tries a restore once
//! at startup, before entering its command loop.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

use eyeball_im::Vector;
use futures_util::{pin_mut, StreamExt};
use matrix_sdk::authentication::matrix::MatrixSession;
use matrix_sdk::deserialized_responses::SyncOrStrippedState;
use matrix_sdk::media::{MediaFormat, MediaThumbnailSettings};
use matrix_sdk::ruma::api::client::space::get_hierarchy;
use matrix_sdk::ruma::api::client::sync::sync_events::v5::request::ListFilters;
use matrix_sdk::ruma::directory::RoomTypeFilter;
use matrix_sdk::ruma::events::room::message::{MessageType, RoomMessageEventContent};
use matrix_sdk::ruma::events::space::child::SpaceChildEventContent;
use matrix_sdk::ruma::events::{StateEventType, SyncStateEvent};
use matrix_sdk::ruma::{uint, RoomId};
use matrix_sdk::sliding_sync::{SlidingSyncList, SlidingSyncMode, Version};
use matrix_sdk::{Client, Room, RoomState};
use matrix_sdk_ui::sync_service::SyncService;
use matrix_sdk_ui::timeline::{RoomExt, Timeline, TimelineItem};
use matrix_sdk_ui::room_list_service::filters;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::state::{ChatMessage, LobbyRoom, RoomSummary};

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
    /// Switch the "watched" room — (re)subscribes to its live timeline.
    OpenRoom(String),
    /// Paginate the currently-open room's timeline backwards.
    LoadOlder(String),
    /// Send a plain-text message to `room_id`.
    Send { room_id: String, text: String },
    /// Fetch a space's full room directory (joined + not-yet-joined) via the
    /// server's `/hierarchy` endpoint, for that space's "Lobby" screen.
    OpenLobby(String),
    /// Join a room surfaced in a space's Lobby directory, then re-fetch that
    /// space's hierarchy so the row flips to "joined".
    JoinRoom { room_id: String, space_id: String },
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
    /// The full joined-room list, re-flattened from the room-list service's
    /// live `Vector<Room>` every time it changes.
    Rooms(Vec<RoomSummary>),
    /// Which room IDs each joined space claims as children (via
    /// `m.space.child` state, one level deep — nested subspaces aren't
    /// flattened). Keyed by space room ID. Updated independently of
    /// [`MatrixEvent::Rooms`] by the hand-rolled spaces sync.
    SpaceChildren(HashMap<String, Vec<String>>),
    /// A full snapshot of the currently-open room's timeline, oldest first.
    Timeline { room_id: String, messages: Vec<ChatMessage> },
    SendFailed { room_id: String, error: String },
    /// A space's full room directory, fetched via `/hierarchy` for its
    /// "Lobby" screen — `rooms` excludes the space's own row.
    SpaceHierarchy { space_id: String, rooms: Vec<LobbyRoom> },
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

/// Everything spun up by [`start_sync`] for one logged-in session: the
/// `SyncService` (drives the joined-room list), the hand-rolled spaces
/// `SlidingSync` background task, and — once a room is opened — that room's
/// live `Timeline` subscription. Bundled together so `Logout`/re-login can
/// tear it all down in one place.
struct ActiveSync {
    service: Arc<SyncService>,
    room_list_task: JoinHandle<()>,
    space_sync_task: JoinHandle<()>,
    active_room: Option<String>,
    timeline_task: Option<JoinHandle<()>>,
    active_timeline: Option<Arc<Timeline>>,
}

impl ActiveSync {
    async fn stop(self) {
        if let Some(task) = self.timeline_task {
            task.abort();
        }
        self.room_list_task.abort();
        self.space_sync_task.abort();
        self.service.stop().await;
    }

    /// Switch the live timeline subscription to `room_id`, aborting whatever
    /// was open before. Blocking here (rather than spawning the switch
    /// itself) is deliberate: building+subscribing a `Timeline` is a bounded,
    /// local-store-backed operation — nothing like the old 10s sync
    /// long-poll it replaces — so a brief wait before the next command is
    /// serviced is an acceptable trade for keeping this straightforward.
    async fn switch_room(
        &mut self,
        client: &Client,
        room_id: String,
        evt_tx: mpsc::UnboundedSender<MatrixEvent>,
    ) {
        if let Some(task) = self.timeline_task.take() {
            task.abort();
        }
        self.active_timeline = None;
        self.active_room = Some(room_id.clone());

        let Ok(rid) = RoomId::parse(&room_id) else { return };
        let Some(room) = client.get_room(&rid) else { return };

        let timeline = match room.timeline().await {
            Ok(t) => Arc::new(t),
            Err(err) => {
                let _ = evt_tx.send(MatrixEvent::Error(format!("timeline: {err}")));
                return;
            }
        };
        let (initial, diff_stream) = timeline.subscribe().await;
        self.active_timeline = Some(timeline);

        self.timeline_task = Some(tokio::spawn(async move {
            let mut items = initial;
            let _ = evt_tx.send(MatrixEvent::Timeline {
                room_id: room_id.clone(),
                messages: flatten_chat_messages(&items),
            });

            pin_mut!(diff_stream);
            while let Some(diffs) = diff_stream.next().await {
                for diff in diffs {
                    diff.apply(&mut items);
                }
                let _ = evt_tx.send(MatrixEvent::Timeline {
                    room_id: room_id.clone(),
                    messages: flatten_chat_messages(&items),
                });
            }
        }));
    }
}

async fn run(mut cmd_rx: mpsc::UnboundedReceiver<MatrixCmd>, evt_tx: mpsc::UnboundedSender<MatrixEvent>) {
    let mut client: Option<Client> = None;
    let mut sync: Option<ActiveSync> = None;

    // Try to pick up where the last "remember me" login left off, before
    // servicing any commands. Silent no-op if there's nothing remembered.
    if let Some((restored, user_id, hs)) = try_restore_session().await {
        let _ = evt_tx.send(MatrixEvent::Connecting);
        match start_sync(restored.clone(), evt_tx.clone()).await {
            Ok(started) => {
                sync = Some(started);
                client = Some(restored);
                let _ = evt_tx.send(MatrixEvent::LoggedIn { user_id, homeserver: hs });
            }
            Err(err) => {
                let _ = evt_tx.send(MatrixEvent::Error(format!("sync: {err}")));
            }
        }
    }

    // See the module doc: SSO logins run on a spawned task and report back
    // here instead of being awaited inline, so this loop keeps servicing
    // other commands while one is in flight.
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
                                if let Some(prev) = sync.take() {
                                    prev.stop().await;
                                }
                                match start_sync(new_client.clone(), evt_tx.clone()).await {
                                    Ok(started) => {
                                        sync = Some(started);
                                        client = Some(new_client);
                                        let _ = evt_tx.send(MatrixEvent::LoggedIn { user_id, homeserver: hs });
                                    }
                                    Err(err) => {
                                        let _ = evt_tx.send(MatrixEvent::LoginFailed(err.to_string()));
                                    }
                                }
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

                    MatrixCmd::OpenRoom(room_id) => {
                        if let (Some(c), Some(active_sync)) = (client.as_ref(), sync.as_mut()) {
                            active_sync.switch_room(c, room_id, evt_tx.clone()).await;
                        }
                    }

                    MatrixCmd::LoadOlder(room_id) => {
                        if let Some(active_sync) = sync.as_ref() {
                            if active_sync.active_room.as_deref() == Some(room_id.as_str()) {
                                if let Some(timeline) = active_sync.active_timeline.clone() {
                                    let evt_tx = evt_tx.clone();
                                    tokio::spawn(async move {
                                        if let Err(e) = timeline.paginate_backwards(40).await {
                                            let _ = evt_tx.send(MatrixEvent::Error(format!("paginate: {e}")));
                                        }
                                    });
                                }
                            }
                        }
                    }

                    MatrixCmd::Send { room_id, text } => {
                        if let Some(active_sync) = sync.as_ref() {
                            if active_sync.active_room.as_deref() == Some(room_id.as_str()) {
                                if let Some(timeline) = active_sync.active_timeline.clone() {
                                    let evt_tx = evt_tx.clone();
                                    tokio::spawn(async move {
                                        let content = RoomMessageEventContent::text_plain(&text);
                                        if let Err(e) = timeline.send(content.into()).await {
                                            let _ = evt_tx.send(MatrixEvent::SendFailed {
                                                room_id,
                                                error: e.to_string(),
                                            });
                                        }
                                    });
                                }
                            }
                        }
                    }

                    MatrixCmd::OpenLobby(space_id) => {
                        if let Some(c) = client.clone() {
                            let evt_tx = evt_tx.clone();
                            tokio::spawn(async move {
                                match fetch_hierarchy(&c, &space_id).await {
                                    Ok(rooms) => {
                                        let _ = evt_tx.send(MatrixEvent::SpaceHierarchy { space_id, rooms });
                                    }
                                    Err(err) => {
                                        let _ = evt_tx.send(MatrixEvent::Error(format!("lobby: {err}")));
                                    }
                                }
                            });
                        }
                    }

                    MatrixCmd::JoinRoom { room_id, space_id } => {
                        if let Some(c) = client.clone() {
                            let evt_tx = evt_tx.clone();
                            tokio::spawn(async move {
                                let Ok(rid) = RoomId::parse(&room_id) else { return };
                                if let Err(err) = c.join_room_by_id(&rid).await {
                                    let _ = evt_tx.send(MatrixEvent::Error(format!("join room: {err}")));
                                    return;
                                }
                                // Refresh the directory so the row this join
                                // came from flips to "joined" immediately,
                                // rather than waiting on the next room-list
                                // sync to notice.
                                match fetch_hierarchy(&c, &space_id).await {
                                    Ok(rooms) => {
                                        let _ = evt_tx.send(MatrixEvent::SpaceHierarchy { space_id, rooms });
                                    }
                                    Err(err) => {
                                        let _ = evt_tx.send(MatrixEvent::Error(format!("lobby: {err}")));
                                    }
                                }
                            });
                        }
                    }

                    MatrixCmd::Logout => {
                        if let Some(active_sync) = sync.take() {
                            active_sync.stop().await;
                        }
                        client = None;
                        clear_saved_session();
                        let _ = evt_tx.send(MatrixEvent::LoggedOut);
                    }
                }
            }

            Some(result) = sso_rx.recv() => {
                match result {
                    Ok((new_client, user_id, hs)) => {
                        if let Some(prev) = sync.take() {
                            prev.stop().await;
                        }
                        match start_sync(new_client.clone(), evt_tx.clone()).await {
                            Ok(started) => {
                                sync = Some(started);
                                client = Some(new_client);
                                let _ = evt_tx.send(MatrixEvent::LoggedIn { user_id, homeserver: hs });
                            }
                            Err(err) => {
                                let _ = evt_tx.send(MatrixEvent::LoginFailed(err.to_string()));
                            }
                        }
                    }
                    Err(err) => {
                        let _ = evt_tx.send(MatrixEvent::LoginFailed(err.to_string()));
                    }
                }
            }
        }
    }
}

/// Starts push-based sliding sync for a freshly-logged-in (or restored)
/// `client`: the `SyncService` (joined-room list) plus the hand-rolled spaces
/// sync, both as detached background tasks forwarding snapshots over
/// `evt_tx`. Fails fast if the homeserver doesn't speak MSC4186 rather than
/// silently hanging.
async fn start_sync(client: Client, evt_tx: mpsc::UnboundedSender<MatrixEvent>) -> anyhow::Result<ActiveSync> {
    let versions = client.available_sliding_sync_versions().await;
    anyhow::ensure!(
        versions.iter().any(|v| matches!(v, Version::Native)),
        "homeserver does not support sliding sync (MSC4186)"
    );

    let service = SyncService::builder(client.clone()).build().await?;
    service.start().await;
    let service = Arc::new(service);

    let room_list_task = spawn_room_list_task(service.clone(), evt_tx.clone());
    let space_sync_task = spawn_space_sync(client.clone(), evt_tx.clone());

    Ok(ActiveSync {
        service,
        room_list_task,
        space_sync_task,
        active_room: None,
        timeline_task: None,
        active_timeline: None,
    })
}

/// Forwards `SyncService`'s room-list diff stream as flattened
/// `MatrixEvent::Rooms` snapshots. Uses the "non-left" filter as the closest
/// match to the old "all joined rooms" behavior — note `RoomListService`'s
/// underlying list hard-codes `not_room_types: [Space]`, which is exactly why
/// [`spawn_space_sync`] exists as a second, independent sync.
fn spawn_room_list_task(service: Arc<SyncService>, evt_tx: mpsc::UnboundedSender<MatrixEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let room_list = match service.room_list_service().all_rooms().await {
            Ok(rl) => rl,
            Err(err) => {
                let _ = evt_tx.send(MatrixEvent::Error(format!("room list: {err}")));
                return;
            }
        };
        let (stream, controller) = room_list.entries_with_dynamic_adapters(500);
        // The stream only starts yielding once a filter is set.
        controller.set_filter(Box::new(filters::new_filter_non_left()));

        pin_mut!(stream);
        let mut rooms: Vector<Room> = Vector::new();
        while let Some(diffs) = stream.next().await {
            for diff in diffs {
                diff.apply(&mut rooms);
            }
            let summaries = flatten_room_summaries(&rooms).await;
            let _ = evt_tx.send(MatrixEvent::Rooms(summaries));
        }
    })
}

/// Matrix Spaces don't come through `RoomListService` (it hard-codes
/// `not_room_types: [Space]` with no way to override it), so this drives a
/// second, hand-rolled `SlidingSync` instance — filtered to
/// `RoomTypeFilter::Space` — purely to make sure joined space rooms (and
/// their `m.space.child` state) land in the local store. Once they do,
/// `collect_space_children` reads them back out via the same
/// `client.rooms()`/`space_child_room_ids` path the old classic-sync code
/// used, so this task only needs to signal "state changed", not carry data
/// itself.
fn spawn_space_sync(client: Client, evt_tx: mpsc::UnboundedSender<MatrixEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let sliding_sync = match build_space_sliding_sync(&client).await {
            Ok(s) => s,
            Err(err) => {
                let _ = evt_tx.send(MatrixEvent::Error(format!("space sync: {err}")));
                return;
            }
        };

        let stream = sliding_sync.sync();
        pin_mut!(stream);
        while let Some(result) = stream.next().await {
            if let Err(err) = result {
                let _ = evt_tx.send(MatrixEvent::Error(format!("space sync: {err}")));
                continue;
            }
            let space_children = collect_space_children(&client).await;
            let _ = evt_tx.send(MatrixEvent::SpaceChildren(space_children));
        }
    })
}

async fn build_space_sliding_sync(client: &Client) -> anyhow::Result<matrix_sdk::sliding_sync::SlidingSync> {
    // `ListFilters` is `#[non_exhaustive]`, so it can't be built with struct-literal
    // syntax outside its crate — default it, then set the one field we need.
    let mut filters = ListFilters::default();
    filters.room_types = vec![RoomTypeFilter::Space];
    let list = SlidingSyncList::builder("spaces")
        .sync_mode(SlidingSyncMode::new_growing(50))
        .timeline_limit(0u32)
        .required_state(vec![
            (StateEventType::RoomName, "".to_owned()),
            (StateEventType::SpaceChild, "*".to_owned()),
        ])
        .filters(Some(filters));

    let sliding_sync =
        client.sliding_sync("neo-spaces")?.version(Version::Native).add_list(list).build().await?;
    Ok(sliding_sync)
}

async fn collect_space_children(client: &Client) -> HashMap<String, Vec<String>> {
    let mut space_children: HashMap<String, Vec<String>> = HashMap::new();
    for room in client.rooms() {
        if !room.is_space() {
            continue;
        }
        let children = space_child_room_ids(&room).await;
        if !children.is_empty() {
            space_children.insert(room.room_id().to_string(), children);
        }
    }
    space_children
}

/// Every room a space advertises via the server's `/hierarchy` endpoint —
/// joined and not — for that space's "Lobby" directory screen. Unlike
/// `space_child_room_ids` (which only reads locally-cached `m.space.child`
/// state for rooms already joined), this hits the server, so rooms the user
/// hasn't joined yet are discoverable too. Paginates until the server stops
/// returning a `next_batch` token, capped at a generous number of pages so a
/// huge space can't loop forever.
async fn fetch_hierarchy(client: &Client, space_id: &str) -> anyhow::Result<Vec<LobbyRoom>> {
    let root = RoomId::parse(space_id)?;
    let mut rooms = Vec::new();
    let mut from = None;
    for _ in 0..20 {
        let mut request = get_hierarchy::v1::Request::new(root.clone());
        request.from = from.clone();
        request.limit = Some(uint!(50));
        let response = client.send(request).await?;
        for chunk in response.rooms {
            if chunk.room_id == root {
                continue; // the space's own row — not a channel to list
            }
            let joined =
                client.get_room(&chunk.room_id).is_some_and(|r| r.state() == RoomState::Joined);
            rooms.push(LobbyRoom {
                id: chunk.room_id.to_string(),
                name: chunk.name.unwrap_or_else(|| chunk.room_id.to_string()),
                joined,
            });
        }
        from = response.next_batch;
        if from.is_none() {
            break;
        }
    }
    rooms.sort_by_key(|r| r.name.to_lowercase());
    Ok(rooms)
}

async fn flatten_room_summaries(rooms: &Vector<Room>) -> Vec<RoomSummary> {
    let mut out = Vec::with_capacity(rooms.len());
    for room in rooms.iter() {
        out.push(room_summary(room).await);
    }
    out.sort_by_key(|r| r.name.to_lowercase());
    out
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

fn flatten_chat_messages(items: &Vector<Arc<TimelineItem>>) -> Vec<ChatMessage> {
    items.iter().filter_map(|item| to_chat_message(item)).collect()
}

/// `None` for anything that isn't a rendered `m.room.message` (day dividers,
/// read markers, redactions, reactions, unsupported msgtypes are skipped
/// rather than shown as raw JSON) — same behavior as the old classic-sync
/// `message_body`, just sourced from a `TimelineItem` instead of a raw event.
fn to_chat_message(item: &TimelineItem) -> Option<ChatMessage> {
    let event = item.as_event()?;
    let msg = event.content().as_message()?;
    Some(ChatMessage {
        event_id: event.event_id().map(|id| id.to_string()),
        sender: event.sender().to_string(),
        body: message_body_text(msg.msgtype(), event.sender().localpart()),
        ts_millis: event.timestamp().0.into(),
        own: event.is_own(),
        pending: event.is_local_echo()
            && !matches!(event.send_state(), Some(matrix_sdk_ui::timeline::EventSendState::Sent { .. })),
    })
}

fn message_body_text(msgtype: &MessageType, sender_localpart: &str) -> String {
    match msgtype {
        MessageType::Text(t) => t.body.clone(),
        MessageType::Emote(t) => format!("* {sender_localpart} {}", t.body),
        MessageType::Notice(t) => t.body.clone(),
        MessageType::Image(t) => format!("🖼 {}", t.body),
        MessageType::File(t) => format!("📎 {}", t.body),
        MessageType::Audio(t) => format!("🔊 {}", t.body),
        MessageType::Video(t) => format!("🎬 {}", t.body),
        MessageType::Location(t) => format!("📍 {}", t.body),
        _ => "[unsupported message]".to_owned(),
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

/// Integration tests for the Lobby directory (`/hierarchy`) round-trip.
///
/// These drive a real `matrix_sdk::Client` against a `wiremock`-mocked
/// homeserver, so they exercise the actual HTTP request/response and ruma
/// (de)serialization — not just `fetch_hierarchy`'s in-memory logic. The
/// crate currently ships as a bin-only target with no `[lib]`, so there's
/// nothing for a `tests/` directory to link against; this lives next to the
/// code it covers instead.
#[cfg(test)]
mod tests {
    use super::*;
    use matrix_sdk::test_utils::logged_in_client_with_server;
    use serde_json::json;
    use wiremock::matchers::{method, path_regex, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn room_chunk(room_id: &str, name: &str) -> serde_json::Value {
        json!({
            "room_id": room_id,
            "name": name,
            "num_joined_members": 3,
            "world_readable": false,
            "guest_can_join": false,
            "children_state": [],
        })
    }

    /// Mounts a single-page `/hierarchy` response containing the space's own
    /// row (which `fetch_hierarchy` must filter out) plus the given children.
    async fn mock_hierarchy(server: &MockServer, space_id: &str, children: &[(&str, &str)]) {
        let mut rooms = vec![room_chunk(space_id, "The Space Itself")];
        rooms.extend(children.iter().map(|(id, name)| room_chunk(id, name)));

        Mock::given(method("GET"))
            .and(path_regex(r"rooms/[^/]+/hierarchy$"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "rooms": rooms })))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn fetch_hierarchy_excludes_the_space_itself_and_sorts_by_name() {
        let (client, server) = logged_in_client_with_server().await;
        let space_id = "!space:example.org";
        mock_hierarchy(
            &server,
            space_id,
            &[("!b:example.org", "Zebras"), ("!a:example.org", "Anteaters")],
        )
        .await;

        let rooms = fetch_hierarchy(&client, space_id).await.unwrap();

        let names: Vec<&str> = rooms.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["Anteaters", "Zebras"], "space's own row must be excluded");
        assert!(rooms.iter().all(|r| !r.joined), "nothing joined yet");
    }

    #[tokio::test]
    async fn fetch_hierarchy_paginates_until_next_batch_is_absent() {
        let (client, server) = logged_in_client_with_server().await;
        let space_id = "!space:example.org";

        Mock::given(method("GET"))
            .and(path_regex(r"rooms/[^/]+/hierarchy$"))
            .and(query_param_is_missing("from"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "next_batch": "page2",
                "rooms": [room_chunk("!a:example.org", "Anteaters")],
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path_regex(r"rooms/[^/]+/hierarchy$"))
            .and(query_param("from", "page2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "rooms": [room_chunk("!b:example.org", "Zebras")],
            })))
            .mount(&server)
            .await;

        let rooms = fetch_hierarchy(&client, space_id).await.unwrap();

        let names: Vec<&str> = rooms.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["Anteaters", "Zebras"], "both pages must be aggregated");
    }

    #[tokio::test]
    async fn fetch_hierarchy_reflects_a_locally_joined_room() {
        let (client, server) = logged_in_client_with_server().await;
        let space_id = "!space:example.org";
        let room_id = "!a:example.org";
        mock_hierarchy(&server, space_id, &[(room_id, "Anteaters")]).await;

        let before = fetch_hierarchy(&client, space_id).await.unwrap();
        assert!(!before[0].joined, "not joined until we actually join");

        Mock::given(method("POST"))
            .and(path_regex(r"rooms/[^/]+/join$"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "room_id": room_id })))
            .mount(&server)
            .await;
        client.join_room_by_id(&RoomId::parse(room_id).unwrap()).await.unwrap();

        let after = fetch_hierarchy(&client, space_id).await.unwrap();
        assert!(after[0].joined, "fetch_hierarchy should see the local join immediately");
    }
}
