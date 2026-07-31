import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:fixnum/fixnum.dart';
import 'package:flutter_protocol/wire_protocol.dart' as wire;

import '../pairing/signed_pairing_offer.dart';
import '../security/device_identity.dart';
import 'direct_transport_protocol.dart';

const Duration _defaultConnectTimeout = Duration(seconds: 10);
const Duration _handshakeTimeout = Duration(seconds: 15);
const int _maximumHandshakeRecordBytes = 16 * 1024;
const int _maximumEncryptedRecordBytes =
    directTransportMaximumPlaintextBytes + 32;

class PairingEnrollmentResult {
  const PairingEnrollmentResult({
    required this.hostId,
    required this.hostname,
    required this.hostIdentityPublicKeyHex,
    required this.address,
    required this.port,
    required this.pairingId,
    required this.awaitingHostConfirmation,
  });

  final String hostId;
  final String hostname;
  final String hostIdentityPublicKeyHex;
  final String address;
  final int port;
  final String pairingId;
  final bool awaitingHostConfirmation;

  PinnedHostIdentity get pinnedHost => PinnedHostIdentity(
    hostId: hostId,
    publicKeyHex: hostIdentityPublicKeyHex,
  );
}

class PendingDirectPairing {
  PendingDirectPairing._({
    required Socket socket,
    required _SocketRecordReader reader,
    required DirectSessionCipher cipher,
    required SignedPairingOffer offer,
    required this.pairingId,
    required this.sas,
  }) : _socket = socket,
       _reader = reader,
       _cipher = cipher,
       _offer = offer;

  static Future<PendingDirectPairing> connect({
    required SignedPairingOffer offer,
    required MobileDeviceIdentity identity,
    required String deviceName,
    Duration connectTimeout = _defaultConnectTimeout,
  }) async {
    if (deviceName.trim().isEmpty || deviceName.length > 128) {
      throw const DirectTransportProtocolException(
        'pairing device name must be between 1 and 128 characters',
      );
    }
    Socket? socket;
    _SocketRecordReader? reader;
    PendingDirectHandshake? pendingHandshake;
    try {
      socket = await Socket.connect(
        offer.directAddress,
        offer.directPort,
        timeout: connectTimeout,
      );
      socket.setOption(SocketOption.tcpNoDelay, true);
      reader = _SocketRecordReader(socket);
      final initiator = DirectHandshakeInitiator(identity: identity);
      final challengeRecord = await reader
          .readRecord(_maximumHandshakeRecordBytes)
          .timeout(_handshakeTimeout);
      final serverChallenge = await initiator.verifyServerChallenge(
        jsonValue: jsonDecode(utf8.decode(challengeRecord)),
        pinnedHost: offer.candidateHostPin,
        nowMs: DateTime.now().millisecondsSinceEpoch,
      );
      final boundChallenge = VerifiedServerChallenge(
        hostId: serverChallenge.hostId,
        hostIdentityPublicKeyHex: serverChallenge.hostIdentityPublicKeyHex,
        bootEpoch: serverChallenge.bootEpoch,
        challenge: pairingConnectionChallenge(
          rendezvousToken: offer.rendezvousToken,
          serverChallenge: serverChallenge.challenge,
        ),
        issuedAtMs: serverChallenge.issuedAtMs,
        expiresAtMs: serverChallenge.expiresAtMs,
      );
      pendingHandshake = await initiator.createInitiatorHello(
        challenge: boundChallenge,
      );
      final request = <String, Object?>{
        'kind': 'pairing',
        'protocolVersion': directTransportProtocolVersion,
        'rendezvousToken': offer.rendezvousToken,
        'deviceName': deviceName,
        'initiator': pendingHandshake.initiator.toJson(),
      };
      await _writeRecord(
        socket,
        utf8.encode(jsonEncode(request)),
        maximumBytes: _maximumHandshakeRecordBytes,
      ).timeout(_handshakeTimeout);
      final responseRecord = await reader
          .readRecord(_maximumHandshakeRecordBytes)
          .timeout(_handshakeTimeout);
      final responseValue = jsonDecode(utf8.decode(responseRecord));
      if (responseValue is! Map) {
        throw const DirectTransportProtocolException(
          'pairing response must be a JSON object',
        );
      }
      final response = Map<String, Object?>.from(responseValue);
      if (response.keys.toSet().difference(const {
            'protocolVersion',
            'pairingId',
            'responder',
          }).isNotEmpty ||
          response.length != 3 ||
          response['protocolVersion'] != directTransportProtocolVersion ||
          response['pairingId'] is! String ||
          (response['pairingId']! as String).trim().isEmpty ||
          (response['pairingId']! as String).length > 256) {
        throw const DirectTransportProtocolException(
          'pairing response context is invalid',
        );
      }
      final material = await pendingHandshake.finishPairing(
        response['responder'],
        expectedHostEphemeralPublicKeyHex: offer.ephemeralPublicKeyHex,
      );
      final cipher = DirectSessionCipher(material.keys);
      material.keys.destroy();
      return PendingDirectPairing._(
        socket: socket,
        reader: reader,
        cipher: cipher,
        offer: offer,
        pairingId: response['pairingId']! as String,
        sas: material.sas,
      );
    } on DirectTransportProtocolException {
      pendingHandshake?.abort();
      await reader?.cancel();
      socket?.destroy();
      rethrow;
    } on Object catch (error) {
      pendingHandshake?.abort();
      await reader?.cancel();
      socket?.destroy();
      throw DirectTransportProtocolException(
        'could not establish the direct pairing session',
        error,
      );
    }
  }

