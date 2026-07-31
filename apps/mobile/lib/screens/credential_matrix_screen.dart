import 'package:flutter/material.dart';

import '../state/mobile_sync_state.dart';
import '../state/bulk_switch_plan.dart';

class CredentialMatrixScreen extends StatelessWidget {
  const CredentialMatrixScreen({
    required this.hosts,
    this.onProvisionCredential,
    this.onAssignCredential,
    this.onRotateCredential,
    this.onBulkAssignCredentials,
    super.key,
  });

  final Iterable<HostSyncState> hosts;
  final VoidCallback? onProvisionCredential;
  final Future<void> Function(
    HostSyncState host,
    String runtimeId,
    String credentialProfileId,
  )?
  onAssignCredential;
  final Future<void> Function(
    HostSyncState host,
    String runtimeId,
    String rotationPoolId,
  )?
  onRotateCredential;
  final Future<void> Function(List<BulkCredentialSwitchTarget> targets)?
  onBulkAssignCredentials;

  @override
  Widget build(BuildContext context) {
    final profiles = <({HostSyncState host, Map<String, Object?> profile})>[];
    for (final host in hosts) {
      final raw = host.snapshot['credentialProfiles'] as List? ?? const [];
      for (final value in raw) {
        if (value is Map) {
          profiles.add((host: host, profile: Map<String, Object?>.from(value)));
        }
      }
    }
    profiles.sort(
      (left, right) => _string(
        left.profile['displayName'],
      ).compareTo(_string(right.profile['displayName'])),
    );

    return Scaffold(
      appBar: AppBar(title: const Text('Account Profiles & Assignments')),
      floatingActionButton: onProvisionCredential == null
          ? null
          : FloatingActionButton.extended(
              onPressed: onProvisionCredential,
              icon: const Icon(Icons.add),
              label: const Text('Add credential'),
            ),
      body: profiles.isEmpty
          ? const Center(
              child: Padding(
                padding: EdgeInsets.all(24),
                child: Text(
                  'No credential profiles are present in the latest '
                  'authenticated host snapshots.',
                  textAlign: TextAlign.center,
                ),
              ),
            )
          : ListView.separated(
              padding: const EdgeInsets.all(16),
              itemCount: profiles.length,
              separatorBuilder: (_, _) => const SizedBox(height: 8),
              itemBuilder: (context, index) {
                final item = profiles[index];
                final profile = item.profile;
                final profileId = _string(profile['profileId']);
                final assignments = _runtimeAssignments(item.host, profileId);
                final status = _credentialStatus(profile['status']);
                return Card(
                  child: ListTile(
                    leading: const Icon(
                      Icons.vpn_key_outlined,
                      color: Colors.purpleAccent,
                    ),
                    title: Text(
                      _string(
                        profile['displayName'],
                        fallback: profileId.isEmpty
                            ? 'Unnamed profile'
                            : profileId,
                      ),
                    ),
                    subtitle: Text(
                      [
                        item.host.displayName,
                        _string(
                          profile['provider'],
                          fallback: 'unknown provider',
                        ),
                        _maskedFingerprint(profile['accountFingerprint']),
                        _lastValidated(profile['lastValidatedAtMs']),
                        if (assignments.isNotEmpty)
                          'assigned to ${assignments.join(', ')}',
                      ].join(' • '),
                    ),
                    onTap: onAssignCredential == null || !item.host.canMutate
                        ? null
                        : () => _chooseRuntimeAssignment(
                            context,
                            item.host,
                            profile,
                          ),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Column(
                          mainAxisSize: MainAxisSize.min,
                          crossAxisAlignment: CrossAxisAlignment.end,
                          children: [
                            Text(status),
                            if (!item.host.canMutate)
                              const Text(
                                'STALE',
                                style: TextStyle(
                                  color: Colors.amber,
                                  fontSize: 10,
                                ),
                              ),
                          ],
                        ),
                        if (item.host.canMutate)
                          PopupMenuButton<String>(
                            tooltip: 'Credential actions',
                            onSelected: (action) {
                              if (action == 'rotate') {
                                _chooseRuntimeRotation(
                                  context,
                                  item.host,
                                  profileId,
                                );
                              } else {
                                _showBulkImpactPlan(context, profile);
                              }
                            },
                            itemBuilder: (context) => [
                              if (onRotateCredential != null &&
                                  assignments.isNotEmpty)
                                const PopupMenuItem(
                                  value: 'rotate',
                                  child: Text('Rotate an assigned runtime'),
                                ),
                              const PopupMenuItem(
                                value: 'bulk',
                                child: Text('Preview multi-host switch'),
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

  Future<void> _showBulkImpactPlan(
    BuildContext context,
    Map<String, Object?> profile,
  ) async {
    final provider = _string(profile['provider']);
    final fingerprint = _string(profile['accountFingerprint']);
    if (provider.isEmpty || fingerprint.isEmpty) return;
    final plan = BulkCredentialSwitchPlan.build(
      hosts: hosts,
      provider: provider,
      accountFingerprint: fingerprint,
    );
    if (onBulkAssignCredentials == null) {
      return;
    }
    final selected = {...plan.ready};
    await showDialog<void>(
      context: context,
      builder: (dialogContext) => StatefulBuilder(
        builder: (dialogContext, setDialogState) => AlertDialog(
          title: const Text('Multi-host switch impact'),
          content: SizedBox(
            width: double.maxFinite,
            child: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text(
                    'Applies to future work only. Busy runtimes are never '
                    'stopped or moved automatically.',
                  ),
                  const SizedBox(height: 12),
                  Text('Ready to switch: ${plan.ready.length}'),
                  for (final target in plan.ready)
                    CheckboxListTile(
                      value: selected.contains(target),
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      title: Text(_targetLabel(target)),
                      onChanged: (checked) => setDialogState(() {
                        if (checked == true) {
                          selected.add(target);
                        } else {
                          selected.remove(target);
                        }
                      }),
                    ),
                  Text('Already assigned: ${plan.alreadyAssigned.length}'),
                  Text('Offline or stale: ${plan.offline.length}'),
                  Text('Missing this account: ${plan.missingProfile.length}'),
                  const SizedBox(height: 12),
                  const Text(
                    'Each selected runtime is an independent, persisted '
                    'operation. Results can be partial; unconfirmed results '
                    'will be reconciled before they are safe to retry.',
                  ),
                ],
              ),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: selected.isEmpty
                  ? null
                  : () async {
                      final targets = selected.toList(growable: false);
                      Navigator.of(dialogContext).pop();
                      await onBulkAssignCredentials!(targets);
                    },
              child: Text('Switch ${selected.length}'),
            ),
          ],
        ),
      ),
    );
  }

  String _targetLabel(BulkCredentialSwitchTarget target) {
    for (final raw in target.host.snapshot['runtimes'] as List? ?? const []) {
      if (raw is Map) {
        final runtime = Map<String, Object?>.from(raw);
        if (_string(runtime['runtimeId']) == target.runtimeId) {
          return '${target.host.displayName} · ${_string(runtime['name'], fallback: target.runtimeId)}';
        }
      }
    }
    return '${target.host.displayName} · ${target.runtimeId}';
  }

  Future<void> _chooseRuntimeAssignment(
    BuildContext context,
    HostSyncState host,
    Map<String, Object?> profile,
  ) async {
    final profileId = _string(profile['profileId']);
    final provider = _string(profile['provider']);
    if (profileId.isEmpty || onAssignCredential == null) {
      return;
    }
    final runtimes = <Map<String, Object?>>[];
    for (final raw in host.snapshot['runtimes'] as List? ?? const []) {
      if (raw is Map) {
        final runtime = Map<String, Object?>.from(raw);
        if (_string(runtime['runtimeId']).isNotEmpty) {
          runtimes.add(runtime);
        }
      }
    }
    if (runtimes.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('This host has no runtime to assign.')),
      );
      return;
    }
    final selected = await showModalBottomSheet<String>(
      context: context,
      showDragHandle: true,
      builder: (sheetContext) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          children: [
            ListTile(
              title: Text(
                'Assign ${_string(profile['displayName'], fallback: profileId)}',
              ),
              subtitle: const Text(
                'Applies to future work only. Active sessions are never moved.',
              ),
            ),
            for (final runtime in runtimes)
              Builder(
                builder: (context) {
                  final alreadyAssigned =
                      runtime['activeCredentialProfileId'] == profileId;
                  return ListTile(
                    leading: const Icon(Icons.terminal),
                    title: Text(
                      _string(
                        runtime['name'],
                        fallback: _string(runtime['runtimeId']),
                      ),
                    ),
                    subtitle: Text(
                      [
                        if (provider.isNotEmpty) provider,
                        alreadyAssigned
                            ? 'already assigned'
                            : 'current: ${_string(runtime['activeCredentialProfileId'], fallback: 'none')}',
                      ].join(' • '),
                    ),
                    onTap: alreadyAssigned
                        ? null
                        : () => Navigator.of(
                            sheetContext,
                          ).pop(_string(runtime['runtimeId'])),
                  );
                },
              ),
          ],
        ),
      ),
    );
    if (selected == null || selected.isEmpty) {
      return;
    }
    await onAssignCredential!(host, selected, profileId);
  }

  Future<void> _chooseRuntimeRotation(
    BuildContext context,
    HostSyncState host,
    String credentialProfileId,
  ) async {
    if (credentialProfileId.isEmpty || onRotateCredential == null) {
      return;
    }
    final navigator = Navigator.of(context);
    final assignedRuntimes = <Map<String, Object?>>[];
    for (final raw in host.snapshot['runtimes'] as List? ?? const []) {
      if (raw is! Map) {
        continue;
      }
      final runtime = Map<String, Object?>.from(raw);
      if (runtime['activeCredentialProfileId'] == credentialProfileId &&
          _string(runtime['runtimeId']).isNotEmpty) {
        assignedRuntimes.add(runtime);
      }
    }
    final runtimeId = await showModalBottomSheet<String>(
      context: context,
      showDragHandle: true,
      builder: (sheetContext) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          children: [
            const ListTile(
              title: Text('Rotate credential'),
              subtitle: Text(
                'Rotation only affects new work. A busy runtime is never '
                'stopped or switched automatically.',
              ),
            ),
            for (final runtime in assignedRuntimes)
              ListTile(
                leading: const Icon(Icons.sync_lock),
                title: Text(
                  _string(
                    runtime['name'],
                    fallback: _string(runtime['runtimeId']),
                  ),
                ),
                subtitle: const Text(
                  'Choose the configured rotation pool next',
                ),
                onTap: () => Navigator.of(
                  sheetContext,
                ).pop(_string(runtime['runtimeId'])),
              ),
          ],
        ),
      ),
    );
    if (runtimeId == null || runtimeId.isEmpty) {
      return;
    }
    if (!navigator.mounted) {
      return;
    }
    final poolId = await _askForRotationPool(navigator.context);
    if (poolId == null || poolId.isEmpty) {
      return;
    }
    await onRotateCredential!(host, runtimeId, poolId);
  }

