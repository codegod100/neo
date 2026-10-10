/// Room list, space and lobby logic, kept free of widgets so it can be
/// tested on its own.
library;

import 'package:matrix/matrix.dart';

/// Joined rooms that carry messages (not spaces), newest activity first, as
/// the client already sorts them.
List<Room> chatRooms(Client client) => client.rooms
    .where((r) => r.membership == Membership.join && !r.isSpace)
    .toList();

/// Joined spaces, alphabetically.
List<Room> joinedSpaces(Client client) {
  final spaces = client.rooms
      .where((r) => r.membership == Membership.join && r.isSpace)
      .toList();
  spaces.sort((a, b) => roomName(a).toLowerCase().compareTo(roomName(b).toLowerCase()));
  return spaces;
}

String roomName(Room room) => room.getLocalizedDisplayname();

/// The room's alias when it publishes one (`#developers:matrix.org`),
/// otherwise its ID.
String roomAddress(Room room) =>
    room.canonicalAlias.isNotEmpty ? room.canonicalAlias : room.id;

String roomPreview(Room room) {
  final event = room.lastEvent;
  if (event == null) return '';
  return event.calcLocalizedBodyFallback(
    const MatrixDefaultLocalizations(),
    hideReply: true,
    withSenderNamePrefix: !room.isDirectChat,
  );
}

/// Rooms matching the search box and, when a space is picked, that space's
/// own `m.space.child` list (one level deep, local state only).
List<Room> filterRooms(List<Room> rooms, {String query = '', Room? space}) {
  final needle = query.trim().toLowerCase();
  final members = space?.spaceChildren.map((c) => c.roomId).whereType<String>().toSet();
  return rooms
      .where((r) => needle.isEmpty || roomName(r).toLowerCase().contains(needle))
      .where((r) => members == null || members.contains(r.id))
      .toList();
}

/// One room in a space's directory, joined or not.
class LobbyRoom {
  final String id;
  final String name;
  final String? topic;
  final Uri? avatar;
  final int members;
  final bool joined;

  /// Servers the space named as able to route a join (`via`).
  final List<String> via;

  const LobbyRoom({
    required this.id,
    required this.name,
    this.topic,
    this.avatar,
    this.members = 0,
    required this.joined,
    this.via = const [],
  });
}

typedef HierarchyPage = Future<GetSpaceHierarchyResponse> Function(String? from);

/// Every room a space's `/hierarchy` lists one level down, following
/// `next_batch` to the end, without the space itself, sorted by name.
Future<List<LobbyRoom>> fetchLobby(
  String spaceId,
  HierarchyPage page, {
  bool Function(String roomId)? isJoined,
}) async {
  final chunks = <SpaceRoomsChunk$2>[];
  String? from;
  do {
    final response = await page(from);
    chunks.addAll(response.rooms);
    from = response.nextBatch;
  } while (from != null);

  final via = <String, List<String>>{};
  for (final chunk in chunks.where((c) => c.roomId == spaceId)) {
    for (final child in chunk.childrenState) {
      if (child.type != EventTypes.SpaceChild) continue;
      final servers = child.content.tryGetList<String>('via');
      final key = child.stateKey;
      if (servers != null && key != null) via[key] = servers;
    }
  }

  final rooms = [
    for (final c in chunks)
      if (c.roomId != spaceId)
        LobbyRoom(
          id: c.roomId,
          name: c.name ?? c.canonicalAlias ?? c.roomId,
          topic: c.topic,
          avatar: c.avatarUrl,
          members: c.numJoinedMembers,
          joined: isJoined?.call(c.roomId) ?? false,
          via: via[c.roomId] ?? const [],
        ),
  ];
  rooms.sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
  return rooms;
}

Future<List<LobbyRoom>> fetchClientLobby(Client client, String spaceId) => fetchLobby(
      spaceId,
      (from) => client.getSpaceHierarchy(spaceId, maxDepth: 1, from: from),
      isJoined: (id) => client.getRoomById(id)?.membership == Membership.join,
    );
