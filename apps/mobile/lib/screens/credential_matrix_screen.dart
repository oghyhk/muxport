import 'package:flutter/material.dart';

import '../state/mobile_sync_state.dart';
import '../state/bulk_switch_plan.dart';
import '../state/rotation_pool.dart';

class CredentialMatrixScreen extends StatelessWidget {
  const CredentialMatrixScreen({
    required this.hosts,
    this.onProvisionCredential,
    this.onAssignCredential,
    this.onRotateCredential,
    this.onBulkAssignCredentials,
    this.onRetryBulkSwitch,
    this.onListRotationPools,
    this.onUpsertRotationPool,
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
  final Future<void> Function(BulkSwitchRetryGroup group)? onRetryBulkSwitch;
  final Future<List<RotationPoolSummary>> Function(HostSyncState host)?
  onListRotationPools;
  final Future<void> Function(HostSyncState host, RotationPoolDraft draft)?
  onUpsertRotationPool;

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
    final assignments = _assignmentRows(hosts);
    final retryGroups = BulkSwitchRetryGroup.failedFromHosts(hosts);

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
              itemCount: profiles.length + 1,
              separatorBuilder: (_, _) => const SizedBox(height: 8),
              itemBuilder: (context, index) {
                if (index == 0) {
                  return _AssignmentMatrixCard(
                    rows: assignments,
                    retryGroups: retryGroups,
                    onRetryBulkSwitch: onRetryBulkSwitch,
                  );
                }
                final item = profiles[index - 1];
                final profile = item.profile;
                final profileId = _string(profile['profileId']);
                final activeAssignments = _runtimeAssignments(
                  item.host,
                  profileId,
                );
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
                        if (activeAssignments.isNotEmpty)
                          'assigned to ${activeAssignments.join(', ')}',
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
                              } else if (action == 'configurePool') {
                                _configureRotationPool(
                                  context,
                                  item.host,
                                  profile,
                                );
                              } else {
                                _showBulkImpactPlan(context, profile);
                              }
                            },
                            itemBuilder: (context) => [
                              if (onRotateCredential != null &&
                                  activeAssignments.isNotEmpty)
                                const PopupMenuItem(
                                  value: 'rotate',
                                  child: Text('Rotate an assigned runtime'),
                                ),
                              if (onUpsertRotationPool != null)
                                const PopupMenuItem(
                                  value: 'configurePool',
                                  child: Text('Configure rotation pool'),
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
                  Text('Incompatible provider: ${plan.incompatible.length}'),
                  Text('Offline or stale: ${plan.offline.length}'),
                  Text('Credential locked: ${plan.locked.length}'),
                  Text('Busy with active work: ${plan.busy.length}'),
                  Text('Unmanaged external runtime: ${plan.unmanaged.length}'),
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
    final poolId = await _chooseRotationPool(navigator.context, host);
    if (poolId == null || poolId.isEmpty) {
      return;
    }
    await onRotateCredential!(host, runtimeId, poolId);
  }

  Future<String?> _chooseRotationPool(
    BuildContext context,
    HostSyncState host,
  ) async {
    if (onListRotationPools == null) return null;
    List<RotationPoolSummary> pools;
    try {
      pools = await onListRotationPools!(host);
    } on Object {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Could not read rotation pools safely.'),
          ),
        );
      }
      return null;
    }
    if (!context.mounted) return null;
    final eligible = pools
        .where(
          (pool) =>
              pool.allowedHostIds.isEmpty ||
              pool.allowedHostIds.contains(host.hostId),
        )
        .toList(growable: false);
    if (eligible.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'No compatible rotation pool is configured for this host.',
          ),
        ),
      );
      return null;
    }
    return showModalBottomSheet<String>(
      context: context,
      showDragHandle: true,
      builder: (sheetContext) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          children: [
            const ListTile(
              title: Text('Choose rotation pool'),
              subtitle: Text(
                'Only non-secret policy metadata is shown. The connector validates profiles again before switching.',
              ),
            ),
            for (final pool in eligible)
              ListTile(
                leading: const Icon(Icons.account_tree_outlined),
                title: Text(pool.poolId),
                subtitle: Text(
                  '${pool.providerId} • ${pool.mode} • ${pool.orderedProfileIds.length} accounts',
                ),
                onTap: () => Navigator.of(sheetContext).pop(pool.poolId),
              ),
          ],
        ),
      ),
    );
  }

  Future<void> _configureRotationPool(
    BuildContext context,
    HostSyncState host,
    Map<String, Object?> sourceProfile,
  ) async {
    if (onUpsertRotationPool == null) return;
    final provider = _string(sourceProfile['provider']);
    final candidates = <Map<String, Object?>>[];
    for (final raw
        in host.snapshot['credentialProfiles'] as List? ?? const []) {
      if (raw is! Map) continue;
      final profile = Map<String, Object?>.from(raw);
      if (_string(profile['provider']) == provider && profile['status'] == 2) {
        candidates.add(profile);
      }
    }
    if (provider.isEmpty || candidates.length < 2) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'At least two active compatible accounts are required.',
          ),
        ),
      );
      return;
    }
    final selectedOrder = [
      for (final profile in candidates)
        if (_string(profile['profileId']).isNotEmpty)
          _string(profile['profileId']),
    ];
    final poolId = TextEditingController(
      text: '${provider.replaceAll(RegExp(r'[^a-zA-Z0-9_-]'), '-')}-rotation',
    );
    try {
      final draft = await showDialog<RotationPoolDraft>(
        context: context,
        builder: (dialogContext) {
          var mode = 'round_robin';
          return StatefulBuilder(
            builder: (dialogContext, setDialogState) => AlertDialog(
              title: const Text('Configure rotation pool'),
              content: SingleChildScrollView(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text(
                      'This saves non-secret policy only. The host validates all account references before saving.',
                    ),
                    TextField(
                      controller: poolId,
                      autocorrect: false,
                      enableSuggestions: false,
                      decoration: const InputDecoration(labelText: 'Pool ID'),
                    ),
                    DropdownButtonFormField<String>(
                      initialValue: mode,
                      decoration: const InputDecoration(labelText: 'Mode'),
                      items: const [
                        DropdownMenuItem(
                          value: 'manual',
                          child: Text('Manual'),
                        ),
                        DropdownMenuItem(
                          value: 'round_robin',
                          child: Text('Round robin'),
                        ),
                        DropdownMenuItem(
                          value: 'scheduled',
                          child: Text('Scheduled (connector policy)'),
                        ),
                        DropdownMenuItem(
                          value: 'confirmed_failure',
                          child: Text('Confirmed failure only'),
                        ),
                      ],
                      onChanged: (value) => setDialogState(() {
                        mode = value ?? mode;
                      }),
                    ),
                    const SizedBox(height: 8),
                    const Text('Active accounts in priority order'),
                    for (final profile in candidates)
                      Builder(
                        builder: (context) {
                          final id = _string(profile['profileId']);
                          final selectedIndex = selectedOrder.indexOf(id);
                          return CheckboxListTile(
                            dense: true,
                            contentPadding: EdgeInsets.zero,
                            controlAffinity: ListTileControlAffinity.leading,
                            value: selectedIndex >= 0,
                            title: Text(
                              _string(profile['displayName'], fallback: id),
                            ),
                            subtitle: Text(
                              _maskedFingerprint(profile['accountFingerprint']),
                            ),
                            secondary: selectedIndex < 0
                                ? null
                                : Row(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      IconButton(
                                        tooltip: 'Higher priority',
                                        onPressed: selectedIndex == 0
                                            ? null
                                            : () => setDialogState(() {
                                                final before =
                                                    selectedOrder[selectedIndex -
                                                        1];
                                                selectedOrder[selectedIndex -
                                                        1] =
                                                    id;
                                                selectedOrder[selectedIndex] =
                                                    before;
                                              }),
                                        icon: const Icon(Icons.arrow_upward),
                                      ),
                                      IconButton(
                                        tooltip: 'Lower priority',
                                        onPressed:
                                            selectedIndex ==
                                                selectedOrder.length - 1
                                            ? null
                                            : () => setDialogState(() {
                                                final after =
                                                    selectedOrder[selectedIndex +
                                                        1];
                                                selectedOrder[selectedIndex +
                                                        1] =
                                                    id;
                                                selectedOrder[selectedIndex] =
                                                    after;
                                              }),
                                        icon: const Icon(Icons.arrow_downward),
                                      ),
                                    ],
                                  ),
                            onChanged: (checked) => setDialogState(() {
                              if (checked == true && id.isNotEmpty) {
                                if (!selectedOrder.contains(id)) {
                                  selectedOrder.add(id);
                                }
                              } else {
                                selectedOrder.remove(id);
                              }
                            }),
                          );
                        },
                      ),
                  ],
                ),
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.of(dialogContext).pop(),
                  child: const Text('Cancel'),
                ),
                FilledButton(
                  onPressed: selectedOrder.length < 2
                      ? null
                      : () => Navigator.of(dialogContext).pop(
                          RotationPoolDraft(
                            poolId: poolId.text.trim(),
                            providerId: provider,
                            orderedProfileIds: List.unmodifiable(selectedOrder),
                            mode: mode,
                            cooldownMs: 60000,
                            maxSwitchesPerHour: 3,
                            allowedHostIds: [host.hostId],
                            quotaFailoverEnabled: false,
                          ),
                        ),
                  child: const Text('Save policy'),
                ),
              ],
            ),
          );
        },
      );
      if (draft != null && draft.poolId.isNotEmpty) {
        await onUpsertRotationPool!(host, draft);
      }
    } finally {
      poolId.dispose();
    }
  }
}