  final Socket _socket;
  final _SocketRecordReader _reader;
  final DirectSessionCipher _cipher;
  final SignedPairingOffer _offer;
  final String pairingId;
  final String sas;
  bool _closed = false;

  Future<PairingEnrollmentResult> confirm() async {
    _ensureOpen();
    try {
      final confirmation = <String, Object?>{
        'protocolVersion': directTransportProtocolVersion,
        'pairingId': pairingId,
        'confirmed': true,
      };
      final encrypted = await _cipher.encrypt(
        utf8.encode(jsonEncode(confirmation)),
      );
      await _writeRecord(
        _socket,
        encrypted.encode(),
        maximumBytes: _maximumHandshakeRecordBytes,
      ).timeout(_handshakeTimeout);
      final acknowledgementRecord = await _reader
          .readRecord(_maximumHandshakeRecordBytes)
          .timeout(_handshakeTimeout);
      final acknowledgementFrame = DirectEncryptedFrame.decode(
        acknowledgementRecord,
        maximumCiphertextBytes: _maximumHandshakeRecordBytes,
      );
      final acknowledgementValue = jsonDecode(
        utf8.decode(await _cipher.decrypt(acknowledgementFrame)),
      );
      if (acknowledgementValue is! Map) {
        throw const DirectTransportProtocolException(
          'pairing acknowledgement must be a JSON object',
        );
      }
      final acknowledgement = Map<String, Object?>.from(acknowledgementValue);
      if (acknowledgement.keys.toSet().difference(const {
            'protocolVersion',
            'pairingId',
            'awaitingHostConfirmation',
          }).isNotEmpty ||
          acknowledgement.length != 3 ||
          acknowledgement['protocolVersion'] !=
              directTransportProtocolVersion ||
          acknowledgement['pairingId'] != pairingId ||
          acknowledgement['awaitingHostConfirmation'] is! bool) {
        throw const DirectTransportProtocolException(
          'pairing acknowledgement context is invalid',
        );
      }
      return PairingEnrollmentResult(
        hostId: _offer.hostId,
        hostname: _offer.hostname,
        hostIdentityPublicKeyHex: _offer.hostIdentityPublicKeyHex,
        address: _offer.directAddress,
        port: _offer.directPort,
        pairingId: pairingId,
        awaitingHostConfirmation:
            acknowledgement['awaitingHostConfirmation']! as bool,
      );
    } finally {
      await close();
    }
  }

