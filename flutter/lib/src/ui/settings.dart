import 'package:flatkit/flatkit.dart';
import 'package:flutter/material.dart';
import 'package:matrix/matrix.dart';

import '../prefs.dart';
import 'common.dart';

class SettingsScreen extends StatelessWidget {
  final Client client;
  final Prefs prefs;
  final VoidCallback onBack;
  final VoidCallback onSignOut;

  const SettingsScreen({
    super.key,
    required this.client,
    required this.prefs,
    required this.onBack,
    required this.onSignOut,
  });

  @override
  Widget build(BuildContext context) {
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      ScreenHeader(title: 'Settings', onBack: onBack),
      Expanded(
        child: ListView(padding: const EdgeInsets.all(12), children: [
          SettingsGroup(title: 'Account', children: [
            SettingsRow(title: 'Signed in as', subtitle: client.userID ?? '—'),
            SettingsRow(title: 'Homeserver', subtitle: client.homeserver?.toString() ?? '—'),
            SettingsRow(title: 'Device', subtitle: client.deviceID ?? '—'),
            SettingsRow(
              title: 'Encryption',
              subtitle: client.encryptionEnabled ? 'End-to-end encryption on' : 'Unavailable on this build',
            ),
          ]),
          const SizedBox(height: 20),
          SettingsGroup(title: 'Appearance', children: [
            SettingsRow(
              title: 'Theme',
              trailing: ListenableBuilder(
                listenable: prefs,
                builder: (context, _) => KitSegmented<ThemeChoice>(
                  segments: const [
                    (ThemeChoice.system, 'System'),
                    (ThemeChoice.light, 'Light'),
                    (ThemeChoice.dark, 'Dark'),
                  ],
                  selected: prefs.theme,
                  onChanged: (v) => prefs.theme = v,
                ),
              ),
            ),
          ]),
          const SizedBox(height: 20),
          Align(
            alignment: Alignment.centerLeft,
            child: KitButton.danger('Sign out', onPressed: () => _confirmSignOut(context)),
          ),
          const SizedBox(height: 16),
          Text('neo — a matrix.org client built with Flutter and flatkit.',
              style: KitText.dim(context, small: true)),
        ]),
      ),
    ]);
  }

  Future<void> _confirmSignOut(BuildContext context) async {
    final ok = await showKitDialog<bool>(
      context,
      (context) => KitDialog(
        title: 'Sign out?',
        content: const Text(
          'This device is removed from your account. Encrypted messages you can’t '
          'recover from another session or key backup will no longer be readable here.',
        ),
        actions: [
          KitButton('Cancel', onPressed: () => Navigator.of(context).pop(false)),
          KitButton.danger('Sign out', onPressed: () => Navigator.of(context).pop(true)),
        ],
      ),
    );
    if (ok == true) onSignOut();
  }
}
