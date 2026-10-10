import 'package:flatkit/flatkit.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:neo/src/prefs.dart';
import 'package:neo/src/session.dart';
import 'package:neo/src/ui/connect.dart';

void main() {
  testWidgets('connect screen shows the sign-in form', (tester) async {
    await tester.pumpWidget(MaterialApp(
      theme: KitTheme.material(KitColors.light),
      builder: (context, child) => KitTheme(
        colors: KitColors.light,
        child: Material(child: DefaultTextStyle(style: kitTextStyle, child: child!)),
      ),
      home: ConnectScreen(session: NeoSession(), prefs: Prefs.memory()),
    ));
    expect(find.text('neo'), findsOneWidget);
    expect(find.text('Sign in'), findsWidgets);
    expect(find.text('Sign in with SSO'), findsOneWidget);
    expect(find.text('Remember me on this device'), findsOneWidget);
    final password = tester.widgetList<TextField>(find.byType(TextField)).last;
    expect(password.obscureText, isTrue);
    expect(find.text('matrix.org'), findsOneWidget);
  });
}
