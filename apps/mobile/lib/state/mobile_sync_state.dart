import 'package:flutter_protocol/flutter_protocol.dart';

const int mobileProtocolVersion = 1;
const int _recentEventLimit = 256;

enum HostSyncPhase {
  cachedStale,
  reconnecting,
  replaying,
  snapshotRequired,
  synchronized,
  offline,
  identityMismatch,
  protocolIncompatible,
}

enum EventApplyOutcome {
  applied,
  duplicateEvent,
  staleSequence,
  staleSourceVersion,
  snapshotRequired,
}

enum MobileOperationState {
  pending,
  connectorAcknowledged,
  succeeded,
  failed,
  expired,
  reconciliationRequired,
}

class SyncCursor {
  const SyncCursor({
    required this.hostEpoch,
    required this.sequence,
  }) : assert(sequence >= 0);

  final String hostEpoch;
  final int sequence;

  Map<String, Object?> toJson() => {
        'hostEpoch': hostEpoch,
        'sequence': sequence,
      };

  factory SyncCursor.fromJson(Map<String, Object?> json) {
    return SyncCursor(
      hostEpoch: json['hostEpoch']! as String,
      sequence: json['sequence']! as int,
    );
  }

  @override
  bool operator ==(Object other) {
    return other is SyncCursor &&
        other.hostEpoch == hostEpoch &&
        other.sequence == sequence;
  }

  @override
  int get hashCode => Object.hash(hostEpoch, sequence);
}

class PendingOperation {
  const PendingOperation({
    required this.idempotencyKey,
    required this.kind,
    required this.createdAtMs,
    required this.deadlineMs,
    required this.state,
    this.requiresSourceConfirmation = true,
  });

  factory PendingOperation.local({
    required String idempotencyKey,
    required String kind,
    required int createdAtMs,
    required int deadlineMs,
    bool requiresSourceConfirmation = true,
  }) {
    if (idempotencyKey.isEmpty) {
      throw ArgumentError.value(
        idempotencyKey,
        'idempotencyKey',
        'must not be empty',
      );
    }
    if (deadlineMs <= createdAtMs) {
      throw ArgumentError.value(
        deadlineMs,
        'deadlineMs',
        'must be after createdAtMs',
      );
    }
    return PendingOperation(
      idempotencyKey: idempotencyKey,
      kind: kind,
      createdAtMs: createdAtMs,
      deadlineMs: deadlineMs,
      state: MobileOperationState.pending,
      requiresSourceConfirmation: requiresSourceConfirmation,
    );
  }

  final String idempotencyKey;
  final String kind;
  final int createdAtMs;
  final int deadlineMs;
  final MobileOperationState state;
  final bool requiresSourceConfirmation;

  bool get isTerminal =>
      state == MobileOperationState.succeeded ||
      state == MobileOperationState.failed ||
      state == MobileOperationState.expired;

  bool get requiresStatusQuery => !isTerminal;

  PendingOperation resolve(RemoteOpState remoteState) {
    final nextState = switch (remoteState) {
      RemoteOpState.succeeded => MobileOperationState.succeeded,
      RemoteOpState.expired => MobileOperationState.expired,
      RemoteOpState.cancelled ||
      RemoteOpState.rejectedOffline ||
      RemoteOpState.failed =>
        MobileOperationState.failed,
      RemoteOpState.outcomeUnknown ||
      RemoteOpState.reconciliationRequired ||
      RemoteOpState.unspecified =>
        MobileOperationState.reconciliationRequired,
      RemoteOpState.sourceAcknowledged ||
      RemoteOpState.reconciled =>
        MobileOperationState.connectorAcknowledged,
      RemoteOpState.created ||
      RemoteOpState.persisted ||
      RemoteOpState.dispatched =>
        MobileOperationState.pending,
    };
    return copyWith(state: nextState);
  }

  PendingOperation expireIfPast(int nowMs) {
    if (!isTerminal && nowMs >= deadlineMs) {
      return copyWith(state: MobileOperationState.expired);
    }
    return this;
  }

  PendingOperation copyWith({MobileOperationState? state}) {
    return PendingOperation(
      idempotencyKey: idempotencyKey,
      kind: kind,
      createdAtMs: createdAtMs,
      deadlineMs: deadlineMs,
      state: state ?? this.state,
      requiresSourceConfirmation: requiresSourceConfirmation,
    );
  }

  Map<String, Object?> toJson() => {
        'idempotencyKey': idempotencyKey,
        'kind': kind,
        'createdAtMs': createdAtMs,
        'deadlineMs': deadlineMs,
        'state': state.name,
        'requiresSourceConfirmation': requiresSourceConfirmation,
      };

