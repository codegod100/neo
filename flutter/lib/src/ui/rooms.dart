import 'package:flatkit/flatkit.dart';
import 'package:flutter/material.dart';
import 'package:matrix/matrix.dart';

import '../media.dart';
import '../rooms.dart';
import 'common.dart';

/// The joined-room list, filtered by the search box and the space picked
/// in the rail.
class RoomsScreen extends StatefulWidget {
  final Client client;
  final Room? space;
  final String? selectedRoomId;
  final ValueChanged<Room> onOpenRoom;
  final VoidCallback onOpenLobby;
  final VoidCallback onOpenSettings;

  const RoomsScreen({
    super.key,
    required this.client,
    required this.space,
    required this.selectedRoomId,
    required this.onOpenRoom,
    required this.onOpenLobby,
    required this.onOpenSettings,
  });

  @override
  State<RoomsScreen> createState() => _RoomsScreenState();
}

class _RoomsScreenState extends State<RoomsScreen> {
  final _filter = TextEditingController();

  @override
  void dispose() {
    _filter.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final client = widget.client;
    final all = chatRooms(client);
    final rooms = filterRooms(all, query: _filter.text, space: widget.space);
    final loading = client.prevBatch == null;

    final String? empty;
    if (rooms.isNotEmpty) {
      empty = null;
    } else if (loading) {
      empty = null;
    } else if (all.isNotEmpty) {
      empty = 'No rooms match your search.';
    } else {
      empty = 'No rooms yet — joined rooms show up here.';
    }

    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      ScreenHeader(
        title: widget.space == null ? 'Home' : roomName(widget.space!),
        subtitle: client.userID,
        actions: [
          KitIconButton(Icons.settings_outlined, onPressed: widget.onOpenSettings, tooltip: 'Settings'),
        ],
      ),
      Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
        child: KitTextField(
          controller: _filter,
          hint: 'Search rooms…',
          prefixIcon: Icons.search,
          onChanged: (_) => setState(() {}),
        ),
      ),
      if (widget.space != null)
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 4, 12, 0),
          child: Align(
            alignment: Alignment.centerLeft,
            child: KitButton.ghost('Lobby', icon: Icons.explore_outlined, onPressed: widget.onOpenLobby),
          ),
        ),
      const SizedBox(height: 4),
      Expanded(
        child: rooms.isEmpty
            ? (empty == null ? const LoadingState('Loading rooms…') : EmptyState(empty))
            : ListView.builder(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
                itemCount: rooms.length,
                itemBuilder: (context, i) => _RoomTile(
                  room: rooms[i],
                  selected: rooms[i].id == widget.selectedRoomId,
                  onTap: () => widget.onOpenRoom(rooms[i]),
                ),
              ),
      ),
    ]);
  }
}

class _RoomTile extends StatelessWidget {
  final Room room;
  final bool selected;
  final VoidCallback onTap;

  const _RoomTile({required this.room, required this.selected, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final c = KitTheme.of(context);
    final strong = selected;
    final name = roomName(room);
    final preview = roomPreview(room);
    final unread = room.notificationCount;
    return KitListRow(
      selected: selected,
      height: 52,
      padding: const EdgeInsets.symmetric(horizontal: 8),
      onTap: onTap,
      child: Row(children: [
        Avatar(client: room.client, mxc: room.avatar, name: name, size: 34),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(children: [
                Flexible(
                  child: Text(name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: KitText.heading(context).copyWith(color: strong ? c.onAccent : null)),
                ),
                if (room.encrypted) ...[
                  const SizedBox(width: 4),
                  Icon(Icons.lock_outline, size: 12, color: strong ? c.onAccent : c.textDim),
                ],
              ]),
              if (preview.isNotEmpty)
                Text(preview,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: KitText.small(context, color: strong ? c.onAccent : c.textDim)),
            ],
          ),
        ),
        if (unread > 0)
          Container(
            margin: const EdgeInsets.only(left: 8),
            padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
            decoration: BoxDecoration(
              color: strong ? c.onAccent : c.accent,
              borderRadius: BorderRadius.circular(999),
            ),
            child: Text('$unread',
                style: KitText.small(context, color: strong ? c.accent : c.onAccent)
                    .copyWith(fontWeight: FontWeight.w600)),
          ),
      ]),
    );
  }
}

/// The space rail: "Home" and one chip per joined space.
class SpaceRail extends StatelessWidget {
  final Client client;
  final List<Room> spaces;
  final String? activeSpaceId;
  final ValueChanged<String?> onSelect;

  const SpaceRail({
    super.key,
    required this.client,
    required this.spaces,
    required this.activeSpaceId,
    required this.onSelect,
  });

  @override
  Widget build(BuildContext context) {
    final c = KitTheme.of(context);
    return Container(
      width: 64,
      decoration: BoxDecoration(border: Border(right: BorderSide(color: c.border))),
      child: ListView(
        padding: const EdgeInsets.symmetric(vertical: 8),
        children: [
          _chip(context, null, 'Home', const Icon(Icons.home_outlined, size: 20)),
          for (final space in spaces)
            _chip(
              context,
              space.id,
              roomName(space),
              Avatar(client: client, mxc: space.avatar, name: roomName(space), size: 36, radius: 10),
            ),
        ],
      ),
    );
  }

  Widget _chip(BuildContext context, String? id, String label, Widget icon) {
    final c = KitTheme.of(context);
    final selected = id == activeSpaceId;
    return KitTooltip(
      message: label,
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Pressable(
          onPressed: () => onSelect(id),
          builder: (context, state) => Center(
            child: Container(
              width: 46,
              height: 46,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                color: state.hovered ? c.hover : null,
                borderRadius: BorderRadius.circular(12),
                border: Border.all(color: selected ? c.accent : const Color(0x00000000), width: 2),
              ),
              child: icon,
            ),
          ),
        ),
      ),
    );
  }
}
