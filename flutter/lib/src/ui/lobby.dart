import 'package:flatkit/flatkit.dart';
import 'package:flutter/material.dart';
import 'package:matrix/matrix.dart';

import '../media.dart';
import '../rooms.dart';
import '../session.dart';
import 'common.dart';

/// A space's room directory from the server's `/hierarchy`, including rooms
/// not joined yet, which can be joined from here.
class LobbyScreen extends StatefulWidget {
  final Client client;
  final Room space;
  final ValueChanged<String> onOpenRoom;
  final VoidCallback onBack;

  const LobbyScreen({
    super.key,
    required this.client,
    required this.space,
    required this.onOpenRoom,
    required this.onBack,
  });

  @override
  State<LobbyScreen> createState() => _LobbyScreenState();
}

class _LobbyScreenState extends State<LobbyScreen> {
  List<LobbyRoom>? _rooms;
  String? _error;
  final _joining = <String>{};

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void didUpdateWidget(LobbyScreen old) {
    super.didUpdateWidget(old);
    if (old.space.id != widget.space.id) {
      _rooms = null;
      _load();
    }
  }

  Future<void> _load() async {
    final spaceId = widget.space.id;
    try {
      final rooms = await fetchClientLobby(widget.client, spaceId);
      if (!mounted || widget.space.id != spaceId) return;
      setState(() {
        _rooms = rooms;
        _error = null;
      });
    } catch (e) {
      if (!mounted || widget.space.id != spaceId) return;
      setState(() => _error = describeError(e));
    }
  }

  Future<void> _join(LobbyRoom room) async {
    setState(() => _joining.add(room.id));
    try {
      await widget.client.joinRoom(room.id, via: room.via.isEmpty ? null : room.via);
      // Wait for the joined room to arrive in a sync so it can be opened.
      await widget.client.waitForRoomInSync(room.id, join: true);
      await _load();
    } catch (e) {
      if (mounted) showKitToast(context, 'Couldn’t join ${room.name}: ${describeError(e)}');
    } finally {
      if (mounted) setState(() => _joining.remove(room.id));
    }
  }

  @override
  Widget build(BuildContext context) {
    final rooms = _rooms;
    final Widget body;
    if (rooms == null && _error != null) {
      body = EmptyState(_error!);
    } else if (rooms == null) {
      body = const LoadingState('Loading directory…');
    } else if (rooms.isEmpty) {
      body = const EmptyState('No rooms in this space’s directory.');
    } else {
      body = ListView.separated(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
        itemCount: rooms.length,
        separatorBuilder: (_, _) => const KitDivider(),
        itemBuilder: (context, i) => _row(context, rooms[i]),
      );
    }
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      ScreenHeader(title: 'Lobby', subtitle: roomName(widget.space), onBack: widget.onBack),
      Expanded(child: body),
    ]);
  }

  Widget _row(BuildContext context, LobbyRoom room) {
    final members = room.members == 1 ? '1 member' : '${room.members} members';
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Row(children: [
        Avatar(client: widget.client, mxc: room.avatar, name: room.name, size: 32),
        const SizedBox(width: 10),
        Expanded(
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(room.name, maxLines: 1, overflow: TextOverflow.ellipsis, style: KitText.heading(context)),
            Text(
              room.topic == null || room.topic!.isEmpty ? members : '$members · ${room.topic}',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: KitText.dim(context, small: true),
            ),
          ]),
        ),
        const SizedBox(width: 8),
        if (room.joined)
          KitButton('Open', onPressed: () => widget.onOpenRoom(room.id))
        else if (_joining.contains(room.id))
          const Padding(padding: EdgeInsets.symmetric(horizontal: 18), child: KitSpinner())
        else
          KitButton.primary('Join', onPressed: () => _join(room)),
      ]),
    );
  }
}
