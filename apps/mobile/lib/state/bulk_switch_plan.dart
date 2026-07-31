import 'mobile_sync_state.dart';

/// A phone-local, read-only impact plan. It deliberately contains profile IDs
/// only, never credential material, and does not claim that an excluded or
/// offline host was changed.
class BulkCredentialSwitchPlan {
  const BulkCredentialSwitchPlan({
    required this.ready,
    required this.alreadyAssigned,
    required this.offline,
    required this.missingProfile,
  });

  final List<BulkCredentialSwitchTarget> ready;
  final List<BulkCredentialSwitchTarget> alreadyAssigned;
  final List<BulkCredentialSwitchTarget> offline;
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
        } else {
          ready.add(target);
        }
      }
    }
    return BulkCredentialSwitchPlan(
      ready: List.unmodifiable(ready),
      alreadyAssigned: List.unmodifiable(alreadyAssigned),
      offline: List.unmodifiable(offline),
      missingProfile: List.unmodifiable(missingProfile),
    );
  }
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
