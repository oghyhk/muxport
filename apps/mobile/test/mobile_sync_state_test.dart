import 'package:flutter_protocol/flutter_protocol.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/state/mobile_sync_state.dart';

void main() {
  group('mobile cache recovery', () {
    test('restored cache is stale and preserves recovery metadata', () {
      final original = _state(
        phase: HostSyncPhase.synchronized,
        pendingOperations: {
          'approval-1': PendingOperation.local(
            idempotencyKey: 'approval-1',
            kind: 'approval',
            createdAtMs: 100,
            deadlineMs: 500,
          ),
        },
      );

      final restored = HostSyncState.fromCacheJson(original.toCacheJson());

      expect(restored.phase, HostSyncPhase.cachedStale);
      expect(restored.canMutate, isFalse);
      expect(
          restored.cursor, const SyncCursor(hostEpoch: 'epoch-a', sequence: 4));
      expect(restored.snapshot, {'session': 'cached'});
      expect(restored.operationIdsRequiringStatusQuery, ['approval-1']);
    });

    test('host key mismatch fails closed', () {
      final authenticated = MobileSyncReducer.authenticate(
        state: _state(),
        observedHostId: 'host-1',
        observedHostKey: 'replacement-key',
        negotiatedProtocolVersion: mobileProtocolVersion,
      );

      expect(authenticated.phase, HostSyncPhase.identityMismatch);
      expect(authenticated.canMutate, isFalse);
    });

    test('incompatible protocol remains read-only', () {
      final authenticated = MobileSyncReducer.authenticate(
        state: _state(),
        observedHostId: 'host-1',
        observedHostKey: 'host-key-1',
        negotiatedProtocolVersion: 99,
      );

      expect(authenticated.phase, HostSyncPhase.protocolIncompatible);
      expect(authenticated.canMutate, isFalse);
    });

    test('replay starts only from a retained cursor in the same epoch', () {
      final authenticated = _authenticate(_state());
      final replaying = MobileSyncReducer.beginRecovery(
        state: authenticated,
        remoteHostEpoch: 'epoch-a',
        oldestAvailableSequence: 5,
      );

      expect(replaying.phase, HostSyncPhase.replaying);
      expect(replaying.canMutate, isFalse);
      expect(
        MobileSyncReducer.completeReplay(replaying).phase,
        HostSyncPhase.synchronized,
      );
    });

    test('journal reset or retention gap requires a snapshot', () {
      final authenticated = _authenticate(_state());

      final reset = MobileSyncReducer.beginRecovery(
        state: authenticated,
        remoteHostEpoch: 'epoch-b',
        oldestAvailableSequence: 1,
      );
      final gap = MobileSyncReducer.beginRecovery(
        state: authenticated,
        remoteHostEpoch: 'epoch-a',
        oldestAvailableSequence: 7,
      );

      expect(reset.phase, HostSyncPhase.snapshotRequired);
      expect(gap.phase, HostSyncPhase.snapshotRequired);
      expect(reset.canMutate, isFalse);
    });
  });

  group('event replay', () {
    test('ordered event advances cursor and must be persisted before ack', () {
      final transition = MobileSyncReducer.applyEvent(
        state: _replayingState(),
        event: _event(
          eventId: 'event-5',
          sequence: 5,
          sourceVersion: 2,
          value: 'live',
        ),
        reduceSnapshot: _mergeValue,
      );

      expect(transition.outcome, EventApplyOutcome.applied);
      expect(transition.persistBeforeAcknowledging, isTrue);
      expect(
        transition.state.cursor,
        const SyncCursor(hostEpoch: 'epoch-a', sequence: 5),
      );
      expect(
        transition.cursorToAcknowledge,
        const SyncCursor(hostEpoch: 'epoch-a', sequence: 5),
      );
      expect(transition.state.snapshot['session'], 'live');
      expect(transition.state.sourceVersions['session-1'], 2);
    });

    test('duplicate event re-acknowledges the durable cursor', () {
      final transition = MobileSyncReducer.applyEvent(
        state: _replayingState(
          cursorSequence: 5,
          recentEventIds: const ['event-5'],
        ),
        event: _event(
          eventId: 'event-5',
          sequence: 5,
          sourceVersion: 2,
        ),
        reduceSnapshot: _mergeValue,
      );

      expect(transition.outcome, EventApplyOutcome.duplicateEvent);
      expect(transition.persistBeforeAcknowledging, isFalse);
      expect(transition.state.cursor?.sequence, 5);
      expect(transition.cursorToAcknowledge?.sequence, 5);
    });

    test('event ID reuse at a new sequence forces a snapshot', () {
      final transition = MobileSyncReducer.applyEvent(
        state: _replayingState(recentEventIds: const ['event-5']),
        event: _event(
          eventId: 'event-5',
          sequence: 5,
          sourceVersion: 2,
        ),
        reduceSnapshot: _mergeValue,
      );

      expect(transition.outcome, EventApplyOutcome.snapshotRequired);
      expect(transition.state.phase, HostSyncPhase.snapshotRequired);
      expect(transition.cursorToAcknowledge, isNull);
    });

    test('stale object version consumes its ordered journal position', () {
      final transition = MobileSyncReducer.applyEvent(
        state: _replayingState(sourceVersion: 3),
        event: _event(
          eventId: 'event-5',
          sequence: 5,
          sourceVersion: 2,
          value: 'stale',
        ),
        reduceSnapshot: _mergeValue,
      );

      expect(transition.outcome, EventApplyOutcome.staleSourceVersion);
      expect(transition.persistBeforeAcknowledging, isTrue);
      expect(transition.cursorToAcknowledge?.sequence, 5);
      expect(transition.state.cursor?.sequence, 5);
      expect(transition.state.snapshot['session'], 'cached');
      expect(transition.state.sourceVersions['session-1'], 3);
    });

    test('sequence gap blocks mutations and forces snapshot recovery', () {
      final transition = MobileSyncReducer.applyEvent(
        state: _replayingState(),
        event: _event(
          eventId: 'event-7',
          sequence: 7,
          sourceVersion: 2,
        ),
        reduceSnapshot: _mergeValue,
      );

      expect(transition.outcome, EventApplyOutcome.snapshotRequired);
      expect(transition.persistBeforeAcknowledging, isFalse);
      expect(transition.state.phase, HostSyncPhase.snapshotRequired);
      expect(transition.state.canMutate, isFalse);
      expect(transition.cursorToAcknowledge, isNull);
    });

    test('authoritative snapshot replaces stale cache and cursor', () {
      final recovered = MobileSyncReducer.applySnapshot(
        state: _state(phase: HostSyncPhase.snapshotRequired),
        snapshot: const AuthoritativeHostSnapshot(
          hostId: 'host-1',
          protocolVersion: mobileProtocolVersion,
          cursor: SyncCursor(hostEpoch: 'epoch-b', sequence: 12),
          data: {'session': 'authoritative'},
          sourceVersions: {'session-1': 9},
        ),
      );

      expect(recovered.phase, HostSyncPhase.synchronized);
      expect(recovered.canMutate, isTrue);
      expect(recovered.snapshot['session'], 'authoritative');
      expect(recovered.cursor?.hostEpoch, 'epoch-b');
      expect(recovered.cursor?.sequence, 12);
      expect(recovered.recentEventIds, isEmpty);
    });

    test('snapshot is rejected before host authentication', () {
      expect(
        () => MobileSyncReducer.applySnapshot(
          state: _state(),
          snapshot: const AuthoritativeHostSnapshot(
            hostId: 'host-1',
            protocolVersion: mobileProtocolVersion,
            cursor: SyncCursor(hostEpoch: 'epoch-b', sequence: 12),
            data: {'session': 'authoritative'},
            sourceVersions: {'session-1': 9},
          ),
        ),
        throwsStateError,
      );
    });
  });

  group('pending operation reconciliation', () {
    test('approval stays pending until final source-confirmed success', () {
      final pending = PendingOperation.local(
        idempotencyKey: 'approval-1',
        kind: 'approval',
        createdAtMs: 100,
        deadlineMs: 500,
      );
      final withPending = _state().addPendingOperation(pending);

      final dispatched = withPending.resolveOperation(
        'approval-1',
        RemoteOpState.dispatched,
      );
      final sourceAcknowledged = dispatched.resolveOperation(
        'approval-1',
        RemoteOpState.sourceAcknowledged,
      );
      final succeeded = sourceAcknowledged.resolveOperation(
        'approval-1',
        RemoteOpState.succeeded,
      );

      expect(
        dispatched.pendingOperations['approval-1']?.state,
        MobileOperationState.pending,
      );
      expect(
        sourceAcknowledged.pendingOperations['approval-1']?.state,
        MobileOperationState.connectorAcknowledged,
      );
      expect(
        sourceAcknowledged.operationIdsRequiringStatusQuery,
        ['approval-1'],
      );
      expect(
        succeeded.pendingOperations['approval-1']?.state,
        MobileOperationState.succeeded,
      );
      expect(succeeded.operationIdsRequiringStatusQuery, isEmpty);
    });

    test('unknown outcome remains explicit and queryable', () {
      final state = _state().addPendingOperation(
        PendingOperation.local(
          idempotencyKey: 'rotate-1',
          kind: 'credential-rotation',
          createdAtMs: 100,
          deadlineMs: 500,
        ),
      );

      final reconciled = state.resolveOperation(
        'rotate-1',
        RemoteOpState.outcomeUnknown,
      );

      expect(
        reconciled.pendingOperations['rotate-1']?.state,
        MobileOperationState.reconciliationRequired,
      );
      expect(reconciled.operationIdsRequiringStatusQuery, ['rotate-1']);
    });

    test('expired local operation is terminal and never replayed', () {
      final state = _state().addPendingOperation(
        PendingOperation.local(
          idempotencyKey: 'interrupt-1',
          kind: 'interrupt',
          createdAtMs: 100,
          deadlineMs: 500,
        ),
      );

      final expired = state.expireOperations(500);

      expect(
        expired.pendingOperations['interrupt-1']?.state,
        MobileOperationState.expired,
      );
      expect(expired.operationIdsRequiringStatusQuery, isEmpty);
    });
  });
}

