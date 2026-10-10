import 'package:flutter/widgets.dart';
import 'package:flutter_vodozemac/flutter_vodozemac.dart' as vod;
import 'package:sqflite_common_ffi/sqflite_ffi.dart';

import 'src/prefs.dart';
import 'src/session.dart';
import 'src/ui/shell.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  sqfliteFfiInit();
  try {
    // End-to-end encryption. Without it neo still works, but encrypted
    // rooms can't be read.
    await vod.init();
  } catch (e) {
    debugPrint('vodozemac unavailable, encryption off: $e');
  }
  final prefs = await Prefs.load();
  final session = NeoSession();
  await session.restore();
  runApp(NeoApp(session: session, prefs: prefs));
}
