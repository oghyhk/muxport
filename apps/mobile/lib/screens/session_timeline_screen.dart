import 'package:flutter/material.dart';

import '../state/mobile_sync_state.dart';

class SessionTimelineScreen extends StatelessWidget {
  const SessionTimelineScreen({
    required this.hosts,
    this.onStartSession,
    this.onSendInput,
    this.onSteerSession,
    this.onInterruptSession,
    super.key,
  });

  final Iterable<HostSyncState> hosts;
  final Future<void> Function(
    HostSyncState host,
    String runtimeId,
    String projectPath,
    String prompt,
    String credentialProfileId,
  )?
  onStartSession;
  final Future<void> Function(
    HostSyncState host,
    String runtimeId,
    String sessionId,
    String text,
  )?
  onSendInput;
  final Future<void> Function(
    HostSyncState host,
    String runtimeId,
    String sessionId,
    String instruction,
  )?
  onSteerSession;
  final Future<void> Function(
    HostSyncState host,
    String runtimeId,
    String sessionId,
  )?
  onInterruptSession;

  @override
  Widget build(BuildContext context) {
    final sessions = <({HostSyncState host, Map<String, Object?> session})>[];
    for (final host in hosts) {
      final raw = host.snapshot['activeSessions'] as List? ?? const [];
      for (final value in raw) {
        if (value is Map) {
          sessions.add((host: host, session: Map<String, Object?>.from(value)));
        }
      }
    }
    sessions.sort(
      (left, right) =>
          _text(left.session['title']).compareTo(_text(right.session['title'])),
    );

    return Scaffold(
      appBar: AppBar(title: const Text('Unified Session Timeline')),
      floatingActionButton: onStartSession == null
          ? null
          : FloatingActionButton.extended(
              onPressed: () => _startSession(context),
              icon: const Icon(Icons.add_comment_outlined),
              label: const Text('New session'),
            ),
      body: sessions.isEmpty
          ? const _EmptySessions()
          : ListView.separated(
              padding: const EdgeInsets.all(16),
              itemCount: sessions.length,
              separatorBuilder: (_, _) => const SizedBox(height: 8),
              itemBuilder: (context, index) {
                final item = sessions[index];
                final session = item.session;
                final status = _text(session['status'], fallback: 'unknown');
                final statusLabel = _sessionStatusLabel(status);
                final profile = _text(session['credentialProfileId']);
                return Card(
                  child: ListTile(
                    leading: Icon(
                      status == 'waiting_approval'
                          ? Icons.gavel
                          : status == 'running' || status == 'inProgress'
                          ? Icons.sync
                          : status == 'outcomeUnknown'
                          ? Icons.warning_amber_rounded
                          : Icons.forum_outlined,
                    ),
                    title: Text(
                      _text(session['title'], fallback: 'Untitled session'),
                    ),
                    subtitle: Text(
                      [
                        item.host.displayName,
                        _text(
                          session['runtimeId'],
                          fallback: 'unknown runtime',
                        ),
                        _text(
                          session['projectPath'],
                          fallback: 'unknown project',
                        ),
                        if (profile.isNotEmpty) 'profile $profile',
                      ].join(' • '),
                      maxLines: 3,
                      overflow: TextOverflow.ellipsis,
                    ),
                    onTap: onSendInput == null || !item.host.canMutate
                        ? null
                        : () =>
                              _showSessionActions(context, item.host, session),
                    trailing: _StatusBadge(
                      status: statusLabel,
                      stale: !item.host.canMutate,
                    ),
                  ),
                );
              },
            ),
    );
  }

