import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/state/mobile_lifecycle.dart';
import 'package:muxport_mobile/state/mobile_sync_state.dart';

void main() {
  test(
    'foreground marks only authenticated reconnectable hosts reconnecting',
    () {
      final reconnectable = _host(
        id: 'online',
        phase: HostSyncPhase.cachedStale,
        directAddress: '127.0.0.1',
        directPort: 45821,
      );
      final localOnly = _host(id: 'local', phase: HostSyncPhase.cachedStale);

      final prepared = MobileLifecycleReducer.prepareForeground(
        hosts: [reconnectable, localOnly],
        canAuthenticateTransport: true,
      );

      expect(prepared['online']?.phase, HostSyncPhase.reconnecting);
      expect(prepared['local']?.phase, HostSyncPhase.cachedStale);
      expect(
        MobileLifecycleReducer.prepareForeground(
          hosts: [reconnectable],
          canAuthenticateTransport: false,
        )['online']?.phase,
        HostSyncPhase.cachedStale,
      );
    },
  );

  test('suspension makes live projections stale without losing cursors', () {
    final live = _host(id: 'live', phase: HostSyncPhase.synchronized);
    final offline = _host(id: 'offline', phase: HostSyncPhase.offline);

    final suspended = MobileLifecycleReducer.prepareSuspension([live, offline]);

    expect(suspended['live']?.phase, HostSyncPhase.cachedStale);
    expect(
      suspended['live']?.cursor,
      const SyncCursor(hostEpoch: 'epoch-live', sequence: 19),
    );
    expect(suspended['offline']?.phase, HostSyncPhase.offline);
  });
}

HostSyncState _host({
  required String id,
  required HostSyncPhase phase,
  String? directAddress,
  int? directPort,
}) {
  return HostSyncState(
    hostId: id,
    pinnedHostKey: 'pin-$id',
    displayName: id,
    protocolVersion: mobileProtocolVersion,
    phase: phase,
    directAddress: directAddress,
    directPort: directPort,
    snapshot: const {},
    cursor: const SyncCursor(hostEpoch: 'epoch-live', sequence: 19),
    sourceVersions: const {},
    recentEventIds: const [],
    pendingOperations: const {},
  );
}
