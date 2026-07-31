import 'dart:convert';

import 'mobile_sync_state.dart';

const _bulkSwitchKindPrefix = 'bulk-switch:';

/// A phone-local, read-only impact plan. It deliberately contains profile IDs
/// only, never credential material, and does not claim that an excluded or
/// offline host was changed.
class BulkCredentialSwitchPlan {
  const BulkCredentialSwitchPlan({
    required this.ready,
    required this.alreadyAssigned,
    required this.offline,
    required this.busy,
    required this.missingProfile,
  });

  final List<BulkCredentialSwitchTarget> ready;
  final List<BulkCredentialSwitchTarget> alreadyAssigned;
  final List<BulkCredentialSwitchTarget> offline;
  final List<BulkCredentialSwitchTarget> busy;
  final List<BulkCredentialSwitchTarget> missingProfile;

  List<BulkCredentialSwitchTarget> get dispatchable => ready;

  static BulkCredentialSwitchPlan build({
    required Iterable<HostSyncState> hosts,
    required String provider,
    required String accountFingerprint,
  }) {
    final ready = <BulkCredentialSwitchTarget>[];
    final alreadyAssigned = <BulkCredentialSwitchTarget>[];
    final offline = <BulkCredentialSwitchTarget>[];
    final busy = <BulkCredentialSwitchTarget>[];
    final missingProfile = <BulkCredentialSwitchTarget>[];
    for (final host in hosts) {
      Map<String, Object?>? profile;
      for (final raw
          in host.snapshot['credentialProfiles'] as List? ?? const []) {
        if (raw is! Map) continue;
        final candidate = Map<String, Object?>.from(raw);
        if (candidate['provider'] == provider &&
            candidate['accountFingerprint'] == accountFingerprint) {
          profile = candidate;
          break;
        }
      }
      final profileId = profile?['profileId'] is String
          ? profile!['profileId'] as String
          : '';
      for (final raw in host.snapshot['runtimes'] as List? ?? const []) {
        if (raw is! Map) continue;
        final runtime = Map<String, Object?>.from(raw);
        final runtimeId = runtime['runtimeId'] is String
            ? runtime['runtimeId'] as String
            : '';
        if (runtimeId.isEmpty) continue;
        final target = BulkCredentialSwitchTarget(
          host: host,
          runtimeId: runtimeId,
          credentialProfileId: profileId,
        );
        if (profileId.isEmpty) {
          missingProfile.add(target);
        } else if (!host.canMutate) {
          offline.add(target);
        } else if (runtime['activeCredentialProfileId'] == profileId) {
          alreadyAssigned.add(target);
        } else if (_hasActiveWork(host, runtimeId)) {
          busy.add(target);
        } else {
          ready.add(target);
        }
      }
    }
    return BulkCredentialSwitchPlan(
      ready: List.unmodifiable(ready),
      alreadyAssigned: List.unmodifiable(alreadyAssigned),
      offline: List.unmodifiable(offline),
      busy: List.unmodifiable(busy),
      missingProfile: List.unmodifiable(missingProfile),
    );
  }
}

bool _hasActiveWork(HostSyncState host, String runtimeId) {
  for (final raw in host.snapshot['activeSessions'] as List? ?? const []) {
    if (raw is Map && raw['runtimeId'] == runtimeId) {
      return true;
    }
  }
  return false;
}

class BulkCredentialSwitchTarget {
  const BulkCredentialSwitchTarget({
    required this.host,
    required this.runtimeId,
    required this.credentialProfileId,
  });
  final HostSyncState host;
  final String runtimeId;
  final String credentialProfileId;
}

/// Non-secret metadata embedded in each child operation. Keeping the original
/// group id with every child lets a restarted phone identify only the targets
/// that conclusively failed; outcome-unknown children are never retried.
String bulkSwitchOperationKind({
  required String groupId,
  required String runtimeId,
  required String credentialProfileId,
}) {
  final encoded = base64Url.encode(
    utf8.encode(
      jsonEncode({
        'groupId': groupId,
        'runtimeId': runtimeId,
        'credentialProfileId': credentialProfileId,
      }),
    ),
  );
  return '$_bulkSwitchKindPrefix$encoded';
}

BulkSwitchOperationRef? parseBulkSwitchOperationKind(String kind) {
  if (!kind.startsWith(_bulkSwitchKindPrefix)) return null;
  try {
    final decoded = jsonDecode(
      utf8.decode(
        base64Url.decode(kind.substring(_bulkSwitchKindPrefix.length)),
      ),
    );
    if (decoded is! Map) return null;
    final groupId = decoded['groupId'];
    final runtimeId = decoded['runtimeId'];
    final credentialProfileId = decoded['credentialProfileId'];
    if (groupId is! String ||
        groupId.isEmpty ||
        runtimeId is! String ||
        runtimeId.isEmpty ||
        credentialProfileId is! String ||
        credentialProfileId.isEmpty) {
      return null;
    }
    return BulkSwitchOperationRef(
      groupId: groupId,
      runtimeId: runtimeId,
      credentialProfileId: credentialProfileId,
    );
  } on FormatException {
    return null;
  }
}

class BulkSwitchOperationRef {
  const BulkSwitchOperationRef({
    required this.groupId,
    required this.runtimeId,
    required this.credentialProfileId,
  });

  final String groupId;
  final String runtimeId;
  final String credentialProfileId;
}

class BulkSwitchRetryGroup {
  const BulkSwitchRetryGroup({
    required this.groupId,
    required this.failedTargets,
  });

  final String groupId;
  final List<BulkCredentialSwitchTarget> failedTargets;

  static List<BulkSwitchRetryGroup> failedFromHosts(
    Iterable<HostSyncState> hosts,
  ) {
    final grouped = <String, List<BulkCredentialSwitchTarget>>{};
    for (final host in hosts) {
      for (final operation in host.pendingOperations.values) {
        if (operation.state != MobileOperationState.failed) continue;
        final ref = parseBulkSwitchOperationKind(operation.kind);
        if (ref == null) continue;
        grouped
            .putIfAbsent(ref.groupId, () => <BulkCredentialSwitchTarget>[])
            .add(
              BulkCredentialSwitchTarget(
                host: host,
                runtimeId: ref.runtimeId,
                credentialProfileId: ref.credentialProfileId,
              ),
            );
      }
    }
    final groups = [
      for (final entry in grouped.entries)
        BulkSwitchRetryGroup(
          groupId: entry.key,
          failedTargets: List.unmodifiable(entry.value),
        ),
    ];
    groups.sort((left, right) => left.groupId.compareTo(right.groupId));
    return List.unmodifiable(groups);
  }
}