  factory PendingOperation.fromJson(Map<String, Object?> json) {
    return PendingOperation(
      idempotencyKey: json['idempotencyKey']! as String,
      kind: json['kind']! as String,
      createdAtMs: json['createdAtMs']! as int,
      deadlineMs: json['deadlineMs']! as int,
      state: _enumByName(
        MobileOperationState.values,
        json['state']! as String,
        MobileOperationState.reconciliationRequired,
      ),
      requiresSourceConfirmation:
          json['requiresSourceConfirmation'] as bool? ?? true,
    );
  }
}

class HostSyncState {
  HostSyncState({
    required this.hostId,
    required this.pinnedHostKey,
    required this.displayName,
    required this.protocolVersion,
    required this.phase,
    required Map<String, Object?> snapshot,
    required this.cursor,
    required Map<String, int> sourceVersions,
    required List<String> recentEventIds,
    required Map<String, PendingOperation> pendingOperations,
  })  : snapshot = Map.unmodifiable(snapshot),
        sourceVersions = Map.unmodifiable(sourceVersions),
        recentEventIds = List.unmodifiable(recentEventIds),
        pendingOperations = Map.unmodifiable(pendingOperations);

  final String hostId;
  final String pinnedHostKey;
  final String displayName;
  final int protocolVersion;
  final HostSyncPhase phase;
  final Map<String, Object?> snapshot;
  final SyncCursor? cursor;
  final Map<String, int> sourceVersions;
  final List<String> recentEventIds;
  final Map<String, PendingOperation> pendingOperations;

  bool get canMutate => phase == HostSyncPhase.synchronized;

  bool get needsSnapshot => phase == HostSyncPhase.snapshotRequired;

  List<String> get operationIdsRequiringStatusQuery => pendingOperations.values
      .where((operation) => operation.requiresStatusQuery)
      .map((operation) => operation.idempotencyKey)
      .toList(growable: false);

  HostSyncState withPhase(HostSyncPhase nextPhase) {
    return _replace(phase: nextPhase);
  }

  HostSyncState addPendingOperation(PendingOperation operation) {
    if (pendingOperations.containsKey(operation.idempotencyKey)) {
      throw StateError(
        'operation ${operation.idempotencyKey} is already tracked',
      );
    }
    return _replace(
      pendingOperations: {
        ...pendingOperations,
        operation.idempotencyKey: operation,
      },
    );
  }

  HostSyncState resolveOperation(
    String idempotencyKey,
    RemoteOpState remoteState,
  ) {
    final operation = pendingOperations[idempotencyKey];
    if (operation == null) {
      throw StateError('unknown operation $idempotencyKey');
    }
    return _replace(
      pendingOperations: {
        ...pendingOperations,
        idempotencyKey: operation.resolve(remoteState),
      },
    );
  }

  HostSyncState expireOperations(int nowMs) {
    return _replace(
      pendingOperations: {
        for (final entry in pendingOperations.entries)
          entry.key: entry.value.expireIfPast(nowMs),
      },
    );
  }

  Map<String, Object?> toCacheJson() => {
        'schemaVersion': 1,
        'hostId': hostId,
        'pinnedHostKey': pinnedHostKey,
        'displayName': displayName,
        'protocolVersion': protocolVersion,
        'snapshot': snapshot,
        'cursor': cursor?.toJson(),
        'sourceVersions': sourceVersions,
        'recentEventIds': recentEventIds,
        'pendingOperations': pendingOperations.values
            .map((operation) => operation.toJson())
            .toList(growable: false),
      };

  factory HostSyncState.fromCacheJson(Map<String, Object?> json) {
    if (json['schemaVersion'] != 1) {
      throw const FormatException('unsupported mobile cache schema');
    }

    final rawCursor = json['cursor'];
    final rawSourceVersions =
        Map<String, Object?>.from(json['sourceVersions']! as Map);
    final rawOperations = json['pendingOperations']! as List;

    return HostSyncState(
      hostId: json['hostId']! as String,
      pinnedHostKey: json['pinnedHostKey']! as String,
      displayName: json['displayName']! as String,
      protocolVersion: json['protocolVersion']! as int,
      phase: HostSyncPhase.cachedStale,
      snapshot: Map<String, Object?>.from(json['snapshot']! as Map),
      cursor: rawCursor == null
          ? null
          : SyncCursor.fromJson(Map<String, Object?>.from(rawCursor as Map)),
      sourceVersions: {
        for (final entry in rawSourceVersions.entries)
          entry.key: entry.value! as int,
      },
      recentEventIds:
          List<String>.from(json['recentEventIds']! as List<dynamic>),
      pendingOperations: {
        for (final rawOperation in rawOperations)
          (rawOperation as Map)['idempotencyKey']! as String:
              PendingOperation.fromJson(
            Map<String, Object?>.from(rawOperation),
          ),
      },
    );
  }

