import 'package:flutter/material.dart';

import '../state/mobile_sync_state.dart';

class CredentialMatrixScreen extends StatelessWidget {
  const CredentialMatrixScreen({
    required this.hosts,
    this.onProvisionCredential,
    this.onAssignCredential,
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
                    trailing: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.end,
                      children: [
                        Text(status),
                        if (!item.host.canMutate)
                          const Text(
                            'STALE',
                            style: TextStyle(color: Colors.amber, fontSize: 10),
                          ),
                      ],
                    ),
                  ),
                );
              },
            ),
    );
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
              ListTile(
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
                    'current: ${_string(runtime['activeCredentialProfileId'], fallback: 'none')}',
                  ].join(' • '),
                ),
                onTap: () => Navigator.of(
                  sheetContext,
                ).pop(_string(runtime['runtimeId'])),
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
