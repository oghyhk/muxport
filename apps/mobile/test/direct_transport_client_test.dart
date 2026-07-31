import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:crypto/crypto.dart' as hashes;
import 'package:cryptography/cryptography.dart';
import 'package:fixnum/fixnum.dart';
import 'package:flutter_protocol/wire_protocol.dart' as wire;
import 'package:muxport_mobile/pairing/signed_pairing_offer.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/security/device_identity.dart';
import 'package:muxport_mobile/transport/direct_transport_client.dart';
import 'package:muxport_mobile/transport/direct_transport_protocol.dart';

void main() {
  test('mobile sync client applies snapshot replay and polls by ack', () async {
    final ed25519 = Ed25519();
    final hostIdentity = await ed25519.newKeyPairFromSeed(
      List<int>.generate(32, (index) => index + 61),
    );
    final hostPublic = await hostIdentity.extractPublicKey();
    final mobileIdentity = await DeviceIdentityManager(
      secureStore: _MemorySecureStore(),
      random: Random(11),
    ).loadOrCreate();
    final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    final serverTask = _serveSync(
      server: server,
      hostIdentity: hostIdentity,
      hostPublicHex: _hex(hostPublic.bytes),
      expectedDeviceId: mobileIdentity.deviceId,
    );

    final connection = await DirectSyncConnection.connect(
      address: InternetAddress.loopbackIPv4.address,
      port: server.port,
      pinnedHost: PinnedHostIdentity(
        hostId: 'host-1',
        publicKeyHex: _hex(hostPublic.bytes),
      ),
      identity: mobileIdentity,
      afterSequence: 0,
      knownHostBootEpoch: 0,
    );
    final initial = await connection.readInitialBatch();

    expect(initial.hostBootEpoch, 22);
    expect(initial.snapshot?.hostId, 'host-1');
    expect(initial.snapshot?.snapshotSequence.toInt(), 1);
    expect(initial.events, hasLength(1));
    expect(initial.events.single.sequence, 2);
    expect(initial.events.single.event.eventId, 'event-2');
    expect(initial.boundarySequence, 2);

    final empty = await connection.acknowledgeAndPoll(
      sequence: initial.boundarySequence,
      hostBootEpoch: initial.hostBootEpoch,
    );
    expect(empty.snapshot, isNull);
    expect(empty.events, isEmpty);
    expect(empty.boundarySequence, 2);

    await connection.close();
    await serverTask;
    await mobileIdentity.destroy();
    hostIdentity.destroy();
    await server.close();
  });

  test('mobile client completes encrypted pairing confirmation', () async {
    final ed25519 = Ed25519();
    final hostIdentity = await ed25519.newKeyPairFromSeed(
      List<int>.generate(32, (index) => index + 21),
    );
    final hostPublic = await hostIdentity.extractPublicKey();
    final hostEphemeral = await X25519().newKeyPair();
    final hostEphemeralPublic = await hostEphemeral.extractPublicKey();
    final mobileIdentity = await DeviceIdentityManager(
      secureStore: _MemorySecureStore(),
      random: Random(5),
    ).loadOrCreate();
    final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    final offerJson = await _signedPairingOffer(
      hostIdentity: hostIdentity,
      hostPublicHex: _hex(hostPublic.bytes),
      hostEphemeralPublicHex: _hex(hostEphemeralPublic.bytes),
      directEndpoint: '${InternetAddress.loopbackIPv4.address}:${server.port}',
    );
    final offer = await SignedPairingOffer.parseAndVerify(
      jsonEncode(offerJson),
      nowMs: DateTime.now().millisecondsSinceEpoch,
    );
    final expectedSas = Completer<String>();
    final serverTask = _servePairing(
      server: server,
      hostIdentity: hostIdentity,
      hostEphemeral: hostEphemeral,
      hostPublicHex: _hex(hostPublic.bytes),
      hostEphemeralPublicHex: _hex(hostEphemeralPublic.bytes),
      rendezvousToken: offer.rendezvousToken,
      expectedDeviceId: mobileIdentity.deviceId,
      expectedDevicePublicKeyHex: _hex(mobileIdentity.publicKeyBytes),
      sas: expectedSas,
    );

    final pairing = await PendingDirectPairing.connect(
      offer: offer,
      identity: mobileIdentity,
      deviceName: 'Test phone',
    );
    expect(pairing.sas, await expectedSas.future);
    expect(pairing.sas, matches(RegExp(r'^\d{6}$')));
    final enrollment = await pairing.confirm();

    expect(enrollment.hostId, 'host-1');
    expect(enrollment.pairingId, 'pairing-live-1');
    expect(enrollment.awaitingHostConfirmation, isTrue);
    expect(enrollment.pinnedHost.publicKeyHex, _hex(hostPublic.bytes));

    await serverTask;
    await mobileIdentity.destroy();
    hostIdentity.destroy();
    await server.close();
  });

  test('mobile client completes authenticated encrypted probe', () async {
    final ed25519 = Ed25519();
    final hostIdentity = await ed25519.newKeyPairFromSeed(
      List<int>.generate(32, (index) => index + 1),
    );
    final hostPublic = await hostIdentity.extractPublicKey();
    final hostPublicHex = _hex(hostPublic.bytes);
    final mobileIdentity = await DeviceIdentityManager(
      secureStore: _MemorySecureStore(),
      random: Random(7),
    ).loadOrCreate();
    final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
    final serverTask = _serveProbe(
      server: server,
      hostIdentity: hostIdentity,
      hostPublicHex: hostPublicHex,
      expectedDeviceId: mobileIdentity.deviceId,
      expectedDevicePublicKeyHex: _hex(mobileIdentity.publicKeyBytes),
    );

    final connection = await AuthenticatedDirectConnection.connect(
      address: InternetAddress.loopbackIPv4.address,
      port: server.port,
      pinnedHost: PinnedHostIdentity(
        hostId: 'host-1',
        publicKeyHex: hostPublicHex,
      ),
      identity: mobileIdentity,
    );
    final result = await connection.probe(
      commandId: 'probe-1',
      idempotencyKey: 'probe-idempotency-1',
    );

    expect(result.commandId, 'probe-1');
    expect(result.success, isTrue);
    expect(result.state, wire.RemoteOpState.REMOTE_OP_STATE_SUCCEEDED);
    expect(result.resultJson, '{"status":"ok"}');

    await connection.close();
    await serverTask;
    await mobileIdentity.destroy();
    hostIdentity.destroy();
    await server.close();
  });

  test(
    'mobile client rejects a challenge outside the pinned identity',
    () async {
      final ed25519 = Ed25519();
      final actualHost = await ed25519.newKeyPair();
      final actualHostPublic = await actualHost.extractPublicKey();
      final pinnedHost = await ed25519.newKeyPair();
      final pinnedHostPublic = await pinnedHost.extractPublicKey();
      final mobileIdentity = await DeviceIdentityManager(
        secureStore: _MemorySecureStore(),
        random: Random(9),
      ).loadOrCreate();
      final server = await ServerSocket.bind(InternetAddress.loopbackIPv4, 0);
      final serverTask = () async {
        final socket = await server.first;
        final challenge = await _signedChallenge(
          hostIdentity: actualHost,
          hostPublicHex: _hex(actualHostPublic.bytes),
        );
        await _writeRecord(socket, utf8.encode(jsonEncode(challenge)));
        await socket.close();
      }();

      await expectLater(
        AuthenticatedDirectConnection.connect(
          address: InternetAddress.loopbackIPv4.address,
          port: server.port,
          pinnedHost: PinnedHostIdentity(
            hostId: 'host-1',
            publicKeyHex: _hex(pinnedHostPublic.bytes),
          ),
          identity: mobileIdentity,
        ),
        throwsA(isA<DirectTransportProtocolException>()),
      );

      await serverTask;
      await mobileIdentity.destroy();
      actualHost.destroy();
      pinnedHost.destroy();
      await server.close();
    },
  );

  test('ordered frame codec rejects replay', () async {
    final initiatorKeys = DirectSessionKeys(
      sendKey: List<int>.filled(32, 1),
      receiveKey: List<int>.filled(32, 2),
      sendNoncePrefix: const [3, 4, 5, 6],
      receiveNoncePrefix: const [7, 8, 9, 10],
      aad: utf8.encode('bound-session'),
    );
    final responderKeys = DirectSessionKeys(
      sendKey: List<int>.filled(32, 2),
      receiveKey: List<int>.filled(32, 1),
      sendNoncePrefix: const [7, 8, 9, 10],
      receiveNoncePrefix: const [3, 4, 5, 6],
      aad: utf8.encode('bound-session'),
    );
    final initiator = DirectSessionCipher(initiatorKeys);
    final responder = DirectSessionCipher(responderKeys);
    initiatorKeys.destroy();
    responderKeys.destroy();

    final frame = await initiator.encrypt(utf8.encode('probe'));
    final decoded = DirectEncryptedFrame.decode(frame.encode());
    expect(utf8.decode(await responder.decrypt(decoded)), 'probe');
    await expectLater(
      responder.decrypt(decoded),
      throwsA(isA<DirectTransportProtocolException>()),
    );

    initiator.destroy();
    responder.destroy();
  });

  test('Dart protocol matches the committed cross-language fixture', () async {
    final fixture = Map<String, Object?>.from(
      jsonDecode(
            File(
              '../../protocol/fixtures/direct_session_v1.json',
            ).readAsStringSync(),
          )
          as Map,
    );
    final sharedSecret = SecretKeyData(
      _unhex(fixture['sharedSecretHex']! as String),
      overwriteWhenDestroyed: true,
    );
    final keys = await deriveInitiatorDirectSessionKeys(
      sharedSecret: sharedSecret,
      transcript: _unhex(fixture['transcriptHex']! as String),
      hostId: fixture['hostId']! as String,
      deviceId: fixture['deviceId']! as String,
    );
    sharedSecret.destroy();

    expect(_hex(keys.sendKey), fixture['initiatorToResponderKeyHex']);
    expect(_hex(keys.receiveKey), fixture['responderToInitiatorKeyHex']);
    expect(_hex(keys.sendNoncePrefix), fixture['initiatorNoncePrefixHex']);
    expect(_hex(keys.receiveNoncePrefix), fixture['responderNoncePrefixHex']);
    expect(_hex(keys.aad), fixture['sessionAadHex']);

    final envelope = wire.MuxportEnvelope(
      header: wire.EnvelopeHeader(
        protocolVersion: directTransportProtocolVersion,
        senderId: fixture['deviceId']! as String,
        recipientId: fixture['hostId']! as String,
        bootEpoch: Int64.parseInt('72623859790382856'),
        sequence: Int64.ONE,
        timestampMs: Int64(1700000000000),
        idempotencyKey: 'fixture-idempotency-1',
      ),
      command: wire.Command(
        commandId: 'fixture-probe-1',
        deadlineMs: Int64(1700000060000),
        probeHost: wire.ProbeHostCmd(),
      ),
    );
    expect(_hex(envelope.writeToBuffer()), fixture['envelopeHex']);

    final cipher = DirectSessionCipher(keys);
    final frame = await cipher.encrypt(envelope.writeToBuffer());
    expect(_hex(frame.encode()), fixture['encryptedFrameHex']);
    cipher.destroy();
    keys.destroy();
  });
}