  Future<String?> _askForRotationPool(BuildContext context) async {
    final controller = TextEditingController();
    try {
      return await showDialog<String>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: const Text('Rotation pool'),
          content: TextField(
            controller: controller,
            autofocus: true,
            autocorrect: false,
            enableSuggestions: false,
            decoration: const InputDecoration(
              labelText: 'Configured pool ID',
              helperText:
                  'The connector validates the pool and never exposes its credentials.',
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(dialogContext).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () =>
                  Navigator.of(dialogContext).pop(controller.text.trim()),
              child: const Text('Continue'),
            ),
          ],
        ),
      );
    } finally {
      controller.dispose();
    }
  }
}

List<String> _runtimeAssignments(HostSyncState host, String profileId) {
  final result = <String>[];
  final runtimes = host.snapshot['runtimes'] as List? ?? const [];
  for (final value in runtimes) {
    if (value is! Map) {
      continue;
    }
    final runtime = Map<String, Object?>.from(value);
    if (runtime['activeCredentialProfileId'] == profileId) {
      result.add(
        _string(runtime['name'], fallback: _string(runtime['runtimeId'])),
      );
    }
  }
  return result;
}

String _credentialStatus(Object? raw) {
  return switch (raw) {
    1 => 'Staged',
    2 => 'Active',
    3 => 'Cooling down',
    4 => 'Invalid',
    5 => 'Revoked',
    6 => 'Disabled locally',
    _ => 'Unknown',
  };
}

String _maskedFingerprint(Object? raw) {
  final fingerprint = _string(raw);
  if (fingerprint.isEmpty) {
    return 'no account fingerprint';
  }
  if (fingerprint.length <= 12) {
    return fingerprint;
  }
  return '${fingerprint.substring(0, 8)}…${fingerprint.substring(fingerprint.length - 4)}';
}

String _lastValidated(Object? raw) {
  if (raw is! int || raw <= 0) {
    return 'never validated';
  }
  final timestamp = DateTime.fromMillisecondsSinceEpoch(raw, isUtc: true);
  return 'validated ${timestamp.toIso8601String()}';
}

String _string(Object? value, {String fallback = ''}) {
  return value is String && value.trim().isNotEmpty ? value : fallback;
}
