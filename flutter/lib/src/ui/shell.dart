import 'package:flatkit/flatkit.dart';
import 'package:flutter/material.dart';
import 'package:matrix/matrix.dart';

import '../prefs.dart';
import '../rooms.dart';
import '../session.dart';
import 'connect.dart';
import 'lobby.dart';
import 'room.dart';
import 'rooms.dart';
import 'settings.dart';

enum Screen { rooms, room, lobby, settings }

/// Above this width the room list stays on the left while a room, the
/// lobby or settings fill the rest; below it they replace each other.
const wideLayout = 760.0;

class NeoApp extends StatelessWidget {
  final NeoSession session;
  final Prefs prefs;

  const NeoApp({super.key, required this.session, required this.prefs});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: prefs,
      builder: (context, _) => KitSystemTheme(
        builder: (context, system) {
          final colors = switch (prefs.theme) {
            ThemeChoice.system => system,
            ThemeChoice.light => KitColors.light,
            ThemeChoice.dark => KitColors.dark,
          };
          return MaterialApp(
            title: 'neo',
            debugShowCheckedModeBanner: false,
            theme: KitTheme.material(colors),
            builder: (context, child) => KitTheme(
              colors: colors,
              child: Material(
                color: colors.background,
                child: DefaultTextStyle(
                  style: kitTextStyle.copyWith(color: colors.text),
                  child: child!,
                ),
              ),
            ),
            home: Shell(session: session, prefs: prefs),
          );
        },
      ),
    );
  }
}

class Shell extends StatefulWidget {
  final NeoSession session;
  final Prefs prefs;

  const Shell({super.key, required this.session, required this.prefs});

  @override
  State<Shell> createState() => _ShellState();
}

class _ShellState extends State<Shell> {
  Screen _screen = Screen.rooms;
  String? _spaceId;
  String? _roomId;

  NeoSession get _session => widget.session;

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ListenableBuilder(
        listenable: _session,
        builder: (context, _) {
          final client = _session.client;
          if (client == null || !_session.loggedIn) {
            _screen = Screen.rooms;
            _spaceId = null;
            _roomId = null;
            return ConnectScreen(session: _session, prefs: widget.prefs);
          }
          return _signedIn(context, client);
        },
      ),
    );
  }

  Widget _signedIn(BuildContext context, Client client) {
    final spaces = joinedSpaces(client);
    final space = spaces.where((s) => s.id == _spaceId).firstOrNull;
    if (space == null) _spaceId = null;
    final room = _roomId == null ? null : client.getRoomById(_roomId!);
    if (_screen == Screen.room && room == null) _screen = Screen.rooms;
    if (_screen == Screen.lobby && space == null) _screen = Screen.rooms;

    return LayoutBuilder(builder: (context, constraints) {
      final wide = constraints.maxWidth >= wideLayout;
      final list = RoomsScreen(
        client: client,
        space: space,
        selectedRoomId: wide && _screen == Screen.room ? _roomId : null,
        onOpenRoom: (r) => setState(() {
          _roomId = r.id;
          _screen = Screen.room;
        }),
        onOpenLobby: () => setState(() => _screen = Screen.lobby),
        onOpenSettings: () => setState(() => _screen = Screen.settings),
      );
      void back() => setState(() => _screen = Screen.rooms);

      final Widget? detail = switch (_screen) {
        Screen.rooms => null,
        Screen.room => RoomScreen(room: room!, onBack: wide ? null : back),
        Screen.lobby => LobbyScreen(
            client: client,
            space: space!,
            onBack: back,
            onOpenRoom: (id) => setState(() {
              _roomId = id;
              _screen = Screen.room;
            }),
          ),
        Screen.settings => SettingsScreen(
            client: client,
            prefs: widget.prefs,
            onBack: back,
            onSignOut: _session.logout,
          ),
      };

      final c = KitTheme.of(context);
      final Widget content;
      if (wide) {
        content = Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
          SizedBox(width: 320, child: list),
          Container(width: 1, color: c.border),
          Expanded(
            child: detail ?? const _Placeholder(),
          ),
        ]);
      } else {
        content = detail ?? list;
      }

      return Row(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
        if (spaces.isNotEmpty && (wide || detail == null))
          SpaceRail(
            client: client,
            spaces: spaces,
            activeSpaceId: _spaceId,
            onSelect: (id) => setState(() {
              _spaceId = id;
              if (_screen == Screen.lobby) _screen = Screen.rooms;
            }),
          ),
        Expanded(child: content),
      ]);
    });
  }
}

class _Placeholder extends StatelessWidget {
  const _Placeholder();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Text('Pick a room to start chatting.', style: KitText.dim(context)),
    );
  }
}