Future<void> _serveSync({
  required ServerSocket server,
  required SimpleKeyPair hostIdentity,
  required String hostPublicHex,
  required String expectedDeviceId,
}) async {
  final socket = await server.first;
  final reader = _TestRecordReader(socket);
  try {
    final challenge = await _signedChallenge(
      hostIdentity: hostIdentity,
      hostPublicHex: hostPublicHex,
    );
    await _writeRecord(socket, utf8.encode(jsonEncode(challenge)));
    final request = Map<String, Object?>.from(
      jsonDecode(utf8.decode(await reader.readRecord())) as Map,
    );
    expect(request['kind'], 'sync');
    expect(request['afterSequence'], 0);
    expect(request['knownBootEpoch'], 0);
    final initiator = Map<String, Object?>.from(request['initiator']! as Map);
    expect(initiator['deviceId'], expectedDeviceId);

    final hostEphemeral = await X25519().newKeyPair();
    final hostEphemeralPublic = await hostEphemeral.extractPublicKey();
    final responder = <String, Object?>{
      'protocolVersion': directTransportProtocolVersion,
      'hostId': 'host-1',
      'deviceId': expectedDeviceId,
      'hostIdentityPublicKeyHex': hostPublicHex,
      'ephemeralPublicKeyHex': _hex(hostEphemeralPublic.bytes),
      'nonceHex': _hex(List<int>.filled(32, 23)),
      'initiatorHashHex': _hex(_initiatorHash(initiator)),
      'signatureHex': '',
    };
    final responderSignature = await Ed25519().sign(
      _responderClaim(responder),
      keyPair: hostIdentity,
    );
    responder['signatureHex'] = _hex(responderSignature.bytes);
    await _writeRecord(socket, utf8.encode(jsonEncode(responder)));

    final transcript = _transcript(initiator, responder);
    final dh = await X25519().sharedSecretKey(
      keyPair: hostEphemeral,
      remotePublicKey: SimplePublicKey(
        _unhex(initiator['ephemeralPublicKeyHex']! as String),
        type: KeyPairType.x25519,
      ),
    );
    final dhBytes = Uint8List.fromList(await dh.extractBytes());
    dh.destroy();
    hostEphemeral.destroy();
    final transcriptHash = hashes.sha256.convert(transcript).bytes;
    final dhInput = SecretKeyData(dhBytes, overwriteWhenDestroyed: true);
    final shared = await Hkdf(hmac: Hmac.sha256(), outputLength: 32).deriveKey(
      secretKey: dhInput,
      nonce: transcriptHash,
      info: utf8.encode('muxport-shared-secret-v1'),
    );
    dhInput.destroy();
    dhBytes.fillRange(0, dhBytes.length, 0);
    final directional = await Hkdf(hmac: Hmac.sha256(), outputLength: 72)
        .deriveKey(
          secretKey: shared,
          nonce: transcriptHash,
          info: utf8.encode('muxport-directional-session-v1'),
        );
    shared.destroy();
    final material = Uint8List.fromList(await directional.extractBytes());
    directional.destroy();
    final hostKeys = DirectSessionKeys(
      sendKey: material.sublist(32, 64),
      receiveKey: material.sublist(0, 32),
      sendNoncePrefix: material.sublist(68, 72),
      receiveNoncePrefix: material.sublist(64, 68),
      aad: _sessionAad('host-1', expectedDeviceId, transcript),
    );
    material.fillRange(0, material.length, 0);
    final cipher = DirectSessionCipher(hostKeys);
    hostKeys.destroy();

    await _writeSyncEnvelope(
      socket,
      cipher,
      wire.MuxportEnvelope(
        header: _hostHeader(
          deviceId: expectedDeviceId,
          sequence: cipher.nextSendSequence,
        ),
        snapshot: wire.HostSnapshot(
          hostId: 'host-1',
          hostname: 'Test host',
          connectorState: wire.ConnectorState.CONNECTOR_STATE_READY,
          snapshotSequence: Int64.ONE,
        ),
      ),
    );
    await _writeSyncEnvelope(
      socket,
      cipher,
      wire.MuxportEnvelope(
        header: _hostHeader(
          deviceId: expectedDeviceId,
          sequence: cipher.nextSendSequence,
          cursor: '2',
        ),
        event: wire.Event(eventId: 'event-2', timestampMs: Int64(2)),
      ),
    );
    await _writeSyncEnvelope(
      socket,
      cipher,
      wire.MuxportEnvelope(
        header: _hostHeader(
          deviceId: expectedDeviceId,
          sequence: cipher.nextSendSequence,
        ),
        ack: wire.Ack(sequenceAcknowledged: Int64(2), bootEpoch: Int64(22)),
      ),
    );

    final acknowledgementFrame = DirectEncryptedFrame.decode(
      await reader.readRecord(),
    );
    final acknowledgement = wire.MuxportEnvelope.fromBuffer(
      await cipher.decrypt(acknowledgementFrame),
    );
    expect(acknowledgement.ack.sequenceAcknowledged.toInt(), 2);
    expect(acknowledgement.ack.bootEpoch.toInt(), 22);
    await _writeSyncEnvelope(
      socket,
      cipher,
      wire.MuxportEnvelope(
        header: _hostHeader(
          deviceId: expectedDeviceId,
          sequence: cipher.nextSendSequence,
        ),
        ack: wire.Ack(sequenceAcknowledged: Int64(2), bootEpoch: Int64(22)),
      ),
    );
    cipher.destroy();
  } finally {
    await reader.cancel();
    await socket.close();
  }
}