List<_AssignmentRow> _assignmentRows(Iterable<HostSyncState> hosts) {
  final rows = <_AssignmentRow>[];
  for (final host in hosts) {
    for (final raw in host.snapshot['runtimes'] as List? ?? const []) {
      if (raw is! Map) continue;
      final runtime = Map<String, Object?>.from(raw);
      final runtimeId = _string(runtime['runtimeId']);
      if (runtimeId.isEmpty) continue;
      final projects = <String>[];
      for (final rawPath in runtime['projectPaths'] as List? ?? const []) {
        final path = _string(rawPath);
        if (path.isNotEmpty) projects.add(path);
      }
      rows.add(
        _AssignmentRow(
          hostName: host.displayName,
          runtimeName: _string(runtime['name'], fallback: runtimeId),
          profileId: _string(runtime['activeCredentialProfileId']),
          projects: List.unmodifiable(projects),
          stale: !host.canMutate,
        ),
      );
    }
  }
  rows.sort(
    (left, right) => '${left.hostName}/${left.runtimeName}'.compareTo(
      '${right.hostName}/${right.runtimeName}',
    ),
  );
  return rows;
}

class _AssignmentMatrixCard extends StatelessWidget {
  const _AssignmentMatrixCard({
    required this.rows,
    required this.retryGroups,
    this.onRetryBulkSwitch,
  });

