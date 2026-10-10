/// neo's own preferences (not Matrix state), saved as a small JSON file.
library;

import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

enum ThemeChoice { system, light, dark }

class Prefs extends ChangeNotifier {
  ThemeChoice _theme;
  String _homeserver;
  final File? _file;

  Prefs._(this._file, {ThemeChoice theme = ThemeChoice.system, String homeserver = 'matrix.org'})
      // ignore: prefer_initializing_formals
      : _theme = theme,
        // ignore: prefer_initializing_formals
        _homeserver = homeserver;

  /// Defaults, saved nowhere. For tests.
  Prefs.memory() : this._(null);

  static Future<Prefs> load() async {
    final dir = await getApplicationSupportDirectory();
    final file = File(p.join(dir.path, 'prefs.json'));
    try {
      final json = jsonDecode(await file.readAsString()) as Map<String, dynamic>;
      return Prefs._(
        file,
        theme: ThemeChoice.values.asNameMap()[json['theme']] ?? ThemeChoice.system,
        homeserver: json['homeserver'] as String? ?? 'matrix.org',
      );
    } catch (_) {
      return Prefs._(file);
    }
  }

  ThemeChoice get theme => _theme;
  set theme(ThemeChoice value) {
    _theme = value;
    _save();
  }

  /// The last homeserver signed in to, prefilled on the connect screen.
  String get homeserver => _homeserver;
  set homeserver(String value) {
    _homeserver = value;
    _save();
  }

  void _save() {
    notifyListeners();
    final file = _file;
    if (file == null) return;
    file.parent.createSync(recursive: true);
    file.writeAsString(jsonEncode({'theme': _theme.name, 'homeserver': _homeserver}));
  }
}