wire.EnvelopeHeader _hostHeader({
  required String deviceId,
  required int sequence,
  String cursor = '',
}) {
  return wire.EnvelopeHeader(
    protocolVersion: directTransportProtocolVersion,
    senderId: 'host-1',
    recipientId: deviceId,
    bootEpoch: Int64(22),
    sequence: Int64(sequence),
    timestampMs: Int64(DateTime.now().millisecondsSinceEpoch),
    idempotencyKey: cursor,
  );
}

Future<void> _writeSyncEnvelope(
  Socket socket,
  DirectSessionCipher cipher,
  wire.MuxportEnvelope envelope,
) async {
  final frame = await cipher.encrypt(envelope.writeToBuffer());
  await _writeRecord(socket, frame.encode());
}

Future<Map<String, Object?>> _signedPairingOffer({
  required SimpleKeyPair hostIdentity,
  required String hostPublicHex,
  required String hostEphemeralPublicHex,
  required String directEndpoint,
}) async {
  final issuedAt = DateTime.now().millisecondsSinceEpoch;
  final offer = <String, Object?>{
    'protocolVersion': directTransportProtocolVersion,
    'hostId': 'host-1',
    'hostname': 'Test host',
    'hostIdentityPublicKeyHex': hostPublicHex,
    'directEndpoint': directEndpoint,
    'rendezvousToken': _hex(List<int>.generate(32, (index) => index + 41)),
    'ephemeralPublicKeyHex': hostEphemeralPublicHex,
    'issuedAtMs': issuedAt,
    'expiresAtMs': issuedAt + 60000,
    'signatureHex': '',
  };
  final signed =
      (BytesBuilder(copy: false)
            ..add(_field(utf8.encode('muxport-pairing-offer-v1')))
            ..add(_u32(offer['protocolVersion']! as int))
            ..add(_field(utf8.encode(offer['hostId']! as String)))
            ..add(_field(utf8.encode(offer['hostname']! as String)))
            ..add(_field(_unhex(hostPublicHex)))
            ..add(_field(utf8.encode(directEndpoint)))
            ..add(_field(_unhex(offer['rendezvousToken']! as String)))
            ..add(_field(_unhex(hostEphemeralPublicHex)))
            ..add(_i64(issuedAt))
            ..add(_i64(issuedAt + 60000)))
          .takeBytes();
  final signature = await Ed25519().sign(signed, keyPair: hostIdentity);
  offer['signatureHex'] = _hex(signature.bytes);
  return offer;
}