  Future<void> _startSession(BuildContext context) async {
    if (onStartSession == null) {
      return;
    }
    final navigator = Navigator.of(context);
    final targets = <_StartTarget>[];
    for (final host in hosts) {
      if (!host.canMutate) {
        continue;
      }
      for (final raw in host.snapshot['runtimes'] as List? ?? const []) {
        if (raw is! Map) {
          continue;
        }
        final runtime = Map<String, Object?>.from(raw);
        final runtimeId = _text(runtime['runtimeId']);
        final profileId = _text(runtime['activeCredentialProfileId']);
        final paths = runtime['projectPaths'] as List? ?? const [];
        if (runtimeId.isEmpty) {
          continue;
        }
        for (final rawPath in paths) {
          final projectPath = _text(rawPath);
          if (projectPath.isNotEmpty) {
            targets.add(
              _StartTarget(
                host: host,
                runtimeId: runtimeId,
                runtimeName: _text(runtime['name'], fallback: runtimeId),
                projectPath: projectPath,
                credentialProfileId: profileId,
              ),
            );
          }
        }
      }
    }
    if (targets.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'No mutable runtime with a known project is available.',
          ),
        ),
      );
      return;
    }
    final selected = await showModalBottomSheet<_StartTarget>(
      context: context,
      showDragHandle: true,
      builder: (sheetContext) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          children: [
            const ListTile(
              title: Text('Start a remote session'),
              subtitle: Text(
                'Choose the host, runtime, and project. The configured '
                'credential is used for this new session.',
              ),
            ),
            for (final target in targets)
              ListTile(
                leading: const Icon(Icons.terminal),
                title: Text(
                  '${target.host.displayName} · ${target.runtimeName}',
                ),
                subtitle: Text(target.projectPath),
                onTap: () => Navigator.of(sheetContext).pop(target),
              ),
          ],
        ),
      ),
    );
    if (selected == null || !navigator.mounted) {
      return;
    }
    final prompt = await _askForText(
      navigator.context,
      title: 'Start ${selected.runtimeName}',
      label: 'Prompt',
      helper:
          'The prompt is sent only through the authenticated host connection.',
      action: 'Start',
    );
    if (prompt == null || prompt.isEmpty) {
      return;
    }
    await onStartSession!(
      selected.host,
      selected.runtimeId,
      selected.projectPath,
      prompt,
      selected.credentialProfileId,
    );
  }

  Future<void> _showSessionActions(
    BuildContext context,
    HostSyncState host,
    Map<String, Object?> session,
  ) async {
    final runtimeId = _text(session['runtimeId']);
    final sessionId = _text(session['sessionId']);
    if (runtimeId.isEmpty || sessionId.isEmpty || onSendInput == null) {
      return;
    }
    final recentOutput = _recentOutput(host, sessionId);
    final navigator = Navigator.of(context);
    final action = await showModalBottomSheet<String>(
      context: context,
      showDragHandle: true,
      builder: (sheetContext) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          children: [
            ListTile(
              title: Text(_text(session['title'], fallback: 'Session actions')),
              subtitle: const Text(
                'Commands are routed to this exact session.',
              ),
            ),
            ListTile(
              leading: const Icon(Icons.send_outlined),
              title: const Text('Send input'),
              subtitle: const Text('Continue this session'),
              onTap: () => Navigator.of(sheetContext).pop('input'),
            ),
            if (recentOutput.isNotEmpty)
              ListTile(
                leading: const Icon(Icons.subject_outlined),
                title: const Text('View recent output'),
                subtitle: const Text(
                  'Live output retained in the encrypted phone cache',
                ),
                onTap: () => Navigator.of(sheetContext).pop('output'),
              ),
            if (onSteerSession != null)
              ListTile(
                leading: const Icon(Icons.alt_route),
                title: const Text('Steer'),
                subtitle: const Text('Add a high-priority instruction'),
                onTap: () => Navigator.of(sheetContext).pop('steer'),
              ),
            if (onInterruptSession != null)
              ListTile(
                leading: const Icon(Icons.stop_circle_outlined),
                title: const Text('Interrupt'),
                subtitle: const Text(
                  'Stop the active turn without retrying it',
                ),
                onTap: () => Navigator.of(sheetContext).pop('interrupt'),
              ),
          ],
        ),
      ),
    );
    if (action == null || !navigator.mounted) {
      return;
    }
    if (action == 'interrupt') {
      await onInterruptSession!(host, runtimeId, sessionId);
      return;
    }
    if (action == 'output') {
      await showDialog<void>(
        context: navigator.context,
        builder: (dialogContext) => AlertDialog(
          title: const Text('Recent output'),
          content: SingleChildScrollView(child: SelectableText(recentOutput)),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Close'),
            ),
          ],
        ),
      );
      return;
    }
    final isSteer = action == 'steer';
    final text = await _askForText(
      navigator.context,
      title: isSteer ? 'Steer session' : 'Send input',
      label: isSteer ? 'Instruction' : 'Message',
      helper: isSteer
          ? 'Steering is delivered to the current turn.'
          : 'The message continues the current session.',
      action: isSteer ? 'Steer' : 'Send',
    );
    if (text == null || text.isEmpty) {
      return;
    }
    if (isSteer) {
      await onSteerSession!(host, runtimeId, sessionId, text);
    } else {
      await onSendInput!(host, runtimeId, sessionId, text);
    }
  }

  Future<String?> _askForText(
    BuildContext context, {
    required String title,
    required String label,
    required String helper,
    required String action,
  }) async {
    final controller = TextEditingController();
    try {
      return await showDialog<String>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: Text(title),
          content: TextField(
            controller: controller,
            autofocus: true,
            autocorrect: false,
            minLines: 3,
            maxLines: 8,
            decoration: InputDecoration(labelText: label, helperText: helper),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () =>
                  Navigator.of(dialogContext).pop(controller.text.trim()),
              child: Text(action),
            ),
          ],
        ),
      );
    } finally {
      controller.dispose();
    }
  }
}

class _StartTarget {
  const _StartTarget({
    required this.host,
    required this.runtimeId,
    required this.runtimeName,
    required this.projectPath,
    required this.credentialProfileId,
  });

  final HostSyncState host;
  final String runtimeId;
  final String runtimeName;
  final String projectPath;
  final String credentialProfileId;
}

class _EmptySessions extends StatelessWidget {
  const _EmptySessions();

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: Padding(
        padding: EdgeInsets.all(24),
        child: Text(
          'No sessions are present in the latest authenticated host snapshots.',
          textAlign: TextAlign.center,
        ),
      ),
    );
  }
}

class _StatusBadge extends StatelessWidget {
  const _StatusBadge({required this.status, required this.stale});

  final String status;
  final bool stale;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.end,
      children: [
        Text(status, style: Theme.of(context).textTheme.labelMedium),
        if (stale)
          Text(
            'STALE',
            style: Theme.of(
              context,
            ).textTheme.labelSmall?.copyWith(color: Colors.amber),
          ),
      ],
    );
  }
}

String _sessionStatusLabel(String status) {
  return switch (status) {
    'outcomeUnknown' => 'OUTCOME UNKNOWN · RESYNCING',
    _ => status,
  };
}

String _recentOutput(HostSyncState host, String sessionId) {
  final parts = <String>[];
  final events = host.snapshot['recentEvents'] as List? ?? const [];
  for (final raw in events) {
    if (raw is! Map) {
      continue;
    }
    final event = Map<String, Object?>.from(raw);
    if (event['kind'] == 'streamDelta' && event['sessionId'] == sessionId) {
      final text = _text(event['deltaText']);
      if (text.isNotEmpty) {
        parts.add(text);
      }
    }
  }
  return parts.join();
}

String _text(Object? value, {String fallback = ''}) {
  return value is String && value.trim().isNotEmpty ? value : fallback;
}