  final List<_AssignmentRow> rows;
  final List<BulkSwitchRetryGroup> retryGroups;
  final Future<void> Function(BulkSwitchRetryGroup group)? onRetryBulkSwitch;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Runtime assignment matrix',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 4),
            const Text(
              'Assignments affect new sessions only. Project paths are read-only source metadata.',
            ),
            const SizedBox(height: 8),
            if (retryGroups.isNotEmpty) ...[
              const Text(
                'Failed bulk-switch targets can be retried independently. '
                'Unknown outcomes are intentionally excluded.',
              ),
              for (final group in retryGroups)
                Align(
                  alignment: Alignment.centerLeft,
                  child: OutlinedButton.icon(
                    icon: const Icon(Icons.refresh),
                    label: Text(
                      'Retry ${group.failedTargets.length} failed target(s)',
                    ),
                    onPressed: onRetryBulkSwitch == null
                        ? null
                        : () {
                            onRetryBulkSwitch!(group);
                          },
                  ),
                ),
              const SizedBox(height: 8),
            ],
            if (rows.isEmpty)
              const Text('No runtime assignments are in the latest snapshots.')
            else
              for (final row in rows)
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  dense: true,
                  leading: const Icon(Icons.terminal),
                  title: Text('${row.hostName} · ${row.runtimeName}'),
                  subtitle: Text(
                    [
                      'profile ${row.profileId.isEmpty ? 'none' : row.profileId}',
                      if (row.projects.isEmpty)
                        'no known project'
                      else
                        row.projects.join(', '),
                      if (row.stale) 'STALE',
                    ].join(' • '),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
          ],
        ),
      ),
    );
  }
}

class _AssignmentRow {
  const _AssignmentRow({
    required this.hostName,
    required this.runtimeName,
    required this.profileId,
    required this.projects,
    required this.stale,
  });

  final String hostName;
  final String runtimeName;
  final String profileId;
  final List<String> projects;
  final bool stale;
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
