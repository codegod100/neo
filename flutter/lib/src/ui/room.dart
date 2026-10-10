import 'package:flatkit/flatkit.dart';
import 'package:flutter/material.dart';
import 'package:matrix/matrix.dart';

import '../media.dart';
import '../rooms.dart';
import '../session.dart';
import '../timeline.dart' as tl;
import 'common.dart';

/// One room: its live timeline, a search box over the loaded messages, and
/// the composer.
class RoomScreen extends StatefulWidget {
  final Room room;
  final VoidCallback? onBack;

  const RoomScreen({super.key, required this.room, this.onBack});

  @override
  State<RoomScreen> createState() => _RoomScreenState();
}

class _RoomScreenState extends State<RoomScreen> {
  Timeline? _timeline;
  final _scroll = ScrollController();
  final _filter = TextEditingController();
  final _compose = TextEditingController();
  final _composeFocus = FocusNode();
  bool _loadingOlder = false;
  bool _awayFromPresent = false;

  @override
  void initState() {
    super.initState();
    _scroll.addListener(_onScroll);
    _open();
  }

  @override
  void didUpdateWidget(RoomScreen old) {
    super.didUpdateWidget(old);
    if (old.room.id != widget.room.id) {
      _timeline?.cancelSubscriptions();
      _timeline = null;
      _filter.clear();
      _awayFromPresent = false;
      if (_scroll.hasClients) _scroll.jumpTo(0);
      _open();
    }
  }

  @override
  void dispose() {
    _timeline?.cancelSubscriptions();
    _scroll.dispose();
    _filter.dispose();
    _compose.dispose();
    _composeFocus.dispose();
    super.dispose();
  }

  Future<void> _open() async {
    final room = widget.room;
    final timeline = await room.getTimeline(onUpdate: () {
      if (mounted) setState(() {});
    });
    if (!mounted || room.id != widget.room.id) {
      timeline.cancelSubscriptions();
      return;
    }
    setState(() => _timeline = timeline);
    timeline.setReadMarker().catchError((_) {});
    // A short room may not fill the screen, so there is no scroll to
    // trigger loading more; fetch a page straight away.
    if (timeline.events.where((e) => tl.shownTypes.contains(e.type)).length < 20) {
      _loadOlder();
    }
  }

  void _onScroll() {
    if (!_scroll.hasClients) return;
    final pos = _scroll.position;
    final away = pos.pixels > 400;
    if (away != _awayFromPresent) setState(() => _awayFromPresent = away);
    // The list is reversed: older history is at the far end.
    if (pos.pixels > pos.maxScrollExtent - 300) _loadOlder();
  }

  Future<void> _loadOlder() async {
    final timeline = _timeline;
    if (timeline == null || _loadingOlder || !timeline.canRequestHistory) return;
    setState(() => _loadingOlder = true);
    try {
      await timeline.requestHistory();
    } catch (e) {
      if (mounted) showKitToast(context, 'Couldn’t load older messages: ${describeError(e)}');
    } finally {
      if (mounted) setState(() => _loadingOlder = false);
    }
  }

  Future<void> _send() async {
    final text = _compose.text.trim();
    if (text.isEmpty) return;
    _compose.clear();
    _composeFocus.requestFocus();
    if (_scroll.hasClients) _scroll.jumpTo(0);
    try {
      await widget.room.sendTextEvent(text);
    } catch (e) {
      if (mounted) showKitToast(context, 'Send failed: ${describeError(e)}');
    }
  }

  @override
  Widget build(BuildContext context) {
    final room = widget.room;
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      ScreenHeader(title: roomName(room), subtitle: roomAddress(room), onBack: widget.onBack),
      Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
        child: KitTextField(
          controller: _filter,
          hint: 'Search loaded messages…',
          prefixIcon: Icons.search,
          onChanged: (_) => setState(() {}),
        ),
      ),
      Expanded(child: _messages(context)),
      Padding(
        padding: const EdgeInsets.fromLTRB(12, 4, 12, 12),
        child: Row(children: [
          Expanded(
            child: KitTextField(
              controller: _compose,
              focusNode: _composeFocus,
              hint: 'Message…',
              onSubmitted: (_) => _send(),
            ),
          ),
          const SizedBox(width: 8),
          KitButton.primary('Send', onPressed: _send),
        ]),
      ),
    ]);
  }

  Widget _messages(BuildContext context) {
    final timeline = _timeline;
    if (timeline == null) return const LoadingState('Loading messages…');
    final rows = tl.buildRows(timeline.events, query: _filter.text);
    if (rows.isEmpty && _filter.text.trim().isNotEmpty) {
      return const EmptyState('No loaded messages match your search.');
    }
    if (rows.isEmpty && !_loadingOlder) {
      return const EmptyState('No messages yet.');
    }
    final ownId = widget.room.client.userID;
    return Stack(children: [
      ListView.builder(
        controller: _scroll,
        reverse: true,
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        itemCount: rows.length + 1,
        itemBuilder: (context, i) {
          if (i == rows.length) {
            return Padding(
              padding: const EdgeInsets.all(12),
              child: Center(
                child: _loadingOlder
                    ? const KitSpinner()
                    : timeline.canRequestHistory
                        ? KitButton.ghost('Load older messages', onPressed: _loadOlder)
                        : Text('Start of the room', style: KitText.dim(context, small: true)),
              ),
            );
          }
          return switch (rows[i]) {
            tl.DayRow(:final day) => _DaySeparator(day),
            tl.MessageRow(:final event) => MessageBubble(
                event: event,
                own: event.senderId == ownId,
                // Group consecutive messages from one sender under one name.
                showSender: _startsGroup(rows, i),
              ),
          };
        },
      ),
      if (_awayFromPresent)
        Positioned(
          bottom: 12,
          left: 0,
          right: 0,
          child: Center(
            child: KitButton.primary('Jump to present', icon: Icons.arrow_downward, onPressed: () {
              _scroll.jumpTo(0);
            }),
          ),
        ),
    ]);
  }

  /// Rows are newest first, so the message above row [i] on screen is
  /// row i + 1.
  static bool _startsGroup(List<tl.TimelineRow> rows, int i) {
    final row = rows[i] as tl.MessageRow;
    if (i + 1 >= rows.length) return true;
    final above = rows[i + 1];
    if (above is! tl.MessageRow) return true;
    if (above.event.senderId != row.event.senderId) return true;
    return row.event.originServerTs.difference(above.event.originServerTs).inMinutes > 5;
  }
}