  Future<void> close() async {
    if (_closed) {
      return;
    }
    _closed = true;
    _cipher.destroy();
    await _reader.cancel();
    try {
      await _socket.close();
    } finally {
      _socket.destroy();
    }
  }

  void _ensureOpen() {
    if (_closed) {
      throw StateError('direct pairing session is closed');
    }
  }
}

class JournaledSyncEvent {
  const JournaledSyncEvent({required this.sequence, required this.event});

  final int sequence;
  final wire.Event event;
}

class DirectSyncBatch {
  const DirectSyncBatch({
    required this.snapshot,
    required this.events,
    required this.boundarySequence,
    required this.hostBootEpoch,
  });

  final wire.HostSnapshot? snapshot;
  final List<JournaledSyncEvent> events;
  final int boundarySequence;
  final int hostBootEpoch;
}

class DirectSyncConnection {
  DirectSyncConnection._({
    required Socket socket,
    required _SocketRecordReader reader,
    required DirectSessionCipher cipher,
    required this.hostId,
    required this.deviceId,
    required this.localBootEpoch,
  }) : _socket = socket,
       _reader = reader,
       _cipher = cipher;

  static Future<DirectSyncConnection> connect({
    required String address,
    required int port,
    required PinnedHostIdentity pinnedHost,
    required MobileDeviceIdentity identity,
    required int afterSequence,
    required int knownHostBootEpoch,
    Duration connectTimeout = _defaultConnectTimeout,
  }) async {
    if (address.trim().isEmpty ||
        port < 1 ||
        port > 65535 ||
        afterSequence < 0 ||
        knownHostBootEpoch < 0) {
      throw const DirectTransportProtocolException(
        'direct sync connection parameters are invalid',
      );
    }
    Socket? socket;
    _SocketRecordReader? reader;
    PendingDirectHandshake? pendingHandshake;
    try {
      socket = await Socket.connect(address, port, timeout: connectTimeout);
      socket.setOption(SocketOption.tcpNoDelay, true);
      reader = _SocketRecordReader(socket);
      final initiator = DirectHandshakeInitiator(identity: identity);
      final challengeRecord = await reader
          .readRecord(_maximumHandshakeRecordBytes)
          .timeout(_handshakeTimeout);
      final challenge = await initiator.verifyServerChallenge(
        jsonValue: jsonDecode(utf8.decode(challengeRecord)),
        pinnedHost: pinnedHost,
        nowMs: DateTime.now().millisecondsSinceEpoch,
      );
      pendingHandshake = await initiator.createInitiatorHello(
        challenge: challenge,
      );
      await _writeRecord(
        socket,
        utf8.encode(
          jsonEncode({
            'kind': 'sync',
            'protocolVersion': directTransportProtocolVersion,
            'afterSequence': afterSequence,
            'knownBootEpoch': knownHostBootEpoch,
            'initiator': pendingHandshake.initiator.toJson(),
          }),
        ),
        maximumBytes: _maximumHandshakeRecordBytes,
      ).timeout(_handshakeTimeout);
      final responderRecord = await reader
          .readRecord(_maximumHandshakeRecordBytes)
          .timeout(_handshakeTimeout);
      final keys = await pendingHandshake.finish(
        jsonDecode(utf8.decode(responderRecord)),
      );
      final cipher = DirectSessionCipher(keys);
      keys.destroy();
      return DirectSyncConnection._(
        socket: socket,
        reader: reader,
        cipher: cipher,
        hostId: pinnedHost.hostId,
        deviceId: identity.deviceId,
        localBootEpoch: _newBootEpoch(),
      );
    } on DirectTransportProtocolException {
      pendingHandshake?.abort();
      await reader?.cancel();
      socket?.destroy();
      rethrow;
    } on Object catch (error) {
      pendingHandshake?.abort();
      await reader?.cancel();
      socket?.destroy();
      throw DirectTransportProtocolException(
        'could not establish the direct sync session',
        error,
      );
    }
  }

