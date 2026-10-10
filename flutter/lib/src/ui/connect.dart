import 'package:flatkit/flatkit.dart';
import 'package:flutter/material.dart' ;

import '../prefs.dart';
import '../session.dart';
import 'common.dart';

class ConnectScreen extends StatefulWidget {
  final NeoSession session;
  final Prefs prefs;

  const ConnectScreen({super.key, required this.session, required this.prefs});

  @override
  State<ConnectScreen> createState() => _ConnectScreenState();
}

class _ConnectScreenState extends State<ConnectScreen> {
  late final _homeserver = TextEditingController(text: widget.prefs.homeserver);
  final _username = TextEditingController();
  final _password = TextEditingController();
  bool _remember = true;

  NeoSession get _session => widget.session;
  bool get _busy => _session.state == SessionState.connecting;

  @override
  void dispose() {
    _homeserver.dispose();
    _username.dispose();
    _password.dispose();
    super.dispose();
  }

  void _signIn() {
    if (_busy || _username.text.trim().isEmpty || _password.text.isEmpty) return;
    widget.prefs.homeserver = _homeserver.text.trim();
    _session.loginWithPassword(
      homeserver: _homeserver.text,
      username: _username.text,
      password: _password.text,
      remember: _remember,
    );
  }

  void _signInSso() {
    if (_busy) return;
    widget.prefs.homeserver = _homeserver.text.trim();
    _session.loginWithSso(homeserver: _homeserver.text, remember: _remember);
  }

  @override
  Widget build(BuildContext context) {
    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(24),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 380),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text('neo',
                  textAlign: TextAlign.center,
                  style: KitText.heading(context).copyWith(fontSize: 28)),
              const SizedBox(height: 4),
              Text('A matrix.org client.', textAlign: TextAlign.center, style: KitText.dim(context)),
              const SizedBox(height: 24),
              if (_session.sso != null) _ssoWaiting(context) else _form(context),
            ],
          ),
        ),
      ),
    );
  }

  Widget _ssoWaiting(BuildContext context) {
    final sso = _session.sso!;
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      Text('Continue in your browser',
          textAlign: TextAlign.center, style: KitText.heading(context).copyWith(fontSize: 16)),
      const SizedBox(height: 8),
      Text('We opened your homeserver’s sign-in page. Waiting for it to finish…',
          textAlign: TextAlign.center, style: KitText.dim(context)),
      const SizedBox(height: 16),
      const Center(child: KitSpinner(size: 20)),
      const SizedBox(height: 16),
      SelectableText(sso.url.toString(),
          textAlign: TextAlign.center, style: KitText.dim(context, small: true)),
      const SizedBox(height: 12),
      Row(mainAxisAlignment: MainAxisAlignment.center, children: [
        KitButton('Open again', onPressed: sso.open),
        const SizedBox(width: 8),
        KitButton.ghost('Cancel', onPressed: _session.cancelSso),
      ]),
    ]);
  }

  Widget _form(BuildContext context) {
    final c = KitTheme.of(context);
    final error = _session.loginError;
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      if (error != null) ...[
        Text(error, style: KitText.body(context).copyWith(color: c.danger)),
        const SizedBox(height: 12),
      ],
      SettingsGroup(
        title: 'Sign in',
        description: 'Your homeserver account — the same one Element uses.',
        children: [
          _field('Homeserver', KitTextField(
            controller: _homeserver,
            hint: 'example.org',
            enabled: !_busy,
          )),
          _field('Username', KitTextField(
            controller: _username,
            hint: 'alice',
            enabled: !_busy,
            autofocus: true,
          )),
          _field('Password', KitTextField(
            controller: _password,
            obscureText: true,
            enabled: !_busy,
            onSubmitted: (_) => _signIn(),
          )),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            child: KitCheckbox(
              value: _remember,
              label: 'Remember me on this device',
              onChanged: _busy ? null : (v) => setState(() => _remember = v),
            ),
          ),
        ],
      ),
      const SizedBox(height: 16),
      if (_busy)
        const Center(child: KitSpinner(size: 20))
      else ...[
        KitButton.primary('Sign in', onPressed: _signIn),
        const SizedBox(height: 8),
        Text('— or —', textAlign: TextAlign.center, style: KitText.dim(context)),
        const SizedBox(height: 8),
        KitButton('Sign in with SSO', onPressed: _signInSso),
      ],
      const SizedBox(height: 16),
      Text(
        'Login only — registration isn’t supported yet. Passwords go straight to your '
        'homeserver over HTTPS and are never stored by neo.',
        style: KitText.dim(context, small: true),
      ),
    ]);
  }

  Widget _field(String label, Widget field) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      child: Row(children: [
        SizedBox(width: 96, child: Text(label)),
        Expanded(child: field),
      ]),
    );
  }
}
