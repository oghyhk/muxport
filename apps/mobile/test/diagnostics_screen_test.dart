import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/screens/diagnostics_screen.dart';
import 'package:muxport_mobile/state/app_bootstrap.dart';
import 'package:muxport_mobile/state/command_audit.dart';
import 'package:muxport_mobile/state/mobile_sync_state.dart';

void main() {
  testWidgets('loads and renders a redacted connector security log', (
    tester,
  ) async {
    final host = HostSyncState(
      hostId: 'host-1',
      pinnedHostKey: 'public-key',
      displayName: 'Development host',
      protocolVersion: mobileProtocolVersion,
      phase: HostSyncPhase.synchronized,
      snapshot: const {},
      cursor: null,
      sourceVersions: const {},
      recentEventIds: const [],
      pendingOperations: const {},
    );
    const entry = CommandAuditEntry(
      actorFingerprint:
          'audit:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
      action: 'change_assignment',
      target: 'assignment_target',
      outcome: 'succeeded',
      completedAtMs: 100,
    );
    await tester.pumpWidget(
      MaterialApp(
        home: DiagnosticsScreen(
          bootstrap: AppBootstrapState.emptyForTest(),
          hosts: [host],
          onLoadCommandAudit: (_) async => [entry],
        ),
      ),
    );

    await tester.tap(find.text('Refresh connector security log'));
    await tester.pumpAndSettle();

    expect(find.text('1 REDACTED RECORDS'), findsOneWidget);
    expect(find.text('Development host security log'), findsOneWidget);
    await tester.tap(find.text('Development host security log'));
    await tester.pumpAndSettle();
    expect(find.text('change_assignment · succeeded'), findsOneWidget);
    expect(
      find.textContaining('assignment_target · device 456789abcdef'),
      findsOneWidget,
    );
  });
}
