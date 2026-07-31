import 'mobile_sync_state.dart';

class MobileLifecycleReducer {
  const MobileLifecycleReducer._();

  static Map<String, HostSyncState> prepareForeground({
    required Iterable<HostSyncState> hosts,
    required bool canAuthenticateTransport,
  }) {
    return {
      for (final host in hosts)
        host.hostId:
            canAuthenticateTransport &&
                host.canReconnect &&
                host.phase != HostSyncPhase.pairingPending &&
                host.phase != HostSyncPhase.identityMismatch &&
                host.phase != HostSyncPhase.protocolIncompatible
            ? host.withPhase(HostSyncPhase.reconnecting)
            : host,
    };
  }

  static Map<String, HostSyncState> prepareSuspension(
    Iterable<HostSyncState> hosts,
  ) {
    return {
      for (final host in hosts)
        host.hostId:
            host.phase == HostSyncPhase.synchronized ||
                host.phase == HostSyncPhase.reconnecting ||
                host.phase == HostSyncPhase.replaying
            ? host.withPhase(HostSyncPhase.cachedStale)
            : host,
    };
  }
}