Future<void> _servePairing({
  required ServerSocket server,
  required SimpleKeyPair hostIdentity,
  required SimpleKeyPair hostEphemeral,
  required String hostPublicHex,
  required String hostEphemeralPublicHex,
  required String rendezvousToken,
  required String expectedDeviceId,
  required String expectedDevicePublicKeyHex,
  required Completer<String> sas,
}) async {
  final socket = await server.first;
  final reader = _TestRecordReader(socket);
  try {
    final challenge = await _signedChallenge(
      hostIdentity: hostIdentity,
      hostPublicHex: hostPublicHex,
    );
    await _writeRecord(socket, utf8.encode(jsonEncode(challenge)));

    final request = Map<String, Object?>.from(
      jsonDecode(utf8.decode(await reader.readRecord())) as Map,
    );
    expect(request['kind'], 'pairing');
    expect(request['rendezvousToken'], rendezvousToken);
    final initiator = Map<String, Object?>.from(request['initiator']! as Map);
    expect(initiator['deviceId'], expectedDeviceId);
    expect(initiator['deviceIdentityPublicKeyHex'], expectedDevicePublicKeyHex);
    expect(
      initiator['challenge'],
      _pairingChallenge(rendezvousToken, challenge['challenge']! as String),
    );
    expect(
      await Ed25519().verify(
        _initiatorClaim(initiator),
        signature: Signature(
          _unhex(initiator['signatureHex']! as String),
          publicKey: SimplePublicKey(
            _unhex(expectedDevicePublicKeyHex),
            type: KeyPairType.ed25519,
          ),
        ),
      ),
      isTrue,
    );

    final responder = <String, Object?>{
      'protocolVersion': directTransportProtocolVersion,
      'hostId': 'host-1',
      'deviceId': expectedDeviceId,
      'hostIdentityPublicKeyHex': hostPublicHex,
      'ephemeralPublicKeyHex': hostEphemeralPublicHex,
      'nonceHex': _hex(List<int>.filled(32, 19)),
      'initiatorHashHex': _hex(_initiatorHash(initiator)),
      'signatureHex': '',
    };
    final responderSignature = await Ed25519().sign(
      _responderClaim(responder),
      keyPair: hostIdentity,
    );
    responder['signatureHex'] = _hex(responderSignature.bytes);
    await _writeRecord(
      socket,
      utf8.encode(
        jsonEncode({
          'protocolVersion': directTransportProtocolVersion,
          'pairingId': 'pairing-live-1',
          'responder': responder,
        }),
      ),
    );

    final transcript = _transcript(initiator, responder);
    final dh = await X25519().sharedSecretKey(
      keyPair: hostEphemeral,
      remotePublicKey: SimplePublicKey(
        _unhex(initiator['ephemeralPublicKeyHex']! as String),
        type: KeyPairType.x25519,
      ),
    );
    final dhBytes = Uint8List.fromList(await dh.extractBytes());
    dh.destroy();
    hostEphemeral.destroy();
    final transcriptHash = hashes.sha256.convert(transcript).bytes;
    final dhInput = SecretKeyData(dhBytes, overwriteWhenDestroyed: true);
    final shared = await Hkdf(hmac: Hmac.sha256(), outputLength: 32).deriveKey(
      secretKey: dhInput,
      nonce: transcriptHash,
      info: utf8.encode('muxport-shared-secret-v1'),
    );
    dhInput.destroy();
    dhBytes.fillRange(0, dhBytes.length, 0);
    final sasKey = await Hkdf(hmac: Hmac.sha256(), outputLength: 4).deriveKey(
      secretKey: shared,
      nonce: transcriptHash,
      info: utf8.encode('muxport-sas-v1'),
    );
    final sasBytes = Uint8List.fromList(await sasKey.extractBytes());
    sasKey.destroy();
    sas.complete(
      (ByteData.sublistView(sasBytes).getUint32(0, Endian.big) % 1000000)
          .toString()
          .padLeft(6, '0'),
    );
    sasBytes.fillRange(0, sasBytes.length, 0);

    final directional = await Hkdf(hmac: Hmac.sha256(), outputLength: 72)
        .deriveKey(
          secretKey: shared,
          nonce: transcriptHash,
          info: utf8.encode('muxport-directional-session-v1'),
        );
    shared.destroy();
    final material = Uint8List.fromList(await directional.extractBytes());
    directional.destroy();
    final hostKeys = DirectSessionKeys(
      sendKey: material.sublist(32, 64),
      receiveKey: material.sublist(0, 32),
      sendNoncePrefix: material.sublist(68, 72),
      receiveNoncePrefix: material.sublist(64, 68),
      aad: transcript,
    );
    material.fillRange(0, material.length, 0);
    final cipher = DirectSessionCipher(hostKeys);
    hostKeys.destroy();

    final confirmationFrame = DirectEncryptedFrame.decode(
      await reader.readRecord(),
      maximumCiphertextBytes: 16 * 1024,
    );
    final confirmation = Map<String, Object?>.from(
      jsonDecode(utf8.decode(await cipher.decrypt(confirmationFrame))) as Map,
    );
    expect(confirmation['protocolVersion'], directTransportProtocolVersion);
    expect(confirmation['pairingId'], 'pairing-live-1');
    expect(confirmation['confirmed'], isTrue);

    final acknowledgement = await cipher.encrypt(
      utf8.encode(
        jsonEncode({
          'protocolVersion': directTransportProtocolVersion,
          'pairingId': 'pairing-live-1',
          'awaitingHostConfirmation': true,
        }),
      ),
    );
    await _writeRecord(socket, acknowledgement.encode());
    cipher.destroy();
  } finally {
    if (!sas.isCompleted) {
      sas.completeError(StateError('pairing host stopped before SAS'));
    }
    await reader.cancel();
    await socket.close();
  }
}

