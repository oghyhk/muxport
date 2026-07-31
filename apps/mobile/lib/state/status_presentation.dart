import 'package:flutter_protocol/flutter_protocol.dart';

import 'mobile_sync_state.dart';

enum StatusTone { neutral, progress, healthy, warning, danger }

class StatusPresentation {
  const StatusPresentation({
    required this.label,
    required this.explanation,
    required this.tone,
  });

  final String label;
  final String explanation;
  final StatusTone tone;
}

StatusPresentation connectorStatusPresentation(Object? raw) {
  return switch (raw) {
    1 => const StatusPresentation(
      label: 'Starting',
      explanation:
          'The connector is initializing and is not accepting remote changes yet.',
      tone: StatusTone.progress,
    ),
    2 => const StatusPresentation(
      label: 'Vault locked',
      explanation:
          'Credentials are unavailable. Unlock the host vault before starting authenticated work.',
      tone: StatusTone.warning,
    ),
    3 => const StatusPresentation(
      label: 'Recovering',
      explanation:
          'The connector is verifying durable state and rebuilding truth from each runtime.',
      tone: StatusTone.progress,
    ),
    4 => const StatusPresentation(
      label: 'Ready',
      explanation:
          'The connector is synchronized and can accept supported remote operations.',
      tone: StatusTone.healthy,
    ),
    5 => const StatusPresentation(
      label: 'Degraded',
      explanation:
          'Some runtime or transport state is unavailable. Review the affected runtime before changing it.',
      tone: StatusTone.warning,
    ),
    6 => const StatusPresentation(
      label: 'Fatal configuration error',
      explanation:
          'The connector cannot safely start with its current configuration and requires host-side repair.',
      tone: StatusTone.danger,
    ),
    _ => const StatusPresentation(
      label: 'Unknown connector state',
      explanation:
          'The host did not provide a recognized connector state. Remote changes remain unsafe.',
      tone: StatusTone.neutral,
    ),
  };
}

StatusPresentation runtimeStatusPresentation(Object? raw) {
  return switch (raw) {
    1 => const StatusPresentation(
      label: 'Unknown',
      explanation: 'This runtime has not been authoritatively discovered yet.',
      tone: StatusTone.neutral,
    ),
    2 => const StatusPresentation(
      label: 'Discovering',
      explanation: 'Muxport is locating and identifying the runtime.',
      tone: StatusTone.progress,
    ),
    3 => const StatusPresentation(
      label: 'Starting',
      explanation: 'The managed runtime process is starting.',
      tone: StatusTone.progress,
    ),
    4 => const StatusPresentation(
      label: 'Synchronizing',
      explanation:
          'Muxport is reading account, project, and session truth before enabling changes.',
      tone: StatusTone.progress,
    ),
    5 => const StatusPresentation(
      label: 'Online · idle',
      explanation: 'The runtime is reachable and has no active work.',
      tone: StatusTone.healthy,
    ),
    6 => const StatusPresentation(
      label: 'Online · running',
      explanation: 'The runtime is reachable and actively processing work.',
      tone: StatusTone.healthy,
    ),
    7 => const StatusPresentation(
      label: 'Waiting for approval',
      explanation:
          'The runtime is paused until a user responds to an approval request.',
      tone: StatusTone.warning,
    ),
    8 => const StatusPresentation(
      label: 'Degraded',
      explanation:
          'The runtime is only partially available or its latest snapshot is stale.',
      tone: StatusTone.warning,
    ),
    9 => const StatusPresentation(
      label: 'Stopped',
      explanation: 'The runtime is not running and will not receive new work.',
      tone: StatusTone.neutral,
    ),
    10 => const StatusPresentation(
      label: 'Crashed',
      explanation:
          'The runtime exited unexpectedly. Its last in-flight result may be unknown.',
      tone: StatusTone.danger,
    ),
    11 => const StatusPresentation(
      label: 'Crash loop',
      explanation:
          'Automatic restarts stopped after repeated failures. Host-side attention is required.',
      tone: StatusTone.danger,
    ),
    12 => const StatusPresentation(
      label: 'Credential locked',
      explanation:
          'The assigned credential is unavailable, so authenticated work is blocked.',
      tone: StatusTone.warning,
    ),
    _ => const StatusPresentation(
      label: 'Unknown runtime state',
      explanation:
          'The host supplied no recognized runtime state. Muxport will wait for a fresh snapshot.',
      tone: StatusTone.neutral,
    ),
  };
}

StatusPresentation operationStatusPresentation(MobileOperationState state) {
  return switch (state) {
    MobileOperationState.pending => const StatusPresentation(
      label: 'Pending',
      explanation: 'The connector has not yet confirmed the operation outcome.',
      tone: StatusTone.progress,
    ),
    MobileOperationState.connectorAcknowledged => const StatusPresentation(
      label: 'Confirming',
      explanation:
          'The connector accepted the operation and is verifying the source runtime outcome.',
      tone: StatusTone.progress,
    ),
    MobileOperationState.succeeded => const StatusPresentation(
      label: 'Succeeded',
      explanation: 'The source runtime confirmed the requested result.',
      tone: StatusTone.healthy,
    ),
    MobileOperationState.failed => const StatusPresentation(
      label: 'Failed',
      explanation:
          'The operation did not complete successfully and was not assumed to have taken effect.',
      tone: StatusTone.danger,
    ),
    MobileOperationState.expired => const StatusPresentation(
      label: 'Expired',
      explanation:
          'The operation deadline passed. Muxport will not execute it later.',
      tone: StatusTone.warning,
    ),
    MobileOperationState.reconciliationRequired => const StatusPresentation(
      label: 'Outcome unknown',
      explanation:
          'Connectivity was lost after dispatch. Muxport must read back source state before retrying.',
      tone: StatusTone.warning,
    ),
  };
}