  final Socket _socket;
  final _SocketRecordReader _reader;
  final DirectSessionCipher _cipher;
  final String hostId;
  final String deviceId;
  final int localBootEpoch;
  int? _remoteBootEpoch;
  bool _closed = false;
  bool _exchangeInFlight = false;

  Future<DirectSyncBatch> readInitialBatch() {
    _ensureOpen();
    return _readBatch();
  }

  Future<DirectSyncBatch> acknowledgeAndPoll({
    required int sequence,
    required int hostBootEpoch,
  }) async {
    _ensureOpen();
    if (_exchangeInFlight || sequence < 0 || hostBootEpoch <= 0) {
      throw const DirectTransportProtocolException(
        'sync acknowledgement context is invalid',
      );
    }
    if (_remoteBootEpoch != hostBootEpoch) {
      throw const DirectTransportProtocolException(
        'sync acknowledgement boot epoch changed',
      );
    }
    _exchangeInFlight = true;
    try {
      final frameSequence = _cipher.nextSendSequence;
      final acknowledgement = wire.MuxportEnvelope(
        header: wire.EnvelopeHeader(
          protocolVersion: directTransportProtocolVersion,
          senderId: deviceId,
          recipientId: hostId,
          bootEpoch: Int64(localBootEpoch),
          sequence: Int64(frameSequence),
          timestampMs: Int64(DateTime.now().millisecondsSinceEpoch),
        ),
        ack: wire.Ack(
          sequenceAcknowledged: Int64(sequence),
          bootEpoch: Int64(hostBootEpoch),
        ),
      );
      final encrypted = await _cipher.encrypt(acknowledgement.writeToBuffer());
      await _writeRecord(
        _socket,
        encrypted.encode(),
        maximumBytes: _maximumEncryptedRecordBytes,
      );
      return await _readBatch();
    } finally {
      _exchangeInFlight = false;
    }
  }

  Future<DirectSyncBatch> _readBatch() async {
    wire.HostSnapshot? snapshot;
    final events = <JournaledSyncEvent>[];
    while (true) {
      final record = await _reader
          .readRecord(_maximumEncryptedRecordBytes)
          .timeout(_handshakeTimeout);
      final frame = DirectEncryptedFrame.decode(record);
      final envelope = wire.MuxportEnvelope.fromBuffer(
        await _cipher.decrypt(frame),
      );
      _validateEnvelope(frame.sequence, envelope);
      if (envelope.hasSnapshot()) {
        if (snapshot != null || events.isNotEmpty) {
          throw const DirectTransportProtocolException(
            'sync batch contains an out-of-order snapshot',
          );
        }
        if (envelope.snapshot.hostId != hostId) {
          throw const DirectTransportProtocolException(
            'sync snapshot belongs to a different host',
          );
        }
        snapshot = envelope.snapshot;
        continue;
      }
      if (envelope.hasEvent()) {
        final journalSequence = int.tryParse(envelope.header.idempotencyKey);
        if (journalSequence == null ||
            journalSequence <= 0 ||
            (events.isNotEmpty &&
                journalSequence != events.last.sequence + 1) ||
            (events.isEmpty &&
                snapshot != null &&
                journalSequence != snapshot.snapshotSequence.toInt() + 1)) {
          throw const DirectTransportProtocolException(
            'sync event journal sequence is invalid',
          );
        }
        events.add(
          JournaledSyncEvent(sequence: journalSequence, event: envelope.event),
        );
        continue;
      }
      if (!envelope.hasAck()) {
        throw const DirectTransportProtocolException(
          'sync batch contains an unexpected payload',
        );
      }
      final boundary = envelope.ack.sequenceAcknowledged.toInt();
      final hostBootEpoch = envelope.ack.bootEpoch.toInt();
      final minimumBoundary = events.isNotEmpty
          ? events.last.sequence
          : snapshot?.snapshotSequence.toInt() ?? 0;
      if (hostBootEpoch != _remoteBootEpoch || boundary < minimumBoundary) {
        throw const DirectTransportProtocolException(
          'sync boundary is inconsistent with the delivered batch',
        );
      }
      return DirectSyncBatch(
        snapshot: snapshot,
        events: List.unmodifiable(events),
        boundarySequence: boundary,
        hostBootEpoch: hostBootEpoch,
      );
    }
  }