Future<void> _serveProbe({
  required ServerSocket server,
  required SimpleKeyPair hostIdentity,
  required String hostPublicHex,
  required String expectedDeviceId,
  required String expectedDevicePublicKeyHex,
}) async {
  final socket = await server.first;
  final reader = _TestRecordReader(socket);
  try {
    final challenge = await _signedChallenge(
      hostIdentity: hostIdentity,
      hostPublicHex: hostPublicHex,
    );
    await _writeRecord(socket, utf8.encode(jsonEncode(challenge)));

    final initiator = Map<String, Object?>.from(
      jsonDecode(utf8.decode(await reader.readRecord())) as Map,
    );
    expect(initiator['protocolVersion'], directTransportProtocolVersion);
    expect(initiator['hostId'], 'host-1');
    expect(initiator['deviceId'], expectedDeviceId);
    expect(initiator['deviceIdentityPublicKeyHex'], expectedDevicePublicKeyHex);
    final initiatorSignatureValid = await Ed25519().verify(
      _initiatorClaim(initiator),
      signature: Signature(
        _unhex(initiator['signatureHex']! as String),
        publicKey: SimplePublicKey(
          _unhex(initiator['deviceIdentityPublicKeyHex']! as String),
          type: KeyPairType.ed25519,
        ),
      ),
    );
    expect(initiatorSignatureValid, isTrue);

    final hostEphemeral = await X25519().newKeyPair();
    final hostEphemeralPublic = await hostEphemeral.extractPublicKey();
    final responder = <String, Object?>{
      'protocolVersion': directTransportProtocolVersion,
      'hostId': 'host-1',
      'deviceId': expectedDeviceId,
      'hostIdentityPublicKeyHex': hostPublicHex,
      'ephemeralPublicKeyHex': _hex(hostEphemeralPublic.bytes),
      'nonceHex': _hex(List<int>.filled(32, 13)),
      'initiatorHashHex': _hex(_initiatorHash(initiator)),
      'signatureHex': '',
    };
    final responderSignature = await Ed25519().sign(
      _responderClaim(responder),
      keyPair: hostIdentity,
    );
    responder['signatureHex'] = _hex(responderSignature.bytes);
    await _writeRecord(socket, utf8.encode(jsonEncode(responder)));

    final transcript = _transcript(initiator, responder);
    final dh = await X25519().sharedSecretKey(
      keyPair: hostEphemeral,
      remotePublicKey: SimplePublicKey(
        _unhex(initiator['ephemeralPublicKeyHex']! as String),
        type: KeyPairType.x25519,
      ),
    );
    final dhBytes = Uint8List.fromList(await dh.extractBytes());
    dh.destroy();
    hostEphemeral.destroy();
    final transcriptHash = hashes.sha256.convert(transcript).bytes;
    final shared = await Hkdf(hmac: Hmac.sha256(), outputLength: 32).deriveKey(
      secretKey: SecretKeyData(dhBytes, overwriteWhenDestroyed: true),
      nonce: transcriptHash,
      info: utf8.encode('muxport-shared-secret-v1'),
    );
    dhBytes.fillRange(0, dhBytes.length, 0);
    final directional = await Hkdf(hmac: Hmac.sha256(), outputLength: 72)
        .deriveKey(
          secretKey: shared,
          nonce: transcriptHash,
          info: utf8.encode('muxport-directional-session-v1'),
        );
    shared.destroy();
    final material = Uint8List.fromList(await directional.extractBytes());
    directional.destroy();
    final hostKeys = DirectSessionKeys(
      sendKey: material.sublist(32, 64),
      receiveKey: material.sublist(0, 32),
      sendNoncePrefix: material.sublist(68, 72),
      receiveNoncePrefix: material.sublist(64, 68),
      aad: _sessionAad('host-1', expectedDeviceId, transcript),
    );
    material.fillRange(0, material.length, 0);
    final cipher = DirectSessionCipher(hostKeys);
    hostKeys.destroy();

    final requestFrame = DirectEncryptedFrame.decode(await reader.readRecord());
    final request = wire.MuxportEnvelope.fromBuffer(
      await cipher.decrypt(requestFrame),
    );
    expect(request.header.senderId, expectedDeviceId);
    expect(request.header.recipientId, 'host-1');
    expect(request.header.sequence.toInt(), requestFrame.sequence);
    expect(request.command.commandId, 'probe-1');
    expect(request.command.hasProbeHost(), isTrue);
    expect(request.header.idempotencyKey, 'probe-idempotency-1');

    final responseSequence = cipher.nextSendSequence;
    final response = wire.MuxportEnvelope(
      header: wire.EnvelopeHeader(
        protocolVersion: directTransportProtocolVersion,
        senderId: 'host-1',
        recipientId: expectedDeviceId,
        bootEpoch: Int64(22),
        sequence: Int64(responseSequence),
        timestampMs: Int64(DateTime.now().millisecondsSinceEpoch),
      ),
      commandResult: wire.CommandResult(
        commandId: request.command.commandId,
        state: wire.RemoteOpState.REMOTE_OP_STATE_SUCCEEDED,
        success: true,
        completedAtMs: Int64(DateTime.now().millisecondsSinceEpoch),
        resultJson: '{"status":"ok"}',
      ),
    );
    final encryptedResponse = await cipher.encrypt(response.writeToBuffer());
    expect(encryptedResponse.sequence, responseSequence);
    await _writeRecord(socket, encryptedResponse.encode());
    cipher.destroy();
  } finally {
    await reader.cancel();
    await socket.close();
  }
}

