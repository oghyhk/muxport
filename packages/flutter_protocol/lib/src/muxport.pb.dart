// Dart protocol bindings matching muxport.proto v1

enum ConnectorState {
  unspecified,
  starting,
  vaultLocked,
  recovering,
  ready,
  degraded,
  fatalError,
}

enum RuntimeState {
  unspecified,
  unknown,
  discovering,
  starting,
  synchronizing,
  onlineIdle,
  onlineRunning,
  onlineWaitingApproval,
  degraded,
  stopped,
  crashed,
  crashLoop,
  credentialLocked,
}

enum RemoteOpState {
  unspecified,
  created,
  persisted,
  dispatched,
  sourceAcknowledged,
  reconciled,
  succeeded,
  expired,
  cancelled,
  rejectedOffline,
  failed,
  outcomeUnknown,
  reconciliationRequired,
}

enum CredentialStatus {
  unspecified,
  staged,
  active,
  coolingDown,
  invalid,
  revoked,
}

class MuxportEnvelope {
  final String senderId;
  final String recipientId;
  final int sequence;
  final int timestampMs;
  final String payloadType;

  MuxportEnvelope({
    required this.senderId,
    required this.recipientId,
    required this.sequence,
    required this.timestampMs,
    required this.payloadType,
  });

  Map<String, dynamic> toJson() => {
        'senderId': senderId,
        'recipientId': recipientId,
        'sequence': sequence,
        'timestampMs': timestampMs,
        'payloadType': payloadType,
      };
}

class HostSnapshot {
  final String hostId;
  final String hostname;
  final ConnectorState connectorState;
  final List<Map<String, dynamic>> runtimes;
  final List<Map<String, dynamic>> credentialProfiles;

  HostSnapshot({
    required this.hostId,
    required this.hostname,
    required this.connectorState,
    required this.runtimes,
    required this.credentialProfiles,
  });
}