class _DaySeparator extends StatelessWidget {
  final DateTime day;
  const _DaySeparator(this.day);

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 16, bottom: 6),
      child: Row(children: [
        const Expanded(child: KitDivider()),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10),
          child: Text(tl.dayLabel(day),
              style: KitText.dim(context, small: true).copyWith(fontWeight: FontWeight.w600)),
        ),
        const Expanded(child: KitDivider()),
      ]),
    );
  }
}

/// A chat bubble: own messages on the right in the accent colour, everyone
/// else's on the left under their avatar and name.
class MessageBubble extends StatelessWidget {
  final Event event;
  final bool own;
  final bool showSender;

  const MessageBubble({super.key, required this.event, required this.own, required this.showSender});

  @override
  Widget build(BuildContext context) {
    final c = KitTheme.of(context);
    final sender = event.senderFromMemoryOrFallback;
    final name = tl.senderName(event);
    final failed = event.status == EventStatus.error;
    final pending = !event.status.isSent && !failed;
    final fg = own ? c.onAccent : c.text;

    final bubble = Container(
      constraints: const BoxConstraints(maxWidth: 520),
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: own ? c.accent : c.surface,
        borderRadius: BorderRadius.circular(10),
        border: own ? null : Border.all(color: c.border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          if (hasPreview(event)) ...[
            _MediaPreview(event: event),
            if (event.messageType != MessageTypes.Image && event.type != EventTypes.Sticker)
              const SizedBox(height: 4),
          ],
          if (!hasPreview(event) || event.messageType == MessageTypes.Video)
            SelectableText(
              tl.messageBody(event),
              style: KitText.body(context).copyWith(
                color: fg,
                fontStyle: event.type == EventTypes.Encrypted ? FontStyle.italic : null,
              ),
            ),
          const SizedBox(height: 2),
          Text(
            failed ? 'Failed to send' : (pending ? 'Sending…' : tl.timeLabel(event.originServerTs)),
            style: KitText.small(context, color: failed ? c.danger : (own ? c.onAccent : c.textDim))
                .copyWith(fontSize: 11),
          ),
        ],
      ),
    );

    return Opacity(
      opacity: pending ? 0.6 : 1,
      child: Padding(
        padding: EdgeInsets.only(top: showSender ? 8 : 2),
        child: Row(
          mainAxisAlignment: own ? MainAxisAlignment.end : MainAxisAlignment.start,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            if (!own)
              SizedBox(
                width: 40,
                child: showSender
                    ? Align(
                        alignment: Alignment.topLeft,
                        child: Avatar(client: event.room.client, mxc: sender.avatarUrl, name: name, size: 30),
                      )
                    : null,
              ),
            Flexible(
              child: Column(
                crossAxisAlignment: own ? CrossAxisAlignment.end : CrossAxisAlignment.start,
                children: [
                  if (showSender && !own)
                    Padding(
                      padding: const EdgeInsets.only(left: 2, bottom: 2),
                      child: Text(name, style: KitText.small(context, color: c.textDim)
                          .copyWith(fontWeight: FontWeight.w600)),
                    ),
                  bubble,
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _MediaPreview extends StatelessWidget {
  final Event event;
  const _MediaPreview({required this.event});

  @override
  Widget build(BuildContext context) {
    final c = KitTheme.of(context);
    return FutureBuilder(
      future: attachmentPreview(event),
      builder: (context, snap) {
        final bytes = snap.data;
        if (bytes == null) {
          return Container(
            width: 240,
            height: 140,
            alignment: Alignment.center,
            decoration: BoxDecoration(color: c.hover, borderRadius: BorderRadius.circular(6)),
            child: snap.connectionState == ConnectionState.done
                ? Text(tl.messageBody(event), style: KitText.dim(context, small: true))
                : const KitSpinner(),
          );
        }
        return ClipRRect(
          borderRadius: BorderRadius.circular(6),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 360, maxHeight: 320),
            child: Image.memory(bytes, fit: BoxFit.contain, gaplessPlayback: true),
          ),
        );
      },
    );
  }
}