String _pairingChallenge(String token, String challenge) {
  final bytes =
      (BytesBuilder(copy: false)
            ..add(
              _field(utf8.encode('muxport-pairing-connection-challenge-v1')),
            )
            ..add(_field(_unhex(token)))
            ..add(_field(_unhex(challenge))))
          .takeBytes();
  return _hex(hashes.sha256.convert(bytes).bytes);
}

Future<Map<String, Object?>> _signedChallenge({
  required SimpleKeyPair hostIdentity,
  required String hostPublicHex,
}) async {
  final issuedAt = DateTime.now().millisecondsSinceEpoch;
  final challenge = <String, Object?>{
    'protocolVersion': directTransportProtocolVersion,
    'hostId': 'host-1',
    'hostIdentityPublicKeyHex': hostPublicHex,
    'bootEpoch': 22,
    'challenge': _hex(List<int>.filled(32, 7)),
    'issuedAtMs': issuedAt,
    'expiresAtMs': issuedAt + directTransportChallengeLifetimeMs,
    'signatureHex': '',
  };
  final signedBytes = BytesBuilder(copy: false)
    ..add(utf8.encode('muxport-server-challenge-v1'))
    ..add(_field(_u32(challenge['protocolVersion']! as int)))
    ..add(_field(utf8.encode(challenge['hostId']! as String)))
    ..add(_field(utf8.encode(challenge['hostIdentityPublicKeyHex']! as String)))
    ..add(_field(_u64(challenge['bootEpoch']! as int)))
    ..add(_field(utf8.encode(challenge['challenge']! as String)))
    ..add(_field(_i64(challenge['issuedAtMs']! as int)))
    ..add(_field(_i64(challenge['expiresAtMs']! as int)));
  final signature = await Ed25519().sign(
    signedBytes.takeBytes(),
    keyPair: hostIdentity,
  );
  challenge['signatureHex'] = _hex(signature.bytes);
  return challenge;
}

