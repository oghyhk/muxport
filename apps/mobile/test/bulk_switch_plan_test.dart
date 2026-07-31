import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/state/bulk_switch_plan.dart';
import 'package:muxport_mobile/state/mobile_sync_state.dart';

void main() {
  test(
    'impact plan separates ready, offline, missing, and unchanged targets',
    () {
      HostSyncState host(
        String id,
        HostSyncPhase phase,
        Object? profile,
        String assigned,
      ) => HostSyncState(
        hostId: id,
        pinnedHostKey: 'key-$id',
        displayName: id,
        protocolVersion: mobileProtocolVersion,
        phase: phase,
        snapshot: {
          'credentialProfiles': profile == null ? [] : [profile],
          'runtimes': [
            {'runtimeId': 'runtime-$id', 'activeCredentialProfileId': assigned},
          ],
        },
        cursor: null,
        sourceVersions: const {},
        recentEventIds: const [],
        pendingOperations: const {},
      );
      const profile = {
        'profileId': 'profile-a',
        'provider': 'opencode-go',
        'accountFingerprint': 'fp-a',
      };
      final plan = BulkCredentialSwitchPlan.build(
        hosts: [
          host('ready', HostSyncPhase.synchronized, profile, 'old'),
          host('same', HostSyncPhase.synchronized, profile, 'profile-a'),
          host('offline', HostSyncPhase.offline, profile, 'old'),
          host('missing', HostSyncPhase.synchronized, null, 'old'),
        ],
        provider: 'opencode-go',
        accountFingerprint: 'fp-a',
      );
      expect(plan.ready, hasLength(1));
      expect(plan.alreadyAssigned, hasLength(1));
      expect(plan.offline, hasLength(1));
      expect(plan.missingProfile, hasLength(1));
    },
  );
}