  void _validateEnvelope(int frameSequence, wire.MuxportEnvelope envelope) {
    if (!envelope.hasHeader()) {
      throw const DirectTransportProtocolException(
        'sync envelope has no authenticated header',
      );
    }
    final header = envelope.header;
    final bootEpoch = header.bootEpoch.toInt();
    if (header.protocolVersion != directTransportProtocolVersion ||
        header.senderId != hostId ||
        header.recipientId != deviceId ||
        header.sequence.toInt() != frameSequence ||
        bootEpoch <= 0) {
      throw const DirectTransportProtocolException(
        'sync envelope authentication context is invalid',
      );
    }
    final existingEpoch = _remoteBootEpoch;
    if (existingEpoch != null && existingEpoch != bootEpoch) {
      throw const DirectTransportProtocolException(
        'connector boot epoch changed inside the sync session',
      );
    }
    _remoteBootEpoch ??= bootEpoch;
  }

  Future<void> close() async {
    if (_closed) {
      return;
    }
    _closed = true;
    _cipher.destroy();
    await _reader.cancel();
    try {
      await _socket.close();
    } finally {
      _socket.destroy();
    }
  }

  void _ensureOpen() {
    if (_closed) {
      throw StateError('direct sync session is closed');
    }
  }
}

class AuthenticatedDirectConnection {
  AuthenticatedDirectConnection._({
    required Socket socket,
    required _SocketRecordReader reader,
    required DirectSessionCipher cipher,
    required SecretProvisioningCipher provisioningCipher,
    required this.hostId,
    required this.deviceId,
    required this.localBootEpoch,
  }) : _socket = socket,
       _reader = reader,
       _cipher = cipher,
       _provisioningCipher = provisioningCipher;

  static Future<AuthenticatedDirectConnection> connect({
    required String address,
    required int port,
    required PinnedHostIdentity pinnedHost,
    required MobileDeviceIdentity identity,
    Duration connectTimeout = _defaultConnectTimeout,
  }) async {
    if (address.trim().isEmpty || port < 1 || port > 65535) {
      throw const DirectTransportProtocolException(
        'direct connector address is invalid',
      );
    }

    Socket? socket;
    _SocketRecordReader? reader;
    PendingDirectHandshake? pendingHandshake;
    SecretProvisioningCipher? provisioningCipher;
    try {
      socket = await Socket.connect(address, port, timeout: connectTimeout);
      socket.setOption(SocketOption.tcpNoDelay, true);
      reader = _SocketRecordReader(socket);
      final initiator = DirectHandshakeInitiator(identity: identity);
      final challengeRecord = await reader
          .readRecord(_maximumHandshakeRecordBytes)
          .timeout(_handshakeTimeout);
      final challenge = await initiator.verifyServerChallenge(
        jsonValue: jsonDecode(utf8.decode(challengeRecord)),
        pinnedHost: pinnedHost,
        nowMs: DateTime.now().millisecondsSinceEpoch,
      );
      pendingHandshake = await initiator.createInitiatorHello(
        challenge: challenge,
      );
      await _writeRecord(
        socket,
        utf8.encode(jsonEncode(pendingHandshake.initiator.toJson())),
        maximumBytes: _maximumHandshakeRecordBytes,
      ).timeout(_handshakeTimeout);
      final responderRecord = await reader
          .readRecord(_maximumHandshakeRecordBytes)
          .timeout(_handshakeTimeout);
      final keys = await pendingHandshake.finish(
        jsonDecode(utf8.decode(responderRecord)),
      );
      final cipher = DirectSessionCipher(keys);
      provisioningCipher = SecretProvisioningCipher(keys.provisioningKey);
      keys.destroy();
      return AuthenticatedDirectConnection._(
        socket: socket,
        reader: reader,
        cipher: cipher,
        provisioningCipher: provisioningCipher,
        hostId: pinnedHost.hostId,
        deviceId: identity.deviceId,
        localBootEpoch: _newBootEpoch(),
      );
    } on DirectTransportProtocolException {
      pendingHandshake?.abort();
      provisioningCipher?.destroy();
      await reader?.cancel();
      socket?.destroy();
      rethrow;
    } on Object catch (error) {
      pendingHandshake?.abort();
      provisioningCipher?.destroy();
      await reader?.cancel();
      socket?.destroy();
      throw DirectTransportProtocolException(
        'could not establish the direct connector session',
        error,
      );
    }
  }