Uint8List _initiatorClaim(Map<String, Object?> hello) {
  return (BytesBuilder(copy: false)
        ..add(_field(utf8.encode('muxport-initiator-hello-v1')))
        ..add(_u32(hello['protocolVersion']! as int))
        ..add(_field(utf8.encode(hello['deviceId']! as String)))
        ..add(_field(utf8.encode(hello['hostId']! as String)))
        ..add(_field(utf8.encode(hello['challenge']! as String)))
        ..add(_field(_unhex(hello['deviceIdentityPublicKeyHex']! as String)))
        ..add(_field(_unhex(hello['ephemeralPublicKeyHex']! as String)))
        ..add(_field(_unhex(hello['nonceHex']! as String))))
      .takeBytes();
}

Uint8List _responderClaim(Map<String, Object?> hello) {
  return (BytesBuilder(copy: false)
        ..add(_field(utf8.encode('muxport-responder-hello-v1')))
        ..add(_u32(hello['protocolVersion']! as int))
        ..add(_field(utf8.encode(hello['hostId']! as String)))
        ..add(_field(utf8.encode(hello['deviceId']! as String)))
        ..add(_field(_unhex(hello['hostIdentityPublicKeyHex']! as String)))
        ..add(_field(_unhex(hello['ephemeralPublicKeyHex']! as String)))
        ..add(_field(_unhex(hello['nonceHex']! as String)))
        ..add(_field(_unhex(hello['initiatorHashHex']! as String))))
      .takeBytes();
}

