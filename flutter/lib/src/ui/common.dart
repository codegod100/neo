import 'package:flatkit/flatkit.dart';
import 'package:flutter/material.dart';

/// A title row with an optional back button, like a libadwaita header.
class ScreenHeader extends StatelessWidget {
  final String title;
  final String? subtitle;
  final VoidCallback? onBack;
  final List<Widget> actions;

  const ScreenHeader({
    super.key,
    required this.title,
    this.subtitle,
    this.onBack,
    this.actions = const [],
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(8, 8, 8, 4),
      child: Row(children: [
        if (onBack != null) KitIconButton(Icons.arrow_back, onPressed: onBack, tooltip: 'Back'),
        if (onBack != null) const SizedBox(width: 4),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(title,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: KitText.heading(context).copyWith(fontSize: 16)),
              if (subtitle != null && subtitle!.isNotEmpty)
                SelectableText(subtitle!, maxLines: 1, style: KitText.dim(context, small: true)),
            ],
          ),
        ),
        ...actions,
      ]),
    );
  }
}

/// A centred spinner with a caption, for screens still loading.
class LoadingState extends StatelessWidget {
  final String label;
  const LoadingState(this.label, {super.key});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(mainAxisSize: MainAxisSize.min, children: [
        const KitSpinner(size: 20),
        const SizedBox(height: 12),
        Text(label, style: KitText.dim(context)),
      ]),
    );
  }
}

class EmptyState extends StatelessWidget {
  final String label;
  const EmptyState(this.label, {super.key});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Text(label, textAlign: TextAlign.center, style: KitText.dim(context)),
      ),
    );
  }
}

/// A titled group of rows, like `AdwPreferencesGroup`.
class SettingsGroup extends StatelessWidget {
  final String title;
  final String? description;
  final List<Widget> children;

  const SettingsGroup({super.key, required this.title, this.description, required this.children});

  @override
  Widget build(BuildContext context) {
    final rows = <Widget>[];
    for (var i = 0; i < children.length; i++) {
      if (i > 0) rows.add(const KitDivider());
      rows.add(children[i]);
    }
    return Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: [
      Text(title, style: KitText.heading(context)),
      if (description != null) ...[
        const SizedBox(height: 2),
        Text(description!, style: KitText.dim(context, small: true)),
      ],
      const SizedBox(height: 8),
      KitPanel(child: Column(crossAxisAlignment: CrossAxisAlignment.stretch, children: rows)),
    ]);
  }
}

/// One row in a [Group]: a label on the left and [trailing] on the right,
/// or a [subtitle] under the label.
class SettingsRow extends StatelessWidget {
  final String title;
  final String? subtitle;
  final Widget? trailing;

  const SettingsRow({super.key, required this.title, this.subtitle, this.trailing});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      child: Row(children: [
        Expanded(
          child: Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
            Text(title),
            if (subtitle != null) SelectableText(subtitle!, style: KitText.dim(context, small: true)),
          ]),
        ),
        if (trailing != null) ...[const SizedBox(width: 12), trailing!],
      ]),
    );
  }
}