HostSyncState _state({
  HostSyncPhase phase = HostSyncPhase.cachedStale,
  Map<String, PendingOperation> pendingOperations = const {},
}) {
  return HostSyncState(
    hostId: 'host-1',
    pinnedHostKey: 'host-key-1',
    displayName: 'Development host',
    protocolVersion: mobileProtocolVersion,
    phase: phase,
    snapshot: const {'session': 'cached'},
    cursor: const SyncCursor(hostEpoch: 'epoch-a', sequence: 4),
    sourceVersions: const {'session-1': 1},
    recentEventIds: const [],
    pendingOperations: pendingOperations,
  );
}

HostSyncState _authenticate(HostSyncState state) {
  return MobileSyncReducer.authenticate(
    state: state,
    observedHostId: 'host-1',
    observedHostKey: 'host-key-1',
    negotiatedProtocolVersion: mobileProtocolVersion,
  );
}

HostSyncState _replayingState({
  int sourceVersion = 1,
  int cursorSequence = 4,
  List<String> recentEventIds = const [],
}) {
  return HostSyncState(
    hostId: 'host-1',
    pinnedHostKey: 'host-key-1',
    displayName: 'Development host',
    protocolVersion: mobileProtocolVersion,
    phase: HostSyncPhase.replaying,
    snapshot: const {'session': 'cached'},
    cursor: SyncCursor(hostEpoch: 'epoch-a', sequence: cursorSequence),
    sourceVersions: {'session-1': sourceVersion},
    recentEventIds: recentEventIds,
    pendingOperations: const {},
  );
}

NormalizedSyncEvent _event({
  required String eventId,
  required int sequence,
  required int sourceVersion,
  String value = 'updated',
}) {
  return NormalizedSyncEvent(
    eventId: eventId,
    hostEpoch: 'epoch-a',
    sequence: sequence,
    sourceObjectId: 'session-1',
    sourceObjectVersion: sourceVersion,
    payload: {'value': value},
  );
}

Map<String, Object?> _mergeValue(
  Map<String, Object?> snapshot,
  NormalizedSyncEvent event,
) {
  return {
    ...snapshot,
    'session': event.payload['value'],
  };
}
