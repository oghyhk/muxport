import 'package:flutter/material.dart';

import '../state/mobile_sync_state.dart';

class SessionTimelineScreen extends StatelessWidget {
  const SessionTimelineScreen({required this.hosts, super.key});

  final Iterable<HostSyncState> hosts;

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
                final profile = _text(session['credentialProfileId']);
                return Card(
                  child: ListTile(
                    leading: Icon(
                      status == 'waiting_approval'
                          ? Icons.gavel
                          : status == 'running' || status == 'inProgress'
                          ? Icons.sync
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
                    trailing: _StatusBadge(
                      status: status,
                      stale: !item.host.canMutate,
                    ),
                  ),
                );
              },
            ),
    );
  }
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

String _text(Object? value, {String fallback = ''}) {
  return value is String && value.trim().isNotEmpty ? value : fallback;
}