  final Socket _socket;
  final _SocketRecordReader _reader;
  final DirectSessionCipher _cipher;
  final SecretProvisioningCipher _provisioningCipher;
  final String hostId;
  final String deviceId;
  final int localBootEpoch;
  int? _remoteBootEpoch;
  bool _closed = false;
  bool _commandInFlight = false;

  Future<wire.CommandResult> probe({
    required String commandId,
    required String idempotencyKey,
    Duration deadline = const Duration(seconds: 30),
  }) {
    if (deadline <= Duration.zero) {
      throw ArgumentError.value(deadline, 'deadline', 'must be positive');
    }
    return sendCommand(
      wire.Command(
        commandId: commandId,
        deadlineMs: Int64(DateTime.now().add(deadline).millisecondsSinceEpoch),
        probeHost: wire.ProbeHostCmd(),
      ),
      idempotencyKey: idempotencyKey,
    );
  }

  Future<wire.CommandResult> respondToApproval({
    required String commandId,
    required String idempotencyKey,
    required String runtimeId,
    required String sessionId,
    required String approvalId,
    required bool approved,
    String decisionReason = '',
    Duration deadline = const Duration(seconds: 30),
  }) {
    if (deadline <= Duration.zero ||
        runtimeId.trim().isEmpty ||
        sessionId.trim().isEmpty ||
        approvalId.trim().isEmpty) {
      throw const DirectTransportProtocolException(
        'approval routing identifiers and a positive deadline are required',
      );
    }
    return sendCommand(
      wire.Command(
        commandId: commandId,
        deadlineMs: Int64(DateTime.now().add(deadline).millisecondsSinceEpoch),
        approveAction: wire.ApproveActionCmd(
          approvalId: approvalId,
          approved: approved,
          decisionReason: decisionReason,
          sessionId: sessionId,
          runtimeId: runtimeId,
        ),
      ),
      idempotencyKey: idempotencyKey,
    );
  }

  Future<wire.CommandResult> queryOperation({
    required String commandId,
    required String idempotencyKey,
    required String targetIdempotencyKey,
    Duration deadline = const Duration(seconds: 30),
  }) {
    if (deadline <= Duration.zero || targetIdempotencyKey.trim().isEmpty) {
      throw const DirectTransportProtocolException(
        'operation query target and a positive deadline are required',
      );
    }
    return sendCommand(
      wire.Command(
        commandId: commandId,
        deadlineMs: Int64(DateTime.now().add(deadline).millisecondsSinceEpoch),
        queryOperation: wire.QueryOperationCmd(
          idempotencyKey: targetIdempotencyKey,
        ),
      ),
      idempotencyKey: idempotencyKey,
    );
  }

