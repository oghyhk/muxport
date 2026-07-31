// This is a basic Flutter widget test.
//
// To perform an interaction with a widget in your test, use the WidgetTester
// utility in the flutter_test package. For example, you can send tap and scroll
// gestures. You can also use WidgetTester to find child widgets in the widget
// tree, read text, and verify that the values of widget properties are correct.

import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/material.dart';

import 'package:muxport_mobile/main.dart';
import 'package:muxport_mobile/screens/session_timeline_screen.dart';
import 'package:muxport_mobile/state/app_bootstrap.dart';
import 'package:muxport_mobile/state/mobile_cache_store.dart';
import 'package:muxport_mobile/state/mobile_sync_state.dart';

void main() {
  testWidgets('opens the host fleet and switches to sessions', (tester) async {
    await tester.pumpWidget(
      MuxportApp(bootstrap: Future.value(AppBootstrapState.emptyForTest())),
    );
    await tester.pumpAndSettle();

    expect(find.text('Host Fleet'), findsOneWidget);
    expect(find.text('Hosts'), findsOneWidget);
    expect(find.text('No cached hosts'), findsOneWidget);
    expect(
      find.textContaining('protected phone identity is locked'),
      findsOneWidget,
    );

    await tester.tap(find.text('Sessions'));
    await tester.pumpAndSettle();

    expect(find.text('Unified Session Timeline'), findsOneWidget);
  });

  testWidgets('renders cached hosts as stale and non-mutable', (tester) async {
    final host = HostSyncState(
      hostId: 'host-1',
      pinnedHostKey: 'public-key',
      displayName: 'Cached development host',
      protocolVersion: mobileProtocolVersion,
      phase: HostSyncPhase.cachedStale,
      snapshot: const {'sessionCount': 2},
      cursor: const SyncCursor(hostEpoch: 'epoch-a', sequence: 8),
      sourceVersions: const {},
      recentEventIds: const [],
      pendingOperations: {
        'approval-1': PendingOperation.local(
          idempotencyKey: 'approval-1',
          kind: 'approval',
          createdAtMs: 100,
          deadlineMs: 500,
        ),
      },
    );
    final bootstrap = AppBootstrapState(
      cache: MobileCacheSnapshot(hosts: [host]),
      cacheGeneration: 3,
      cacheStatus: CacheBootstrapStatus.ready,
      identity: null,
      identityStatus: IdentityBootstrapStatus.unavailable,
    );

    await tester.pumpWidget(MuxportApp(bootstrap: Future.value(bootstrap)));
    await tester.pumpAndSettle();

    expect(find.text('Cached development host'), findsOneWidget);
    expect(find.text('Cached · stale'), findsOneWidget);
    expect(find.text('1 operation(s) require reconciliation'), findsOneWidget);
    expect(find.textContaining('saved local state'), findsOneWidget);
  });

  testWidgets('renders multiple independently paired hosts', (tester) async {
    HostSyncState host(String id, String name, int sequence) => HostSyncState(
      hostId: id,
      pinnedHostKey: 'pin-$id',
      displayName: name,
      protocolVersion: mobileProtocolVersion,
      phase: HostSyncPhase.cachedStale,
      snapshot: const {},
      cursor: SyncCursor(hostEpoch: 'epoch-$id', sequence: sequence),
      sourceVersions: const {},
      recentEventIds: const [],
      pendingOperations: const {},
    );
    final bootstrap = AppBootstrapState(
      cache: MobileCacheSnapshot(
        hosts: [
          host('host-laptop', 'Development laptop', 4),
          host('host-vps', 'Remote VPS', 12),
        ],
      ),
      cacheGeneration: 2,
      cacheStatus: CacheBootstrapStatus.ready,
      identity: null,
      identityStatus: IdentityBootstrapStatus.unavailable,
    );

    await tester.pumpWidget(MuxportApp(bootstrap: Future.value(bootstrap)));
    await tester.pumpAndSettle();

    expect(find.text('Development laptop'), findsOneWidget);
    expect(find.text('Remote VPS'), findsOneWidget);
    expect(find.text('Cached cursor: epoch-host-laptop / 4'), findsOneWidget);
    expect(find.text('Cached cursor: epoch-host-vps / 12'), findsOneWidget);
  });

  testWidgets('diagnostics never claims unwired controls are healthy', (
    tester,
  ) async {
    await tester.pumpWidget(
      MuxportApp(bootstrap: Future.value(AppBootstrapState.emptyForTest())),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Diagnostics'));
    await tester.pumpAndSettle();

    expect(find.text('NOT CONNECTED'), findsOneWidget);
    expect(find.text('NOT RUN'), findsOneWidget);
    expect(find.textContaining('ONLINE'), findsNothing);
    expect(find.textContaining('0 SECRETS'), findsNothing);
  });

  testWidgets('renders synchronized session approval and account projections', (
    tester,
  ) async {
    final host = HostSyncState(
      hostId: 'host-1',
      pinnedHostKey: 'public-key',
      displayName: 'Development VPS',
      protocolVersion: mobileProtocolVersion,
      phase: HostSyncPhase.synchronized,
      snapshot: const {
        'connectorState': 4,
        'runtimes': [
          {
            'runtimeId': 'codex-managed-work',
            'name': 'Codex',
            'state': 7,
            'activeCredentialProfileId': 'work-account',
          },
        ],
        'activeSessions': [
          {
            'sessionId': 'session-1',
            'runtimeId': 'codex-managed-work',
            'projectPath': '/srv/project',
            'title': 'Implement sync',
            'credentialProfileId': 'work-account',
            'status': 'waiting_approval',
          },
        ],
        'credentialProfiles': [
          {
            'profileId': 'work-account',
            'displayName': 'Work account',
            'provider': 'openai',
            'accountFingerprint': 'fingerprint-1234567890',
            'status': 2,
            'lastValidatedAtMs': 10,
          },
        ],
        'recentEvents': [
          {
            'eventId': 'event-1',
            'timestampMs': 10,
            'kind': 'approvalRequested',
            'approvalId': 'approval-1234567890',
            'sessionId': 'session-1',
            'actionType': 'command',
          },
        ],
      },
      cursor: const SyncCursor(hostEpoch: '1', sequence: 10),
      sourceVersions: const {},
      recentEventIds: const [],
      pendingOperations: const {},
    );
    final bootstrap = AppBootstrapState(
      cache: MobileCacheSnapshot(hosts: [host]),
      cacheGeneration: 1,
      cacheStatus: CacheBootstrapStatus.ready,
      identity: null,
      identityStatus: IdentityBootstrapStatus.unavailable,
    );
    await tester.pumpWidget(MuxportApp(bootstrap: Future.value(bootstrap)));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Sessions'));
    await tester.pumpAndSettle();
    expect(find.text('Implement sync'), findsOneWidget);
    expect(find.textContaining('profile work-account'), findsOneWidget);

    await tester.tap(find.text('Approvals'));
    await tester.pumpAndSettle();
    expect(find.text('command approval'), findsOneWidget);
    expect(find.text('Session session-1'), findsOneWidget);

    await tester.tap(find.text('Accounts'));
    await tester.pumpAndSettle();
    expect(find.text('Work account'), findsOneWidget);
    expect(find.textContaining('assigned to Codex'), findsOneWidget);
    expect(
      find.textContaining('validated 1970-01-01T00:00:00.010Z'),
      findsOneWidget,
    );
    expect(find.text('Active'), findsOneWidget);
  });

  testWidgets('opens recent bounded session output from the action sheet', (
    tester,
  ) async {
    final host = HostSyncState(
      hostId: 'host-1',
      pinnedHostKey: 'public-key',
      displayName: 'Development VPS',
      protocolVersion: mobileProtocolVersion,
      phase: HostSyncPhase.synchronized,
      snapshot: const {
        'activeSessions': [
          {
            'sessionId': 'session-1',
            'runtimeId': 'codex-work',
            'projectPath': '/srv/project',
            'title': 'Live task',
            'credentialProfileId': 'work-account',
            'status': 'running',
          },
        ],
        'recentEvents': [
          {
            'kind': 'streamDelta',
            'sessionId': 'session-1',
            'deltaText': 'Live response from the remote runtime.',
          },
        ],
      },
      cursor: const SyncCursor(hostEpoch: '1', sequence: 10),
      sourceVersions: const {},
      recentEventIds: const [],
      pendingOperations: const {},
    );
    await tester.pumpWidget(
      MaterialApp(
        home: SessionTimelineScreen(
          hosts: [host],
          onSendInput: (_, _, _, _) async {},
        ),
      ),
    );
    await tester.tap(find.text('Live task'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('View recent output'));
    await tester.pumpAndSettle();
    expect(find.text('Live response from the remote runtime.'), findsOneWidget);
  });
}
