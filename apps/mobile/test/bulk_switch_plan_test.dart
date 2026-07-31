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
          HostSyncState(
            hostId: 'busy',
            pinnedHostKey: 'key-busy',
            displayName: 'busy',
            protocolVersion: mobileProtocolVersion,
            phase: HostSyncPhase.synchronized,
            snapshot: const {
              'credentialProfiles': [profile],
              'runtimes': [
                {
                  'runtimeId': 'runtime-busy',
                  'activeCredentialProfileId': 'old',
                },
              ],
              'activeSessions': [
                {'runtimeId': 'runtime-busy', 'sessionId': 'session-busy'},
              ],
            },
            cursor: null,
            sourceVersions: const {},
            recentEventIds: const [],
            pendingOperations: const {},
          ),
          host('missing', HostSyncPhase.synchronized, null, 'old'),
        ],
        provider: 'opencode-go',
        accountFingerprint: 'fp-a',
      );
      expect(plan.ready, hasLength(1));
      expect(plan.alreadyAssigned, hasLength(1));
      expect(plan.offline, hasLength(1));
      expect(plan.busy, hasLength(1));
      expect(plan.missingProfile, hasLength(1));
    },
  );

  test(
    'preserves group identity and retries only conclusively failed children',
    () {
      final kind = bulkSwitchOperationKind(
        groupId: 'group-1',
        runtimeId: 'runtime-1',
        credentialProfileId: 'profile-1',
      );
      final ref = parseBulkSwitchOperationKind(kind);
      expect(ref?.groupId, 'group-1');
      expect(ref?.runtimeId, 'runtime-1');
      expect(ref?.credentialProfileId, 'profile-1');

      final host = HostSyncState(
        hostId: 'host-1',
        pinnedHostKey: 'key',
        displayName: 'Host',
        protocolVersion: mobileProtocolVersion,
        phase: HostSyncPhase.synchronized,
        snapshot: const {},
        cursor: null,
        sourceVersions: const {},
        recentEventIds: const [],
        pendingOperations: {
          'failed': PendingOperation(
            idempotencyKey: 'failed',
            kind: kind,
            createdAtMs: 1,
            deadlineMs: 2,
            state: MobileOperationState.failed,
          ),
          'unknown': PendingOperation(
            idempotencyKey: 'unknown',
            kind: kind,
            createdAtMs: 1,
            deadlineMs: 2,
            state: MobileOperationState.reconciliationRequired,
          ),
        },
      );
      final groups = BulkSwitchRetryGroup.failedFromHosts([host]);
      expect(groups, hasLength(1));
      expect(groups.single.groupId, 'group-1');
      expect(groups.single.failedTargets, hasLength(1));
      expect(groups.single.failedTargets.single.runtimeId, 'runtime-1');
    },
  );

  test('excludes incompatible and locked targets before any dispatch', () {
    const targetProfile = {
      'profileId': 'target',
      'provider': 'opencode-go',
      'accountFingerprint': 'fp',
      'status': 2,
    };
    HostSyncState host(String id, Map<String, Object?> snapshot) =>
        HostSyncState(
          hostId: id,
          pinnedHostKey: 'pin-$id',
          displayName: id,
          protocolVersion: mobileProtocolVersion,
          phase: HostSyncPhase.synchronized,
          snapshot: snapshot,
          cursor: null,
          sourceVersions: const {},
          recentEventIds: const [],
          pendingOperations: const {},
        );
    final plan = BulkCredentialSwitchPlan.build(
      hosts: [
        host('incompatible', const {
          'credentialProfiles': [
            targetProfile,
            {'profileId': 'old', 'provider': 'openai'},
          ],
          'runtimes': [
            {'runtimeId': 'r1', 'activeCredentialProfileId': 'old'},
          ],
        }),
        host('locked', const {
          'connectorState': 2,
          'credentialProfiles': [targetProfile],
          'runtimes': [
            {'runtimeId': 'r2', 'activeCredentialProfileId': 'old'},
          ],
        }),
      ],
      provider: 'opencode-go',
      accountFingerprint: 'fp',
    );
    expect(plan.ready, isEmpty);
    expect(plan.incompatible, hasLength(1));
    expect(plan.locked, hasLength(1));
  });
}
