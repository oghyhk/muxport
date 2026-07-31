import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/screens/credential_matrix_screen.dart';
import 'package:muxport_mobile/state/bulk_switch_plan.dart';
import 'package:muxport_mobile/state/mobile_sync_state.dart';

void main() {
  testWidgets('previews and selects independent bulk credential targets', (
    tester,
  ) async {
    HostSyncState host(String id) => HostSyncState(
      hostId: id,
      pinnedHostKey: 'pin-$id',
      displayName: 'Host $id',
      protocolVersion: mobileProtocolVersion,
      phase: HostSyncPhase.synchronized,
      snapshot: const {
        'credentialProfiles': [
          {
            'profileId': 'profile-a',
            'displayName': 'OpenCode Go',
            'provider': 'opencode-go',
            'accountFingerprint': 'account-fingerprint',
          },
        ],
        'runtimes': [
          {
            'runtimeId': 'runtime-a',
            'name': 'OpenCode',
            'activeCredentialProfileId': 'profile-old',
            'connectorManaged': true,
          },
        ],
      },
      cursor: null,
      sourceVersions: const {},
      recentEventIds: const [],
      pendingOperations: const {},
    );
    List<BulkCredentialSwitchTarget>? dispatched;
    await tester.pumpWidget(
      MaterialApp(
        home: CredentialMatrixScreen(
          hosts: [host('one'), host('two')],
          onBulkAssignCredentials: (targets) async {
            dispatched = targets;
          },
        ),
      ),
    );

    await tester.tap(find.byTooltip('Credential actions').first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Preview multi-host switch'));
    await tester.pumpAndSettle();

    expect(find.text('Ready to switch: 2'), findsOneWidget);
    expect(find.text('Switch 2'), findsOneWidget);
    await tester.tap(find.text('Switch 2'));
    await tester.pumpAndSettle();

    expect(dispatched, hasLength(2));
    expect(dispatched!.map((target) => target.host.hostId), {'one', 'two'});
  });
}
