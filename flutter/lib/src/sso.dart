/// Browser single sign-on: neo listens on a localhost port, sends the
/// browser to the homeserver's SSO page with that port as the redirect, and
/// waits for the redirect to bring back a `loginToken`.
library;

import 'dart:async';
import 'dart:io';

import 'package:url_launcher/url_launcher.dart';

class SsoCancelled implements Exception {
  @override
  String toString() => 'Sign-in cancelled';
}

class SsoLogin {
  final Uri url;
  final HttpServer _server;
  final _token = Completer<String>();

  SsoLogin._(this.url, this._server) {
    _server.listen(_handle);
  }

  /// The login token, once the browser comes back with one.
  Future<String> get token => _token.future;

  static Future<SsoLogin> start(Uri homeserver) async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    final redirect = Uri(scheme: 'http', host: 'localhost', port: server.port, path: '/');
    final url = ssoRedirectUrl(homeserver, redirect);
    final login = SsoLogin._(url, server);
    await login.open();
    return login;
  }

  /// Opens the sign-in page in the default browser (again).
  Future<bool> open() async {
    try {
      return await launchUrl(url, mode: LaunchMode.externalApplication);
    } catch (_) {
      return false;
    }
  }

  void cancel() {
    if (!_token.isCompleted) _token.completeError(SsoCancelled());
    _server.close(force: true);
  }

  Future<void> _handle(HttpRequest request) async {
    final token = request.uri.queryParameters['loginToken'];
    request.response.headers.contentType = ContentType.html;
    if (token == null) {
      request.response.statusCode = HttpStatus.notFound;
      await request.response.close();
      return;
    }
    request.response.write(
      '<!doctype html><meta charset="utf-8"><title>neo</title>'
      '<body style="font-family:sans-serif;text-align:center;margin-top:20vh">'
      '<h1>Signed in</h1><p>You can close this tab and go back to neo.</p>',
    );
    await request.response.close();
    if (!_token.isCompleted) _token.complete(token);
    await _server.close();
  }
}

Uri ssoRedirectUrl(Uri homeserver, Uri redirect) => homeserver.resolveUri(Uri(
      path: '_matrix/client/v3/login/sso/redirect',
      queryParameters: {'redirectUrl': redirect.toString()},
    ));
