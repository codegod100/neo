/// The Matrix side of neo: one [Client] per signed-in account, plus the
/// sign-in, sign-out and session-restore flows around it.
///
/// A remembered session lives in a sqlite database under the app support
/// directory, so the next launch restores it (and its end-to-end encryption
/// keys) instead of logging in as a new device. Without "remember me" the
/// client gets an in-memory database and is forgotten when neo quits.
library;

import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:matrix/matrix.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';
import 'package:sqflite_common_ffi/sqflite_ffi.dart';

import 'sso.dart';

enum SessionState { disconnected, connecting, connected }

extension SessionStateLabel on SessionState {
  String get label => switch (this) {
        SessionState.disconnected => 'Disconnected',
        SessionState.connecting => 'Connecting…',
        SessionState.connected => 'Connected',
      };
}

const _clientName = 'neo';
const _rememberedDb = 'neo.sqlite';

class NeoSession extends ChangeNotifier {
  Client? _client;
  SessionState _state = SessionState.disconnected;
  String? _loginError;
  SsoLogin? _sso;
  StreamSubscription<SyncUpdate>? _syncSub;
  StreamSubscription<LoginState>? _loginSub;

  Client? get client => _client;
  SessionState get state => _state;
  String? get loginError => _loginError;

  /// The browser sign-in in progress, if any. Its [SsoLogin.url] is shown on
  /// the connect screen in case the browser didn't open on its own.
  SsoLogin? get sso => _sso;

  bool get loggedIn => _client?.isLogged() ?? false;

  /// Restores the remembered session, if there is one. Called once at
  /// startup; leaves neo on the connect screen otherwise.
  Future<void> restore() async {
    final path = await _rememberedPath();
    if (!File(path).existsSync()) return;
    _set(SessionState.connecting);
    try {
      final client = await _openClient(remember: true);
      await client.init(waitForFirstSync: false, waitUntilLoadCompletedLoaded: true);
      if (client.isLogged()) {
        _attach(client);
        _set(SessionState.connected);
        return;
      }
      await client.dispose();
    } catch (e) {
      _loginError = 'Couldn’t restore your session: $e';
    }
    _set(SessionState.disconnected);
  }

  Future<void> loginWithPassword({
    required String homeserver,
    required String username,
    required String password,
    required bool remember,
  }) =>
      _login(homeserver, remember, (client) {
        return client.login(
          LoginType.mLoginPassword,
          identifier: AuthenticationUserIdentifier(user: username.trim()),
          password: password,
          initialDeviceDisplayName: _clientName,
        );
      });

  /// Signs in through the homeserver's SSO page in the browser. The browser
  /// redirects back to a short-lived localhost server carrying a login token
  /// that is then exchanged for a session.
  Future<void> loginWithSso({required String homeserver, required bool remember}) =>
      _login(homeserver, remember, (client) async {
        final sso = await SsoLogin.start(client.homeserver!);
        _sso = sso;
        notifyListeners();
        try {
          final token = await sso.token;
          return await client.login(
            LoginType.mLoginToken,
            token: token,
            initialDeviceDisplayName: _clientName,
          );
        } finally {
          _sso = null;
        }
      });

  /// Stops waiting for the browser. The homeserver can't be told, so
  /// finishing the browser flow afterwards does nothing.
  void cancelSso() {
    _sso?.cancel();
  }

  Future<void> _login(
    String homeserver,
    bool remember,
    Future<LoginResponse> Function(Client client) login,
  ) async {
    _loginError = null;
    _set(SessionState.connecting);
    Client? client;
    try {
      if (remember) await _deleteRemembered();
      client = await _openClient(remember: remember);
      await client.init(waitForFirstSync: false, waitUntilLoadCompletedLoaded: false);
      await client.checkHomeserver(homeserverUri(homeserver));
      await login(client);
      _attach(client);
      _set(SessionState.connected);
    } catch (e) {
      await client?.dispose();
      if (remember) await _deleteRemembered();
      _loginError = e is SsoCancelled ? null : describeError(e);
      _set(SessionState.disconnected);
    }
  }

  Future<void> logout() async {
    final client = _client;
    if (client == null) return;
    _detach();
    try {
      await client.logout();
    } catch (_) {
      // Signing out locally still matters when the server can't be reached.
    }
    await client.dispose();
    await _deleteRemembered();
    _set(SessionState.disconnected);
  }

  void _attach(Client client) {
    _client = client;
    _syncSub = client.onSync.stream.listen((_) => notifyListeners());
    _loginSub = client.onLoginStateChanged.stream.listen((state) async {
      // The server ended the session (token revoked, device deleted).
      if (state == LoginState.loggedOut && identical(_client, client)) {
        _detach();
        await client.dispose();
        await _deleteRemembered();
        _set(SessionState.disconnected);
      }
    });
  }

  void _detach() {
    _syncSub?.cancel();
    _loginSub?.cancel();
    _syncSub = null;
    _loginSub = null;
    _client = null;
  }

  void _set(SessionState state) {
    _state = state;
    notifyListeners();
  }

  @override
  void dispose() {
    _detach();
    super.dispose();
  }

  static Future<Client> _openClient({required bool remember}) async {
    final factory = databaseFactoryFfi;
    final db = await factory.openDatabase(
      remember ? await _rememberedPath() : inMemoryDatabasePath,
    );
    return Client(
      _clientName,
      database: await MatrixSdkDatabase.init(_clientName, database: db, sqfliteFactory: factory),
      logLevel: kReleaseMode ? Level.warning : Level.info,
    );
  }

  static Future<String> _rememberedPath() async {
    final dir = await getApplicationSupportDirectory();
    await dir.create(recursive: true);
    return p.join(dir.path, _rememberedDb);
  }

  static Future<void> _deleteRemembered() async {
    final path = await _rememberedPath();
    await databaseFactoryFfi.deleteDatabase(path);
  }
}

/// `matrix.org`, `https://matrix.org` and `http://localhost:8008` all work;
/// a bare host means https.
Uri homeserverUri(String input) {
  var text = input.trim();
  if (text.isEmpty) text = 'matrix.org';
  if (!text.contains('://')) text = 'https://$text';
  return Uri.parse(text);
}

/// Matrix errors carry a readable message; everything else is shown as is.
String describeError(Object e) {
  if (e is MatrixException) return e.errorMessage;
  if (e is SocketException) return 'Can’t reach the homeserver: ${e.message}';
  return e.toString();
}