  /// Consumes and clears [secret] after encrypting it with the dedicated
  /// provisioning key. The plaintext never enters a normal command field.
  Future<wire.CommandResult> provisionCredential({
    required String commandId,
    required String idempotencyKey,
    required String profileId,
    required String displayName,
    required String provider,
    required String credentialType,
    required String accountFingerprint,
    required Uint8List secret,
    Duration deadline = const Duration(seconds: 30),
  }) async {
    if (deadline <= Duration.zero ||
        profileId.trim().isEmpty ||
        displayName.trim().isEmpty ||
        provider.trim().isEmpty ||
        credentialType.trim().isEmpty ||
        accountFingerprint.trim().isEmpty ||
        secret.isEmpty ||
        secret.length > 64 * 1024) {
      secret.fillRange(0, secret.length, 0);
      throw const DirectTransportProtocolException(
        'credential provisioning input is invalid',
      );
    }
    final aad = secretProvisioningAad(
      hostId: hostId,
      deviceId: deviceId,
      commandId: commandId,
      idempotencyKey: idempotencyKey,
      profileId: profileId,
      displayName: displayName,
      provider: provider,
      credentialType: credentialType,
      accountFingerprint: accountFingerprint,
    );
    SealedProvisioningSecret? sealed;
    try {
      sealed = await _provisioningCipher.seal(secret, aad: aad);
      return await sendCommand(
        wire.Command(
          commandId: commandId,
          deadlineMs: Int64(
            DateTime.now().add(deadline).millisecondsSinceEpoch,
          ),
          provisionCredential: wire.ProvisionCredentialCmd(
            profileId: profileId,
            displayName: displayName,
            provider: provider,
            credentialType: credentialType,
            accountFingerprint: accountFingerprint,
            secretNonce: sealed.nonce,
            secretCiphertext: sealed.ciphertext,
          ),
        ),
        idempotencyKey: idempotencyKey,
      );
    } finally {
      secret.fillRange(0, secret.length, 0);
      aad.fillRange(0, aad.length, 0);
      sealed?.destroy();
    }
  }

  Future<wire.CommandResult> sendCommand(
    wire.Command command, {
    required String idempotencyKey,
  }) async {
    _ensureOpen();
    if (_commandInFlight) {
      throw StateError(
        'only one direct command may be in flight per connection',
      );
    }
    if (command.commandId.trim().isEmpty ||
        idempotencyKey.trim().isEmpty ||
        !command.hasDeadlineMs() ||
        command.deadlineMs <= Int64.ZERO) {
      throw const DirectTransportProtocolException(
        'command id, deadline, and idempotency key are required',
      );
    }
    final remainingMs =
        command.deadlineMs.toInt() - DateTime.now().millisecondsSinceEpoch;
    if (remainingMs <= 0) {
      throw const DirectTransportProtocolException(
        'command deadline has already expired',
      );
    }
    _commandInFlight = true;
    try {
      final sequence = _cipher.nextSendSequence;
      final envelope = wire.MuxportEnvelope(
        header: wire.EnvelopeHeader(
          protocolVersion: directTransportProtocolVersion,
          senderId: deviceId,
          recipientId: hostId,
          bootEpoch: Int64(localBootEpoch),
          sequence: Int64(sequence),
          timestampMs: Int64(DateTime.now().millisecondsSinceEpoch),
          idempotencyKey: idempotencyKey,
        ),
        command: command,
      );
      final encrypted = await _cipher.encrypt(envelope.writeToBuffer());
      if (encrypted.sequence != sequence) {
        throw const DirectTransportProtocolException(
          'cipher and envelope sequence diverged',
        );
      }
      await _writeRecord(
        _socket,
        encrypted.encode(),
        maximumBytes: _maximumEncryptedRecordBytes,
      );

      final responseRecord = await _reader
          .readRecord(_maximumEncryptedRecordBytes)
          .timeout(Duration(milliseconds: remainingMs));
      final responseFrame = DirectEncryptedFrame.decode(responseRecord);
      final plaintext = await _cipher.decrypt(responseFrame);
      final response = wire.MuxportEnvelope.fromBuffer(plaintext);
      _validateResponseEnvelope(responseFrame.sequence, response);
      if (!response.hasCommandResult()) {
        if (response.hasError()) {
          throw DirectTransportProtocolException(
            'connector rejected command: ${response.error.message}',
          );
        }
        throw const DirectTransportProtocolException(
          'connector response is not a command result',
        );
      }
      if (response.commandResult.commandId != command.commandId) {
        throw const DirectTransportProtocolException(
          'command result does not match the submitted command',
        );
      }
      return response.commandResult;
    } on DirectTransportProtocolException {
      await _closeAfterFailure();
      rethrow;
    } on Object catch (error) {
      await _closeAfterFailure();
      throw DirectTransportProtocolException(
        'direct command exchange failed',
        error,
      );
    } finally {
      _commandInFlight = false;
    }
  }

