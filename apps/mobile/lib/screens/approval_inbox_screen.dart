import 'package:flutter/material.dart';

import '../state/mobile_sync_state.dart';

class ApprovalInboxScreen extends StatelessWidget {
  const ApprovalInboxScreen({required this.hosts, super.key});

  final Iterable<HostSyncState> hosts;

  @override
  Widget build(BuildContext context) {
    final approvals =
        <String, ({HostSyncState host, Map<String, Object?> event})>{};
    for (final host in hosts) {
      final recent = host.snapshot['recentEvents'] as List? ?? const [];
      for (final value in recent) {
        if (value is! Map) {
          continue;
        }
        final event = Map<String, Object?>.from(value);
        final approvalId = event['approvalId'];
        if (approvalId is! String || approvalId.isEmpty) {
          continue;
        }
        switch (event['kind']) {
          case 'approvalRequested':
            approvals[approvalId] = (host: host, event: event);
            break;
          case 'approvalResolved':
            approvals.remove(approvalId);
            break;
        }
      }
    }
    final pending = approvals.values.toList(growable: false)
      ..sort(
        (left, right) => _timestamp(
          right.event,
        ).compareTo(_timestamp(left.event)),
      );

    return Scaffold(
      appBar: AppBar(title: const Text('Approval Inbox')),
      body: pending.isEmpty
          ? const Center(
              child: Padding(
                padding: EdgeInsets.all(24),
                child: Text(
                  'No unresolved approvals are present in the synchronized '
                  'event window.',
                  textAlign: TextAlign.center,
                ),
              ),
            )
          : ListView.separated(
              padding: const EdgeInsets.all(16),
              itemCount: pending.length,
              separatorBuilder: (_, _) => const SizedBox(height: 8),
              itemBuilder: (context, index) {
                final item = pending[index];
                final event = item.event;
                final approvalId = event['approvalId']! as String;
                final actionType = _text(
                  event['actionType'],
                  fallback: 'action',
                );
                return Card(
                  color: Colors.amber.withValues(alpha: 0.08),
                  child: Padding(
                    padding: const EdgeInsets.all(16),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Row(
                          children: [
                            const Icon(Icons.gavel, color: Colors.amber),
                            const SizedBox(width: 8),
                            Expanded(
                              child: Text(
                                '$actionType approval',
                                style: Theme.of(context).textTheme.titleMedium,
                              ),
                            ),
                            Text(item.host.displayName),
                          ],
                        ),
                        const SizedBox(height: 8),
                        Text(
                          'Session ${_text(event['sessionId'], fallback: 'unknown')}',
                        ),
                        Text(
                          'Approval ${_shortId(approvalId)}',
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                        const SizedBox(height: 12),
                        Text(
                          item.host.canMutate
                              ? 'Action details are intentionally fetched only '
                                    'when interactive approval dispatch is wired.'
                              : 'Cached event • reconnect before taking action.',
                        ),
                        const SizedBox(height: 12),
                        Row(
                          mainAxisAlignment: MainAxisAlignment.end,
                          children: [
                            OutlinedButton(
                              onPressed: null,
                              child: const Text('Reject'),
                            ),
                            const SizedBox(width: 8),
                            FilledButton(
                              onPressed: null,
                              child: const Text('Approve'),
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                );
              },
            ),
    );
  }
}

int _timestamp(Map<String, Object?> event) {
  final value = event['timestampMs'];
  return value is int ? value : 0;
}

String _shortId(String value) {
  return value.length <= 12 ? value : '${value.substring(0, 12)}…';
}

String _text(Object? value, {String fallback = ''}) {
  return value is String && value.trim().isNotEmpty ? value : fallback;
}
