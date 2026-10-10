import 'package:flutter_test/flutter_test.dart';
import 'package:matrix/matrix.dart';
import 'package:neo/src/rooms.dart';
import 'package:neo/src/session.dart';
import 'package:neo/src/sso.dart';
import 'package:neo/src/timeline.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';

Map<String, Object?> chunk(String id, String name, {List<Map<String, Object?>> children = const []}) => {
      'room_id': id,
      'name': name,
      'num_joined_members': 3,
      'world_readable': false,
      'guest_can_join': false,
      'children_state': children,
    };

Map<String, Object?> child(String id, List<String> via) => {
      'type': 'm.space.child',
      'state_key': id,
      'sender': '@admin:example.org',
      'origin_server_ts': 0,
      'content': {'via': via},
    };

Future<Client> testClient() async {
  sqfliteFfiInit();
  final db = await databaseFactoryFfi.openDatabase(inMemoryDatabasePath);
  return Client(
    'neo-test',
    database: await MatrixSdkDatabase.init('neo-test', database: db, sqfliteFactory: databaseFactoryFfi),
  );
}

Event message(Room room, String id, String sender, String body, DateTime ts,
        {String type = EventTypes.Message}) =>
    Event(
      eventId: id,
      senderId: sender,
      type: type,
      content: {'msgtype': 'm.text', 'body': body},
      originServerTs: ts,
      room: room,
    );

void main() {
  group('lobby', () {
    test('lists every page, leaves out the space and sorts by name', () async {
      final pages = {
        null: GetSpaceHierarchyResponse.fromJson({
          'rooms': [
            chunk('!space:x', 'Space', children: [child('!b:x', ['x', 'y'])]),
            chunk('!b:x', 'beta'),
          ],
          'next_batch': 'p2',
        }),
        'p2': GetSpaceHierarchyResponse.fromJson({
          'rooms': [chunk('!a:x', 'Alpha')],
        }),
      };
      final rooms = await fetchLobby(
        '!space:x',
        (from) async => pages[from]!,
        isJoined: (id) => id == '!a:x',
      );
      expect(rooms.map((r) => r.name), ['Alpha', 'beta']);
      expect(rooms.first.joined, isTrue);
      expect(rooms.last.joined, isFalse);
      expect(rooms.last.via, ['x', 'y']);
      expect(rooms.last.members, 3);
    });
  });

  group('timeline rows', () {
    late Client client;
    late Room room;

    setUp(() async {
      client = await testClient();
      room = Room(id: '!r:x', client: client);
    });

    // The client is left open: looking up a sender fetches their profile
    // in the background, which would fail against a closed database.
    test('puts a day separator above each day’s oldest message', () {
      final day1 = DateTime(2026, 10, 8, 9);
      final day2 = DateTime(2026, 10, 9, 9);
      // Newest first, as Timeline.events keeps them.
      final rows = buildRows([
        message(room, '\$3', '@a:x', 'three', day2.add(const Duration(minutes: 1))),
        message(room, '\$2', '@b:x', 'two', day2),
        message(room, '\$1', '@a:x', 'one', day1),
      ]);
      expect(rows.map((r) => switch (r) {
            MessageRow(:final event) => event.body,
            DayRow(:final day) => day.day.toString(),
          }), ['three', 'two', '9', 'one', '8']);
    });

    test('hides non-message events and filters by search', () {
      final t = DateTime(2026, 10, 9, 9);
      final events = [
        message(room, '\$2', '@bob:x', 'hello there', t),
        message(room, '\$1', '@a:x', 'joined', t, type: EventTypes.RoomMember),
        message(room, '\$0', '@carol:x', 'goodbye', t),
      ];
      expect(buildRows(events).whereType<MessageRow>().length, 2);
      expect(buildRows(events, query: 'HELLO').whereType<MessageRow>().single.event.eventId, '\$2');
      expect(buildRows(events, query: 'carol').whereType<MessageRow>().single.event.eventId, '\$0');
      expect(buildRows(events, query: 'nothing'), isEmpty);
    });
  });

  test('day labels', () {
    final now = DateTime(2026, 8, 21, 12);
    expect(dayLabel(DateTime(2026, 8, 21), now: now), 'Today');
    expect(dayLabel(DateTime(2026, 8, 20), now: now), 'Yesterday');
    expect(dayLabel(DateTime(2026, 8, 19), now: now), 'Wednesday, August 19, 2026');
  });

  test('homeserver input defaults to https', () {
    expect(homeserverUri('matrix.org'), Uri.parse('https://matrix.org'));
    expect(homeserverUri(' '), Uri.parse('https://matrix.org'));
    expect(homeserverUri('http://localhost:8008'), Uri.parse('http://localhost:8008'));
  });

  test('SSO redirect URL carries the local callback', () {
    final url = ssoRedirectUrl(Uri.parse('https://matrix.org'), Uri.parse('http://localhost:1234/'));
    expect(url.path, '/_matrix/client/v3/login/sso/redirect');
    expect(url.queryParameters['redirectUrl'], 'http://localhost:1234/');
  });
}
