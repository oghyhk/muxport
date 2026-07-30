// This is a basic Flutter widget test.
//
// To perform an interaction with a widget in your test, use the WidgetTester
// utility in the flutter_test package. For example, you can send tap and scroll
// gestures. You can also use WidgetTester to find child widgets in the widget
// tree, read text, and verify that the values of widget properties are correct.

import 'package:flutter_test/flutter_test.dart';

import 'package:muxport_mobile/main.dart';
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
    expect(find.text('Cached • stale'), findsOneWidget);
    expect(find.text('1 operation(s) require reconciliation'), findsOneWidget);
    expect(
      find.textContaining('Remote controls remain disabled'),
      findsOneWidget,
    );
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
}