  HostSyncState _replace({
    HostSyncPhase? phase,
    Map<String, Object?>? snapshot,
    SyncCursor? cursor,
    Map<String, int>? sourceVersions,
    List<String>? recentEventIds,
    Map<String, PendingOperation>? pendingOperations,
  }) {
    return HostSyncState(
      hostId: hostId,
      pinnedHostKey: pinnedHostKey,
      displayName: displayName,
      protocolVersion: protocolVersion,
      phase: phase ?? this.phase,
      snapshot: snapshot ?? this.snapshot,
      cursor: cursor ?? this.cursor,
      sourceVersions: sourceVersions ?? this.sourceVersions,
      recentEventIds: recentEventIds ?? this.recentEventIds,
      pendingOperations: pendingOperations ?? this.pendingOperations,
    );
  }
}

class NormalizedSyncEvent {
  const NormalizedSyncEvent({
    required this.eventId,
    required this.hostEpoch,
    required this.sequence,
    required this.sourceObjectId,
    required this.sourceObjectVersion,
    required this.payload,
  })  : assert(sequence > 0),
        assert(sourceObjectVersion >= 0);

  final String eventId;
  final String hostEpoch;
  final int sequence;
  final String sourceObjectId;
  final int sourceObjectVersion;
  final Map<String, Object?> payload;
}

class AuthoritativeHostSnapshot {
  const AuthoritativeHostSnapshot({
    required this.hostId,
    required this.protocolVersion,
    required this.cursor,
    required this.data,
    required this.sourceVersions,
  });

  final String hostId;
  final int protocolVersion;
  final SyncCursor cursor;
  final Map<String, Object?> data;
  final Map<String, int> sourceVersions;
}

typedef SnapshotEventReducer = Map<String, Object?> Function(
  Map<String, Object?> currentSnapshot,
  NormalizedSyncEvent event,
);

class EventApplyTransition {
  const EventApplyTransition({
    required this.state,
    required this.outcome,
    required this.persistBeforeAcknowledging,
    required this.cursorToAcknowledge,
  });

  final HostSyncState state;
  final EventApplyOutcome outcome;

  /// When true, the caller must atomically persist [state] before acknowledging
  /// its cursor to the connector.
  final bool persistBeforeAcknowledging;

  /// A previously persisted cursor may be acknowledged again after a duplicate
  /// delivery. A null value means the caller must request a snapshot instead.
  final SyncCursor? cursorToAcknowledge;
}

class MobileSyncReducer {
  const MobileSyncReducer._();

  static HostSyncState authenticate({
    required HostSyncState state,
    required String observedHostId,
    required String observedHostKey,
    required int negotiatedProtocolVersion,
  }) {
    if (observedHostId != state.hostId ||
        observedHostKey != state.pinnedHostKey) {
      return state.withPhase(HostSyncPhase.identityMismatch);
    }
    if (negotiatedProtocolVersion != state.protocolVersion ||
        negotiatedProtocolVersion != mobileProtocolVersion) {
      return state.withPhase(HostSyncPhase.protocolIncompatible);
    }
    return state.withPhase(HostSyncPhase.reconnecting);
  }

  static HostSyncState beginRecovery({
    required HostSyncState state,
    required String remoteHostEpoch,
    required int oldestAvailableSequence,
  }) {
    if (state.phase != HostSyncPhase.reconnecting) {
      throw StateError('host identity must be authenticated before recovery');
    }
    if (remoteHostEpoch.isEmpty || oldestAvailableSequence < 1) {
      throw ArgumentError('invalid remote journal boundary');
    }

    final cursor = state.cursor;
    if (cursor == null ||
        cursor.hostEpoch != remoteHostEpoch ||
        cursor.sequence + 1 < oldestAvailableSequence) {
      return state.withPhase(HostSyncPhase.snapshotRequired);
    }
    return state.withPhase(HostSyncPhase.replaying);
  }

  static HostSyncState completeReplay(HostSyncState state) {
    if (state.phase != HostSyncPhase.replaying) {
      throw StateError('cannot complete replay from ${state.phase.name}');
    }
    return state.withPhase(HostSyncPhase.synchronized);
  }

