import 'package:flutter_protocol/wire_protocol.dart' as wire;

import '../security/device_identity.dart';
import '../transport/direct_transport_client.dart';
import '../transport/direct_transport_protocol.dart';
import 'mobile_cache_store.dart';
import 'mobile_sync_state.dart';

class HostSyncOrchestrator {
  const HostSyncOrchestrator({this.maximumBatches = 32});

  final int maximumBatches;

  Future<HostSyncState> synchronizeOnce({
    required HostSyncState state,
    required MobileDeviceIdentity identity,
    required GenerationMobileCacheStore cacheStore,
    required Iterable<HostSyncState> allHosts,
  }) async {
    if (!state.canReconnect || maximumBatches < 1) {
      throw const DirectTransportProtocolException(
        'host has no usable direct sync endpoint',
      );
    }
    final connection = await DirectSyncConnection.connect(
      address: state.directAddress!,
      port: state.directPort!,
      pinnedHost: PinnedHostIdentity(
        hostId: state.hostId,
        publicKeyHex: state.pinnedHostKey,
      ),
      identity: identity,
      afterSequence: state.cursor?.sequence ?? 0,
      knownHostBootEpoch: int.tryParse(state.cursor?.hostEpoch ?? '') ?? 0,
    );
    var current = state;
    try {
      var batch = await connection.readInitialBatch();
      for (var index = 0; index < maximumBatches; index += 1) {
        final changed = batch.snapshot != null || batch.events.isNotEmpty;
        current = _applyBatch(current, batch);
        await cacheStore.save(
          MobileCacheSnapshot(
            hosts: [
              for (final host in allHosts)
                if (host.hostId == current.hostId) current else host,
            ],
          ),
        );
        final next = await connection.acknowledgeAndPoll(
          sequence: batch.boundarySequence,
          hostBootEpoch: batch.hostBootEpoch,
        );
        if (!changed &&
            next.snapshot == null &&
            next.events.isEmpty &&
            next.boundarySequence == batch.boundarySequence) {
          return current;
        }
        if (next.snapshot == null &&
            next.events.isEmpty &&
            next.boundarySequence == batch.boundarySequence) {
          return current;
        }
        batch = next;
      }
      return current;
    } finally {
      await connection.close();
    }
  }

  HostSyncState _applyBatch(HostSyncState previous, DirectSyncBatch batch) {
    final epoch = batch.hostBootEpoch.toString();
    HostSyncState state;
    final snapshot = batch.snapshot;
    if (snapshot != null) {
      if (snapshot.hostId != previous.hostId ||
          snapshot.snapshotSequence.toInt() > batch.boundarySequence) {
        throw const DirectTransportProtocolException(
          'authoritative snapshot boundary is invalid',
        );
      }
      state = HostSyncState(
        hostId: previous.hostId,
        pinnedHostKey: previous.pinnedHostKey,
        displayName: snapshot.hostname.isEmpty
            ? previous.displayName
            : snapshot.hostname,
        protocolVersion: previous.protocolVersion,
        phase: HostSyncPhase.replaying,
        directAddress: previous.directAddress,
        directPort: previous.directPort,
        pairingPending: false,
        snapshot: _snapshotJson(snapshot),
        cursor: SyncCursor(
          hostEpoch: epoch,
          sequence: snapshot.snapshotSequence.toInt(),
        ),
        sourceVersions: const {},
        recentEventIds: const [],
        pendingOperations: previous.pendingOperations,
      );
    } else {
      final authenticated = MobileSyncReducer.authenticate(
        state: previous,
        observedHostId: previous.hostId,
        observedHostKey: previous.pinnedHostKey,
        negotiatedProtocolVersion: mobileProtocolVersion,
      );
      final oldest = batch.events.isEmpty
          ? (previous.cursor?.sequence ?? 0) + 1
          : batch.events.first.sequence;
      state = MobileSyncReducer.beginRecovery(
        state: authenticated,
        remoteHostEpoch: epoch,
        oldestAvailableSequence: oldest,
      );
      if (state.phase == HostSyncPhase.snapshotRequired) {
        throw const DirectTransportProtocolException(
          'connector did not supply the required recovery snapshot',
        );
      }
    }

    for (final journaled in batch.events) {
      final normalized = _normalizeEvent(
        journaled.event,
        journaled.sequence,
        epoch,
      );
      final transition = MobileSyncReducer.applyEvent(
        state: state,
        event: normalized,
        reduceSnapshot: _reduceSnapshot,
      );
      if (transition.outcome == EventApplyOutcome.snapshotRequired) {
        throw const DirectTransportProtocolException(
          'event replay developed a cursor gap',
        );
      }
      state = transition.state;
    }
    final cursor = state.cursor;
    if (cursor == null || cursor.sequence != batch.boundarySequence) {
      if (batch.events.isEmpty &&
          cursor != null &&
          cursor.sequence == batch.boundarySequence) {
        return MobileSyncReducer.completeReplay(state);
      }
      throw const DirectTransportProtocolException(
        'sync boundary does not match the durable mobile cursor',
      );
    }
    return MobileSyncReducer.completeReplay(state);
  }
}

