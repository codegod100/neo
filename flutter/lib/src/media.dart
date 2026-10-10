/// Avatars and inline image previews, fetched from the homeserver's media
/// repository (with the access token, for authenticated media) and kept in
/// memory for the session.
library;

import 'dart:typed_data';

import 'package:flatkit/flatkit.dart';
import 'package:flutter/widgets.dart';
import 'package:matrix/matrix.dart';

final _thumbnails = <String, Future<Uint8List?>>{};
final _attachments = <String, Future<Uint8List?>>{};

Future<Uint8List?> _thumbnail(Client client, Uri mxc, int size) =>
    _thumbnails.putIfAbsent('$mxc@$size', () async {
      try {
        final uri = await mxc.getThumbnailUri(client, width: size, height: size);
        final response = await client.httpClient.get(
          uri,
          headers: {'authorization': 'Bearer ${client.accessToken}'},
        );
        return response.statusCode == 200 ? response.bodyBytes : null;
      } catch (_) {
        return null;
      }
    });

/// The decrypted thumbnail (or the image itself when it has none) of an
/// image, sticker or video message.
Future<Uint8List?> attachmentPreview(Event event) =>
    _attachments.putIfAbsent(event.eventId, () async {
      try {
        final file = await event.downloadAndDecryptAttachment(getThumbnail: event.hasThumbnail);
        return file.bytes;
      } catch (_) {
        return null;
      }
    });

bool hasPreview(Event event) =>
    event.type == EventTypes.Sticker ||
    (event.type == EventTypes.Message &&
        (event.messageType == MessageTypes.Image ||
            (event.messageType == MessageTypes.Video && event.hasThumbnail)));

/// A round avatar: the image at [mxc] when there is one, otherwise the
/// first letter of [name] on a colour picked from the name.
class Avatar extends StatelessWidget {
  final Client client;
  final Uri? mxc;
  final String name;
  final double size;
  final double radius;

  const Avatar({
    super.key,
    required this.client,
    required this.mxc,
    required this.name,
    this.size = 32,
    this.radius = 999,
  });

  @override
  Widget build(BuildContext context) {
    final fallback = _Initial(name: name, size: size, radius: radius);
    final mxc = this.mxc;
    if (mxc == null) return fallback;
    final px = (size * MediaQuery.devicePixelRatioOf(context)).ceil().clamp(32, 256);
    return FutureBuilder<Uint8List?>(
      future: _thumbnail(client, mxc, px),
      builder: (context, snap) {
        final bytes = snap.data;
        if (bytes == null) return fallback;
        return ClipRRect(
          borderRadius: BorderRadius.circular(radius),
          child: Image.memory(
            bytes,
            width: size,
            height: size,
            fit: BoxFit.cover,
            gaplessPlayback: true,
            errorBuilder: (_, _, _) => fallback,
          ),
        );
      },
    );
  }
}

const _palette = [
  Color(0xFF3584E4), Color(0xFF2EC27E), Color(0xFFE5A50A), Color(0xFFE66100),
  Color(0xFFC01C28), Color(0xFF9141AC), Color(0xFF986A44), Color(0xFF1C71D8),
];

class _Initial extends StatelessWidget {
  final String name;
  final double size;
  final double radius;

  const _Initial({required this.name, required this.size, required this.radius});

  @override
  Widget build(BuildContext context) {
    final letters = name.replaceFirst(RegExp(r'^[#@!]'), '').trim();
    final initial = letters.isEmpty ? '?' : letters.characters.first.toUpperCase();
    final color = _palette[name.hashCode.abs() % _palette.length];
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(color: color, borderRadius: BorderRadius.circular(radius)),
      child: Text(
        initial,
        style: KitText.heading(context).copyWith(
          color: const Color(0xFFFFFFFF),
          fontSize: size * 0.42,
          height: 1,
        ),
      ),
    );
  }
}
