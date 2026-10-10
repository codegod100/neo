/// Turns a room's timeline into the rows neo draws: chat messages with a
/// day separator wherever the calendar day changes.
library;

import 'package:matrix/matrix.dart';

/// Event types shown as chat lines. Everything else (membership, room
/// setup, reactions) stays out of the timeline.
const shownTypes = {EventTypes.Message, EventTypes.Encrypted, EventTypes.Sticker};

sealed class TimelineRow {
  const TimelineRow();
}

class MessageRow extends TimelineRow {
  final Event event;
  const MessageRow(this.event);
}

class DayRow extends TimelineRow {
  final DateTime day;
  const DayRow(this.day);
}

String senderName(Event event) => event.senderFromMemoryOrFallback.calcDisplayname();

String messageBody(Event event) {
  if (event.type == EventTypes.Encrypted) {
    return 'Unable to decrypt this message yet.';
  }
  return event.calcLocalizedBodyFallback(const MatrixDefaultLocalizations(), hideReply: true);
}

/// Whether [event] matches the room's search box: its text, sender ID or
/// display name.
bool matchesSearch(Event event, String query) {
  final needle = query.trim().toLowerCase();
  if (needle.isEmpty) return true;
  return messageBody(event).toLowerCase().contains(needle) ||
      event.senderId.toLowerCase().contains(needle) ||
      senderName(event).toLowerCase().contains(needle);
}

/// [events] newest first (as [Timeline.events] keeps them); the result is
/// newest first too, for a reversed list, with each day's separator placed
/// after (above, on screen) that day's oldest message.
List<TimelineRow> buildRows(Iterable<Event> events, {String query = ''}) {
  final shown = events
      .where((e) => shownTypes.contains(e.type) && e.redacted == false)
      .where((e) => matchesSearch(e, query))
      .toList();
  final rows = <TimelineRow>[];
  for (var i = 0; i < shown.length; i++) {
    final event = shown[i];
    rows.add(MessageRow(event));
    final day = dayOf(event.originServerTs);
    final older = i + 1 < shown.length ? dayOf(shown[i + 1].originServerTs) : null;
    if (older != day) rows.add(DayRow(day));
  }
  return rows;
}

DateTime dayOf(DateTime t) {
  final local = t.toLocal();
  return DateTime(local.year, local.month, local.day);
}

const _weekdays = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];
const _months = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
];

/// "Today", "Yesterday", or "Wednesday, August 20, 2026".
String dayLabel(DateTime day, {DateTime? now}) {
  final today = dayOf(now ?? DateTime.now());
  if (day == today) return 'Today';
  if (day == today.subtract(const Duration(days: 1))) return 'Yesterday';
  return '${_weekdays[day.weekday - 1]}, ${_months[day.month - 1]} ${day.day}, ${day.year}';
}

String timeLabel(DateTime t) {
  final local = t.toLocal();
  return '${local.hour.toString().padLeft(2, '0')}:${local.minute.toString().padLeft(2, '0')}';
}