  Future<void> close() async {
    if (_closed) {
      return;
    }
    _closed = true;
    _cipher.destroy();
    _provisioningCipher.destroy();
    await _reader.cancel();
    try {
      await _socket.close();
    } finally {
      _socket.destroy();
    }
  }

  Future<void> _closeAfterFailure() async {
    try {
      await close();
    } on Object {
      // Preserve the authenticated protocol/transport error that caused the
      // connection to become unusable.
    }
  }

  void _validateResponseEnvelope(
    int frameSequence,
    wire.MuxportEnvelope envelope,
  ) {
    if (!envelope.hasHeader()) {
      throw const DirectTransportProtocolException(
        'connector response has no secure envelope header',
      );
    }
    final header = envelope.header;
    final bootEpoch = header.bootEpoch.toInt();
    if (header.protocolVersion != directTransportProtocolVersion ||
        header.senderId != hostId ||
        header.recipientId != deviceId ||
        header.sequence.toInt() != frameSequence ||
        bootEpoch <= 0) {
      throw const DirectTransportProtocolException(
        'connector response envelope context is invalid',
      );
    }
    final existingEpoch = _remoteBootEpoch;
    if (existingEpoch != null && existingEpoch != bootEpoch) {
      throw const DirectTransportProtocolException(
        'connector boot epoch changed inside the encrypted session',
      );
    }
    _remoteBootEpoch ??= bootEpoch;
  }

  void _ensureOpen() {
    if (_closed) {
      throw StateError('direct connector session is closed');
    }
  }
}

class _SocketRecordReader {
  _SocketRecordReader(Socket socket)
    : _iterator = StreamIterator<Uint8List>(socket);

  final StreamIterator<Uint8List> _iterator;
  final List<int> _buffer = [];
  bool _cancelled = false;

  Future<Uint8List> readRecord(int maximumBytes) async {
    if (_cancelled || maximumBytes <= 0) {
      throw const DirectTransportProtocolException(
        'transport record reader is unavailable',
      );
    }
    final lengthBytes = await _readExactly(4);
    final length =
        (lengthBytes[0] << 24) |
        (lengthBytes[1] << 16) |
        (lengthBytes[2] << 8) |
        lengthBytes[3];
    if (length <= 0 || length > maximumBytes) {
      throw const DirectTransportProtocolException(
        'transport record exceeded its configured size',
      );
    }
    return _readExactly(length);
  }

  Future<Uint8List> _readExactly(int length) async {
    while (_buffer.length < length) {
      if (!await _iterator.moveNext()) {
        throw const DirectTransportProtocolException(
          'connector closed the transport unexpectedly',
        );
      }
      _buffer.addAll(_iterator.current);
    }
    final output = Uint8List.fromList(_buffer.take(length).toList());
    _buffer.removeRange(0, length);
    return output;
  }

  Future<void> cancel() async {
    if (_cancelled) {
      return;
    }
    _cancelled = true;
    _buffer.fillRange(0, _buffer.length, 0);
    _buffer.clear();
    await _iterator.cancel();
  }
}

Future<void> _writeRecord(
  Socket socket,
  List<int> bytes, {
  required int maximumBytes,
}) async {
  if (bytes.isEmpty || bytes.length > maximumBytes) {
    throw const DirectTransportProtocolException(
      'outbound transport record exceeded its configured size',
    );
  }
  final length = ByteData(4)..setUint32(0, bytes.length, Endian.big);
  socket
    ..add(length.buffer.asUint8List())
    ..add(bytes);
  await socket.flush();
}

int _newBootEpoch() {
  final random = Random.secure();
  final high = random.nextInt(0x80000000);
  final low = random.nextInt(0x100000000);
  final value = (high << 32) | low;
  return value == 0 ? 1 : value;
}