  static EventApplyTransition applyEvent({
    required HostSyncState state,
    required NormalizedSyncEvent event,
    required SnapshotEventReducer reduceSnapshot,
  }) {
    if (state.phase != HostSyncPhase.replaying &&
        state.phase != HostSyncPhase.synchronized) {
      throw StateError('events are not accepted while ${state.phase.name}');
    }

    final cursor = state.cursor;
    if (cursor == null || event.hostEpoch != cursor.hostEpoch) {
      return EventApplyTransition(
        state: state.withPhase(HostSyncPhase.snapshotRequired),
        outcome: EventApplyOutcome.snapshotRequired,
        persistBeforeAcknowledging: false,
        cursorToAcknowledge: null,
      );
    }
    if (state.recentEventIds.contains(event.eventId)) {
      if (event.sequence > cursor.sequence) {
        return EventApplyTransition(
          state: state.withPhase(HostSyncPhase.snapshotRequired),
          outcome: EventApplyOutcome.snapshotRequired,
          persistBeforeAcknowledging: false,
          cursorToAcknowledge: null,
        );
      }
      return EventApplyTransition(
        state: state,
        outcome: EventApplyOutcome.duplicateEvent,
        persistBeforeAcknowledging: false,
        cursorToAcknowledge: cursor,
      );
    }
    if (event.sequence <= cursor.sequence) {
      return EventApplyTransition(
        state: state,
        outcome: EventApplyOutcome.staleSequence,
        persistBeforeAcknowledging: false,
        cursorToAcknowledge: cursor,
      );
    }
    if (event.sequence != cursor.sequence + 1) {
      return EventApplyTransition(
        state: state.withPhase(HostSyncPhase.snapshotRequired),
        outcome: EventApplyOutcome.snapshotRequired,
        persistBeforeAcknowledging: false,
        cursorToAcknowledge: null,
      );
    }

    final sourceVersions = Map<String, int>.from(state.sourceVersions);
    final currentSourceVersion = sourceVersions[event.sourceObjectId] ?? -1;
    final isStaleSourceVersion =
        event.sourceObjectVersion <= currentSourceVersion;
    if (!isStaleSourceVersion) {
      sourceVersions[event.sourceObjectId] = event.sourceObjectVersion;
    }

    final recentEventIds = [...state.recentEventIds, event.eventId];
    if (recentEventIds.length > _recentEventLimit) {
      recentEventIds.removeRange(
        0,
        recentEventIds.length - _recentEventLimit,
      );
    }

    final nextState = HostSyncState(
      hostId: state.hostId,
      pinnedHostKey: state.pinnedHostKey,
      displayName: state.displayName,
      protocolVersion: state.protocolVersion,
      phase: state.phase,
      snapshot: isStaleSourceVersion
          ? state.snapshot
          : reduceSnapshot(state.snapshot, event),
      cursor: SyncCursor(
        hostEpoch: event.hostEpoch,
        sequence: event.sequence,
      ),
      sourceVersions: sourceVersions,
      recentEventIds: recentEventIds,
      pendingOperations: state.pendingOperations,
    );

    return EventApplyTransition(
      state: nextState,
      outcome: isStaleSourceVersion
          ? EventApplyOutcome.staleSourceVersion
          : EventApplyOutcome.applied,
      persistBeforeAcknowledging: true,
      cursorToAcknowledge: nextState.cursor,
    );
  }

  static HostSyncState applySnapshot({
    required HostSyncState state,
    required AuthoritativeHostSnapshot snapshot,
  }) {
    if (state.phase != HostSyncPhase.reconnecting &&
        state.phase != HostSyncPhase.replaying &&
        state.phase != HostSyncPhase.snapshotRequired &&
        state.phase != HostSyncPhase.synchronized) {
      throw StateError(
        'cannot accept a snapshot before host authentication',
      );
    }
    if (snapshot.hostId != state.hostId) {
      throw StateError('snapshot host identity does not match the cache');
    }
    if (snapshot.protocolVersion != state.protocolVersion ||
        snapshot.protocolVersion != mobileProtocolVersion) {
      return state.withPhase(HostSyncPhase.protocolIncompatible);
    }
    return HostSyncState(
      hostId: state.hostId,
      pinnedHostKey: state.pinnedHostKey,
      displayName: state.displayName,
      protocolVersion: state.protocolVersion,
      phase: HostSyncPhase.synchronized,
      snapshot: snapshot.data,
      cursor: snapshot.cursor,
      sourceVersions: snapshot.sourceVersions,
      recentEventIds: const [],
      pendingOperations: state.pendingOperations,
    );
  }

  static HostSyncState markOffline(HostSyncState state) {
    return state.withPhase(HostSyncPhase.offline);
  }
}

T _enumByName<T extends Enum>(
  List<T> values,
  String name,
  T fallback,
) {
  for (final value in values) {
    if (value.name == name) {
      return value;
    }
  }
  return fallback;
}