StatusPresentation remoteOperationStatusPresentation(RemoteOpState state) {
  return switch (state) {
    RemoteOpState.unspecified => const StatusPresentation(
      label: 'Outcome unknown',
      explanation:
          'The connector returned no recognized operation state. Reconciliation is required.',
      tone: StatusTone.warning,
    ),
    RemoteOpState.created => const StatusPresentation(
      label: 'Created',
      explanation:
          'The operation exists locally but has not yet been durably reserved by the connector.',
      tone: StatusTone.progress,
    ),
    RemoteOpState.persisted => const StatusPresentation(
      label: 'Persisted',
      explanation:
          'The connector durably reserved the operation before runtime dispatch.',
      tone: StatusTone.progress,
    ),
    RemoteOpState.dispatched => const StatusPresentation(
      label: 'Dispatched',
      explanation:
          'The request reached the runtime adapter, but its source outcome is not confirmed yet.',
      tone: StatusTone.progress,
    ),
    RemoteOpState.sourceAcknowledged => const StatusPresentation(
      label: 'Source acknowledged',
      explanation:
          'The source runtime accepted the request and Muxport is verifying readback.',
      tone: StatusTone.progress,
    ),
    RemoteOpState.reconciled => const StatusPresentation(
      label: 'Reconciled',
      explanation:
          'Source state was read back after dispatch and the final result is being recorded.',
      tone: StatusTone.progress,
    ),
    RemoteOpState.succeeded => const StatusPresentation(
      label: 'Succeeded',
      explanation: 'The source runtime confirmed the requested result.',
      tone: StatusTone.healthy,
    ),
    RemoteOpState.expired => const StatusPresentation(
      label: 'Expired',
      explanation:
          'The deadline passed before safe execution. The request will not run later.',
      tone: StatusTone.warning,
    ),
    RemoteOpState.cancelled => const StatusPresentation(
      label: 'Cancelled',
      explanation: 'The operation was cancelled before completion.',
      tone: StatusTone.neutral,
    ),
    RemoteOpState.rejectedOffline => const StatusPresentation(
      label: 'Rejected while offline',
      explanation:
          'This operation is unsafe to queue and was not dispatched while the host was offline.',
      tone: StatusTone.warning,
    ),
    RemoteOpState.failed => const StatusPresentation(
      label: 'Failed',
      explanation:
          'The operation failed and was not assumed to have changed source state.',
      tone: StatusTone.danger,
    ),
    RemoteOpState.outcomeUnknown => const StatusPresentation(
      label: 'Outcome unknown',
      explanation:
          'Connectivity was lost after dispatch, so Muxport cannot safely claim success or retry.',
      tone: StatusTone.warning,
    ),
    RemoteOpState.reconciliationRequired => const StatusPresentation(
      label: 'Reconciliation required',
      explanation:
          'Muxport must obtain authoritative source state before another mutation is safe.',
      tone: StatusTone.warning,
    ),
  };
}

StatusPresentation hostSyncPresentation(HostSyncPhase phase) {
  return switch (phase) {
    HostSyncPhase.pairingPending => const StatusPresentation(
      label: 'Awaiting host confirmation',
      explanation:
          'Confirm the matching pairing code on both devices before trusting this host.',
      tone: StatusTone.progress,
    ),
    HostSyncPhase.cachedStale => const StatusPresentation(
      label: 'Cached · stale',
      explanation:
          'This is saved local state and has not been confirmed by the host.',
      tone: StatusTone.warning,
    ),
    HostSyncPhase.reconnecting => const StatusPresentation(
      label: 'Reconnecting',
      explanation:
          'Muxport is re-establishing an authenticated connection to the host.',
      tone: StatusTone.progress,
    ),
    HostSyncPhase.replaying => const StatusPresentation(
      label: 'Replaying',
      explanation:
          'Muxport is applying missed durable events from the last acknowledged cursor.',
      tone: StatusTone.progress,
    ),
    HostSyncPhase.snapshotRequired => const StatusPresentation(
      label: 'Snapshot required',
      explanation:
          'The replay cursor is no longer usable. A complete authenticated snapshot is required.',
      tone: StatusTone.warning,
    ),
    HostSyncPhase.synchronized => const StatusPresentation(
      label: 'Synchronized',
      explanation:
          'Host identity and the latest snapshot boundary are verified.',
      tone: StatusTone.healthy,
    ),
    HostSyncPhase.offline => const StatusPresentation(
      label: 'Offline',
      explanation:
          'The host is unreachable. Immediate changes and approvals are disabled.',
      tone: StatusTone.warning,
    ),
    HostSyncPhase.identityMismatch => const StatusPresentation(
      label: 'Identity mismatch',
      explanation:
          'The host key changed unexpectedly. Do not reconnect until the host identity is verified.',
      tone: StatusTone.danger,
    ),
    HostSyncPhase.protocolIncompatible => const StatusPresentation(
      label: 'Upgrade required',
      explanation:
          'The phone and connector cannot safely agree on command semantics.',
      tone: StatusTone.warning,
    ),
  };
}