Uint8List _initiatorHash(Map<String, Object?> initiator) {
  return Uint8List.fromList(
    hashes.sha256.convert([
      ..._initiatorClaim(initiator),
      ..._unhex(initiator['signatureHex']! as String),
    ]).bytes,
  );
}

Uint8List _transcript(
  Map<String, Object?> initiator,
  Map<String, Object?> responder,
) {
  return (BytesBuilder(copy: false)
        ..add(_field(utf8.encode('muxport-authenticated-transcript-v1')))
        ..add(_field(_initiatorClaim(initiator)))
        ..add(_field(_unhex(initiator['signatureHex']! as String)))
        ..add(_field(_responderClaim(responder)))
        ..add(_field(_unhex(responder['signatureHex']! as String))))
      .takeBytes();
}

Uint8List _sessionAad(String hostId, String deviceId, List<int> transcript) {
  final host = utf8.encode(hostId);
  final device = utf8.encode(deviceId);
  final transcriptHash = hashes.sha256.convert(transcript).bytes;
  return (BytesBuilder(copy: false)
        ..add(utf8.encode('muxport-secure-envelope-v1'))
        ..add(_u64(host.length))
        ..add(host)
        ..add(_u64(device.length))
        ..add(device)
        ..add(_u64(transcriptHash.length))
        ..add(transcriptHash))
      .takeBytes();
}

Future<void> _writeRecord(Socket socket, List<int> bytes) async {
  socket
    ..add(_u32(bytes.length))
    ..add(bytes);
  await socket.flush();
}

class _TestRecordReader {
  _TestRecordReader(Socket socket)
    : _iterator = StreamIterator<Uint8List>(socket);

  final StreamIterator<Uint8List> _iterator;
  final List<int> _buffer = [];

  Future<Uint8List> readRecord() async {
    final header = await _readExact(4);
    final length = ByteData.sublistView(header).getUint32(0, Endian.big);
    if (length <= 0 || length > directTransportMaximumPlaintextBytes + 64) {
      throw StateError('bad test record length');
    }
    return _readExact(length);
  }

  Future<Uint8List> _readExact(int length) async {
    while (_buffer.length < length) {
      if (!await _iterator.moveNext()) {
        throw StateError('test peer closed early');
      }
      _buffer.addAll(_iterator.current);
    }
    final output = Uint8List.fromList(_buffer.take(length).toList());
    _buffer.removeRange(0, length);
    return output;
  }

  Future<void> cancel() => _iterator.cancel();
}

class _MemorySecureStore implements MobileSecureValueStore {
  String? value;

  @override
  Future<String?> read(String key) async => value;

  @override
  Future<void> write(String key, String value) async {
    this.value = value;
  }
}

Uint8List _field(List<int> bytes) {
  return Uint8List.fromList([..._u32(bytes.length), ...bytes]);
}

Uint8List _u32(int value) {
  return (ByteData(4)..setUint32(0, value, Endian.big)).buffer.asUint8List();
}

Uint8List _u64(int value) {
  return (ByteData(8)..setUint64(0, value, Endian.big)).buffer.asUint8List();
}

Uint8List _i64(int value) {
  return (ByteData(8)..setInt64(0, value, Endian.big)).buffer.asUint8List();
}

String _hex(List<int> bytes) {
  return bytes.map((byte) => byte.toRadixString(16).padLeft(2, '0')).join();
}

Uint8List _unhex(String value) {
  final bytes = Uint8List(value.length ~/ 2);
  for (var index = 0; index < bytes.length; index += 1) {
    bytes[index] = int.parse(
      value.substring(index * 2, index * 2 + 2),
      radix: 16,
    );
  }
  return bytes;
}