Map<String, Object?> _snapshotJson(wire.HostSnapshot snapshot) {
  return {
    'connectorState': snapshot.connectorState.value,
    'runtimes': [
      for (final runtime in snapshot.runtimes)
        {
          'runtimeId': runtime.runtimeId,
          'agentType': runtime.agentType.value,
          'name': runtime.name,
          'state': runtime.state.value,
          'activeCredentialProfileId': runtime.activeCredentialProfileId,
          'projectPaths': runtime.projectPaths,
        },
    ],
    'credentialProfiles': [
      for (final profile in snapshot.credentialProfiles)
        {
          'profileId': profile.profileId,
          'displayName': profile.displayName,
          'provider': profile.provider,
          'accountFingerprint': profile.accountFingerprint,
          'status': profile.status.value,
          'lastValidatedAtMs': profile.lastValidatedAtMs.toInt(),
        },
    ],
    'activeSessions': [
      for (final session in snapshot.activeSessions)
        {
          'sessionId': session.sessionId,
          'runtimeId': session.runtimeId,
          'projectPath': session.projectPath,
          'title': session.title,
          'credentialProfileId': session.credentialProfileId,
          'status': session.status,
        },
    ],
    'recentEvents': <Object?>[],
  };
}

NormalizedSyncEvent _normalizeEvent(
  wire.Event event,
  int sequence,
  String epoch,
) {
  final payload = <String, Object?>{
    'eventId': event.eventId,
    'timestampMs': event.timestampMs.toInt(),
    'kind': event.whichInner().name,
  };
  switch (event.whichInner()) {
    case wire.Event_Inner.approvalRequested:
      payload.addAll({
        'approvalId': event.approvalRequested.approvalId,
        'sessionId': event.approvalRequested.sessionId,
        'actionType': event.approvalRequested.actionType,
      });
    case wire.Event_Inner.approvalResolved:
      payload.addAll({
        'approvalId': event.approvalResolved.approvalId,
        'sessionId': event.approvalResolved.sessionId,
        'approved': event.approvalResolved.approved,
      });
    case wire.Event_Inner.sessionUpdated:
      payload.addAll({
        'sessionId': event.sessionUpdated.sessionId,
        'runtimeId': event.sessionUpdated.runtimeId,
        'status': event.sessionUpdated.status,
      });
    case wire.Event_Inner.streamDelta:
      payload.addAll({
        'sessionId': event.streamDelta.sessionId,
        'turnId': event.streamDelta.turnId,
        // Retain a bounded mobile projection of live output. The connector
        // journal remains authoritative; this cache is only for a readable
        // timeline while the phone is connected or recovering.
        'deltaText': _boundedDelta(event.streamDelta.deltaText),
        'isFinal': event.streamDelta.isFinal,
      });
    case wire.Event_Inner.credentialRotated:
      payload.addAll({
        'profileId': event.credentialRotated.profileId,
        'runtimeId': event.credentialRotated.runtimeId,
      });
    default:
      break;
  }
  final sourceId = switch (event.whichInner()) {
    wire.Event_Inner.hostStatus => event.hostStatus.hostId,
    wire.Event_Inner.runtimeState => event.runtimeState.runtimeId,
    wire.Event_Inner.sessionUpdated => event.sessionUpdated.sessionId,
    wire.Event_Inner.streamDelta =>
      '${event.streamDelta.sessionId}:${event.streamDelta.turnId}',
    wire.Event_Inner.approvalRequested => event.approvalRequested.approvalId,
    wire.Event_Inner.approvalResolved => event.approvalResolved.approvalId,
    wire.Event_Inner.credentialRotated => event.credentialRotated.profileId,
    wire.Event_Inner.auditLogged => event.auditLogged.auditId,
    wire.Event_Inner.notSet => event.eventId,
  };
  return NormalizedSyncEvent(
    eventId: event.eventId,
    hostEpoch: epoch,
    sequence: sequence,
    sourceObjectId: sourceId.isEmpty ? event.eventId : sourceId,
    sourceObjectVersion: sequence,
    payload: payload,
  );
}

String _boundedDelta(String value) {
  const maximumCharacters = 4096;
  if (value.length <= maximumCharacters) {
    return value;
  }
  return '${value.substring(0, maximumCharacters)}\n[output truncated on mobile]';
}

Map<String, Object?> _reduceSnapshot(
  Map<String, Object?> snapshot,
  NormalizedSyncEvent event,
) {
  final recent = List<Object?>.from(
    snapshot['recentEvents'] as List? ?? const [],
  )..add(event.payload);
  if (recent.length > 256) {
    recent.removeRange(0, recent.length - 256);
  }
  return {...snapshot, 'recentEvents': recent};
}
