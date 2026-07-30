import 'dart:convert';
import 'dart:math';
import 'dart:typed_data';

import 'package:crypto/crypto.dart' as hashes;
import 'package:cryptography/cryptography.dart';

import 'transport_identity.dart';

const int directTransportProtocolVersion = 1;
const int directTransportChallengeLifetimeMs = 15000;
const int directTransportClockSkewMs = 60000;
const int directTransportMaximumPlaintextBytes = 1024 * 1024;

class DirectTransportProtocolException implements Exception {
  const DirectTransportProtocolException(this.message, [this.cause]);

  final String message;
  final Object? cause;

  @override
  String toString() {
    final detail = cause == null ? '' : ': $cause';
    return 'DirectTransportProtocolException: $message$detail';
  }
}

class PinnedHostIdentity {
  PinnedHostIdentity({required this.hostId, required this.publicKeyHex})
    : publicKeyBytes = _decodeHex(publicKeyHex, expectedBytes: 32) {
    if (hostId.trim().isEmpty) {
      throw const DirectTransportProtocolException(
        'pinned host id must not be empty',
      );
    }
    if (publicKeyHex != _encodeHex(publicKeyBytes)) {
      throw const DirectTransportProtocolException(
        'pinned host key must use canonical lowercase hexadecimal',
      );
    }
  }

  final String hostId;
  final String publicKeyHex;
  final Uint8List publicKeyBytes;
}

class VerifiedServerChallenge {
  const VerifiedServerChallenge({
    required this.hostId,
    required this.hostIdentityPublicKeyHex,
    required this.bootEpoch,
    required this.challenge,
    required this.issuedAtMs,
    required this.expiresAtMs,
  });

  final String hostId;
  final String hostIdentityPublicKeyHex;
  final int bootEpoch;
  final String challenge;
  final int issuedAtMs;
  final int expiresAtMs;
}

class InitiatorHello {
  const InitiatorHello({
    required this.protocolVersion,
    required this.deviceId,
    required this.hostId,
    required this.challenge,
    required this.deviceIdentityPublicKeyHex,
    required this.ephemeralPublicKeyHex,
    required this.nonceHex,
    required this.signatureHex,
  });

  final int protocolVersion;
  final String deviceId;
  final String hostId;
  final String challenge;
  final String deviceIdentityPublicKeyHex;
  final String ephemeralPublicKeyHex;
  final String nonceHex;
  final String signatureHex;

  Map<String, Object?> toJson() => {
    'protocolVersion': protocolVersion,
    'deviceId': deviceId,
    'hostId': hostId,
    'challenge': challenge,
    'deviceIdentityPublicKeyHex': deviceIdentityPublicKeyHex,
    'ephemeralPublicKeyHex': ephemeralPublicKeyHex,
    'nonceHex': nonceHex,
    'signatureHex': signatureHex,
  };

  InitiatorHello copyWith({String? signatureHex}) {
    return InitiatorHello(
      protocolVersion: protocolVersion,
      deviceId: deviceId,
      hostId: hostId,
      challenge: challenge,
      deviceIdentityPublicKeyHex: deviceIdentityPublicKeyHex,
      ephemeralPublicKeyHex: ephemeralPublicKeyHex,
      nonceHex: nonceHex,
      signatureHex: signatureHex ?? this.signatureHex,
    );
  }
}

class ResponderHello {
  const ResponderHello({
    required this.protocolVersion,
    required this.hostId,
    required this.deviceId,
    required this.hostIdentityPublicKeyHex,
    required this.ephemeralPublicKeyHex,
    required this.nonceHex,
    required this.initiatorHashHex,
    required this.signatureHex,
  });

  factory ResponderHello.fromJson(Object? value) {
    final json = _strictJsonObject(value, const {
      'protocolVersion',
      'hostId',
      'deviceId',
      'hostIdentityPublicKeyHex',
      'ephemeralPublicKeyHex',
      'nonceHex',
      'initiatorHashHex',
      'signatureHex',
    }, 'responder hello');
    return ResponderHello(
      protocolVersion: _jsonInt(json, 'protocolVersion'),
      hostId: _jsonString(json, 'hostId'),
      deviceId: _jsonString(json, 'deviceId'),
      hostIdentityPublicKeyHex: _jsonString(json, 'hostIdentityPublicKeyHex'),
      ephemeralPublicKeyHex: _jsonString(json, 'ephemeralPublicKeyHex'),
      nonceHex: _jsonString(json, 'nonceHex'),
      initiatorHashHex: _jsonString(json, 'initiatorHashHex'),
      signatureHex: _jsonString(json, 'signatureHex'),
    );
  }

  final int protocolVersion;
  final String hostId;
  final String deviceId;
  final String hostIdentityPublicKeyHex;
  final String ephemeralPublicKeyHex;
  final String nonceHex;
  final String initiatorHashHex;
  final String signatureHex;
}

class DirectSessionKeys {
  DirectSessionKeys({
    required List<int> sendKey,
    required List<int> receiveKey,
    required List<int> sendNoncePrefix,
    required List<int> receiveNoncePrefix,
    required List<int> aad,
  }) : sendKey = Uint8List.fromList(sendKey),
       receiveKey = Uint8List.fromList(receiveKey),
       sendNoncePrefix = Uint8List.fromList(sendNoncePrefix),
       receiveNoncePrefix = Uint8List.fromList(receiveNoncePrefix),
       aad = Uint8List.fromList(aad) {
    if (sendKey.length != 32 ||
        receiveKey.length != 32 ||
        sendNoncePrefix.length != 4 ||
        receiveNoncePrefix.length != 4 ||
        aad.isEmpty) {
      throw const DirectTransportProtocolException(
        'directional session key material is invalid',
      );
    }
  }

  final Uint8List sendKey;
  final Uint8List receiveKey;
  final Uint8List sendNoncePrefix;
  final Uint8List receiveNoncePrefix;
  final Uint8List aad;

  void destroy() {
    sendKey.fillRange(0, sendKey.length, 0);
    receiveKey.fillRange(0, receiveKey.length, 0);
    sendNoncePrefix.fillRange(0, sendNoncePrefix.length, 0);
    receiveNoncePrefix.fillRange(0, receiveNoncePrefix.length, 0);
    aad.fillRange(0, aad.length, 0);
  }
}

Future<DirectSessionKeys> deriveInitiatorDirectSessionKeys({
  required SecretKey sharedSecret,
  required List<int> transcript,
  required String hostId,
  required String deviceId,
}) async {
  if (transcript.isEmpty || hostId.trim().isEmpty || deviceId.trim().isEmpty) {
    throw const DirectTransportProtocolException(
      'session key derivation context is invalid',
    );
  }
  final transcriptHash = hashes.sha256.convert(transcript).bytes;
  final directional = await Hkdf(hmac: Hmac.sha256(), outputLength: 72)
      .deriveKey(
        secretKey: sharedSecret,
        nonce: transcriptHash,
        info: utf8.encode('muxport-directional-session-v1'),
      );
  late final Uint8List material;
  try {
    material = Uint8List.fromList(await directional.extractBytes());
  } finally {
    directional.destroy();
  }
  try {
    return DirectSessionKeys(
      sendKey: material.sublist(0, 32),
      receiveKey: material.sublist(32, 64),
      sendNoncePrefix: material.sublist(64, 68),
      receiveNoncePrefix: material.sublist(68, 72),
      aad: _sessionAad(hostId, deviceId, transcript),
    );
  } finally {
    material.fillRange(0, material.length, 0);
  }
}

class DirectEncryptedFrame {
  DirectEncryptedFrame({required this.sequence, required List<int> ciphertext})
    : ciphertext = Uint8List.fromList(ciphertext) {
    if (sequence <= 0) {
      throw const DirectTransportProtocolException(
        'encrypted frame sequence must be positive',
      );
    }
  }

  factory DirectEncryptedFrame.decode(
    List<int> encoded, {
    int maximumCiphertextBytes = directTransportMaximumPlaintextBytes + 16,
  }) {
    if (encoded.length < 16 ||
        encoded[0] != 0x4d ||
        encoded[1] != 0x55 ||
        encoded[2] != 0x58 ||
        encoded[3] != 0x31) {
      throw const DirectTransportProtocolException(
        'encrypted frame header is invalid',
      );
    }
    final sequence = _readUint64(encoded, 4);
    final ciphertextLength = _readUint32(encoded, 12);
    if (sequence <= 0 ||
        ciphertextLength > maximumCiphertextBytes ||
        encoded.length != 16 + ciphertextLength) {
      throw const DirectTransportProtocolException(
        'encrypted frame length or sequence is invalid',
      );
    }
    return DirectEncryptedFrame(
      sequence: sequence,
      ciphertext: encoded.sublist(16),
    );
  }

  final int sequence;
  final Uint8List ciphertext;

  Uint8List encode() {
    final output = BytesBuilder(copy: false)
      ..add(const [0x4d, 0x55, 0x58, 0x31])
      ..add(_uint64(sequence))
      ..add(_uint32(ciphertext.length))
      ..add(ciphertext);
    return output.takeBytes();
  }
}

class DirectSessionCipher {
  DirectSessionCipher(DirectSessionKeys keys)
    : _sendKey = SecretKeyData(keys.sendKey, overwriteWhenDestroyed: true),
      _receiveKey = SecretKeyData(
        keys.receiveKey,
        overwriteWhenDestroyed: true,
      ),
      _sendNoncePrefix = Uint8List.fromList(keys.sendNoncePrefix),
      _receiveNoncePrefix = Uint8List.fromList(keys.receiveNoncePrefix),
      _aad = Uint8List.fromList(keys.aad);

  final SecretKeyData _sendKey;
  final SecretKeyData _receiveKey;
  final Uint8List _sendNoncePrefix;
  final Uint8List _receiveNoncePrefix;
  final Uint8List _aad;
  final Cipher _cipher = Chacha20.poly1305Aead();
  int _sendSequence = 0;
  int _receiveSequence = 0;
  bool _destroyed = false;

  int get nextSendSequence => _sendSequence + 1;

  Future<DirectEncryptedFrame> encrypt(List<int> plaintext) async {
    _ensureUsable();
    if (plaintext.length > directTransportMaximumPlaintextBytes) {
      throw const DirectTransportProtocolException(
        'secure envelope plaintext is too large',
      );
    }
    final sequence = _sendSequence + 1;
    final nonce = _frameNonce(_sendNoncePrefix, sequence);
    final box = await _cipher.encrypt(
      plaintext,
      secretKey: _sendKey,
      nonce: nonce,
      aad: _frameAad(sequence, _aad),
    );
    _sendSequence = sequence;
    return DirectEncryptedFrame(
      sequence: sequence,
      ciphertext: [...box.cipherText, ...box.mac.bytes],
    );
  }

  Future<Uint8List> decrypt(DirectEncryptedFrame frame) async {
    _ensureUsable();
    final expected = _receiveSequence + 1;
    if (frame.sequence != expected || frame.ciphertext.length < 16) {
      throw const DirectTransportProtocolException(
        'encrypted frame was replayed or arrived out of order',
      );
    }
    final macOffset = frame.ciphertext.length - 16;
    try {
      final plaintext = await _cipher.decrypt(
        SecretBox(
          frame.ciphertext.sublist(0, macOffset),
          nonce: _frameNonce(_receiveNoncePrefix, frame.sequence),
          mac: Mac(frame.ciphertext.sublist(macOffset)),
        ),
        secretKey: _receiveKey,
        aad: _frameAad(frame.sequence, _aad),
      );
      if (plaintext.length > directTransportMaximumPlaintextBytes) {
        throw const DirectTransportProtocolException(
          'decrypted secure envelope is too large',
        );
      }
      _receiveSequence = frame.sequence;
      return Uint8List.fromList(plaintext);
    } on DirectTransportProtocolException {
      rethrow;
    } on Object catch (error) {
      throw DirectTransportProtocolException(
        'encrypted frame authentication failed',
        error,
      );
    }
  }

  void destroy() {
    if (_destroyed) {
      return;
    }
    _destroyed = true;
    _sendKey.destroy();
    _receiveKey.destroy();
    _sendNoncePrefix.fillRange(0, _sendNoncePrefix.length, 0);
    _receiveNoncePrefix.fillRange(0, _receiveNoncePrefix.length, 0);
    _aad.fillRange(0, _aad.length, 0);
  }

  void _ensureUsable() {
    if (_destroyed) {
      throw StateError('direct session cipher has been destroyed');
    }
  }
}

class DirectHandshakeInitiator {
  DirectHandshakeInitiator({
    required this.identity,
    X25519? x25519,
    Ed25519? ed25519,
    Random? random,
  }) : _x25519 = x25519 ?? X25519(),
       _ed25519 = ed25519 ?? Ed25519(),
       _random = random ?? Random.secure();

  final DirectTransportIdentity identity;
  final X25519 _x25519;
  final Ed25519 _ed25519;
  final Random _random;

  Future<VerifiedServerChallenge> verifyServerChallenge({
    required Object? jsonValue,
    required PinnedHostIdentity pinnedHost,
    required int nowMs,
  }) async {
    final json = _strictJsonObject(jsonValue, const {
      'protocolVersion',
      'hostId',
      'hostIdentityPublicKeyHex',
      'bootEpoch',
      'challenge',
      'issuedAtMs',
      'expiresAtMs',
      'signatureHex',
    }, 'server challenge');
    final protocolVersion = _jsonInt(json, 'protocolVersion');
    final hostId = _jsonString(json, 'hostId');
    final publicKeyHex = _jsonString(json, 'hostIdentityPublicKeyHex');
    final bootEpoch = _jsonInt(json, 'bootEpoch');
    final challengeHex = _jsonString(json, 'challenge');
    final issuedAtMs = _jsonInt(json, 'issuedAtMs');
    final expiresAtMs = _jsonInt(json, 'expiresAtMs');
    final signature = _decodeHex(
      _jsonString(json, 'signatureHex'),
      expectedBytes: 64,
    );
    final challenge = _decodeHex(challengeHex, expectedBytes: 32);
    final validContext =
        protocolVersion == directTransportProtocolVersion &&
        hostId == pinnedHost.hostId &&
        publicKeyHex == pinnedHost.publicKeyHex &&
        bootEpoch > 0 &&
        challenge.any((byte) => byte != 0) &&
        expiresAtMs - issuedAtMs == directTransportChallengeLifetimeMs &&
        nowMs >= issuedAtMs - directTransportClockSkewMs &&
        nowMs <= expiresAtMs + directTransportClockSkewMs;
    if (!validContext) {
      throw const DirectTransportProtocolException(
        'server challenge does not match the pinned host or validity window',
      );
    }
    final signed = BytesBuilder(copy: false)
      ..add(utf8.encode('muxport-server-challenge-v1'))
      ..add(_lengthPrefixed(_uint32(protocolVersion)))
      ..add(_lengthPrefixed(utf8.encode(hostId)))
      ..add(_lengthPrefixed(utf8.encode(publicKeyHex)))
      ..add(_lengthPrefixed(_uint64(bootEpoch)))
      ..add(_lengthPrefixed(utf8.encode(challengeHex)))
      ..add(_lengthPrefixed(_int64(issuedAtMs)))
      ..add(_lengthPrefixed(_int64(expiresAtMs)));
    final verified = await _ed25519.verify(
      signed.takeBytes(),
      signature: Signature(
        signature,
        publicKey: SimplePublicKey(
          pinnedHost.publicKeyBytes,
          type: KeyPairType.ed25519,
        ),
      ),
    );
    if (!verified) {
      throw const DirectTransportProtocolException(
        'server challenge signature is invalid',
      );
    }
    return VerifiedServerChallenge(
      hostId: hostId,
      hostIdentityPublicKeyHex: publicKeyHex,
      bootEpoch: bootEpoch,
      challenge: challengeHex,
      issuedAtMs: issuedAtMs,
      expiresAtMs: expiresAtMs,
    );
  }

  Future<PendingDirectHandshake> createInitiatorHello({
    required VerifiedServerChallenge challenge,
  }) async {
    final ephemeral = await _x25519.newKeyPair();
    try {
      final ephemeralPublic = await ephemeral.extractPublicKey();
      final nonce = _randomBytes(32, _random);
      final unsigned = InitiatorHello(
        protocolVersion: directTransportProtocolVersion,
        deviceId: identity.deviceId,
        hostId: challenge.hostId,
        challenge: challenge.challenge,
        deviceIdentityPublicKeyHex: _encodeHex(identity.publicKeyBytes),
        ephemeralPublicKeyHex: _encodeHex(ephemeralPublic.bytes),
        nonceHex: _encodeHex(nonce),
        signatureHex: '',
      );
      final signature = await identity.sign(_initiatorClaim(unsigned));
      final hello = unsigned.copyWith(signatureHex: _encodeHex(signature));
      return PendingDirectHandshake._(
        challenge: challenge,
        initiator: hello,
        ephemeral: ephemeral,
        x25519: _x25519,
        ed25519: _ed25519,
      );
    } on Object {
      ephemeral.destroy();
      rethrow;
    }
  }
}

class PendingDirectHandshake {
  PendingDirectHandshake._({
    required this.challenge,
    required this.initiator,
    required SimpleKeyPair ephemeral,
    required X25519 x25519,
    required Ed25519 ed25519,
  }) : _ephemeral = ephemeral,
       _x25519 = x25519,
       _ed25519 = ed25519;

  final VerifiedServerChallenge challenge;
  final InitiatorHello initiator;
  final SimpleKeyPair _ephemeral;
  final X25519 _x25519;
  final Ed25519 _ed25519;
  bool _consumed = false;

  Future<DirectSessionKeys> finish(Object? responderJson) async {
    if (_consumed) {
      throw StateError('direct handshake has already been consumed');
    }
    _consumed = true;
    try {
      final responder = ResponderHello.fromJson(responderJson);
      final initiatorHash = _initiatorHash(initiator);
      final expectedInitiatorHash = _encodeHex(initiatorHash);
      final validContext =
          responder.protocolVersion == directTransportProtocolVersion &&
          responder.hostId == challenge.hostId &&
          responder.deviceId == initiator.deviceId &&
          responder.hostIdentityPublicKeyHex ==
              challenge.hostIdentityPublicKeyHex &&
          responder.initiatorHashHex == expectedInitiatorHash;
      if (!validContext) {
        throw const DirectTransportProtocolException(
          'responder hello does not match the authenticated initiator',
        );
      }
      _decodeHex(responder.nonceHex, expectedBytes: 32);
      final responderEphemeral = _decodeHex(
        responder.ephemeralPublicKeyHex,
        expectedBytes: 32,
      );
      final signature = _decodeHex(responder.signatureHex, expectedBytes: 64);
      final hostIdentity = _decodeHex(
        responder.hostIdentityPublicKeyHex,
        expectedBytes: 32,
      );
      final signatureValid = await _ed25519.verify(
        _responderClaim(responder),
        signature: Signature(
          signature,
          publicKey: SimplePublicKey(hostIdentity, type: KeyPairType.ed25519),
        ),
      );
      if (!signatureValid) {
        throw const DirectTransportProtocolException(
          'responder hello signature is invalid',
        );
      }

      final transcript = _authenticatedTranscript(initiator, responder);
      final dhKey = await _x25519.sharedSecretKey(
        keyPair: _ephemeral,
        remotePublicKey: SimplePublicKey(
          responderEphemeral,
          type: KeyPairType.x25519,
        ),
      );
      late final Uint8List dhBytes;
      try {
        dhBytes = Uint8List.fromList(await dhKey.extractBytes());
      } finally {
        dhKey.destroy();
      }
      if (dhBytes.every((byte) => byte == 0)) {
        dhBytes.fillRange(0, dhBytes.length, 0);
        throw const DirectTransportProtocolException(
          'host supplied a non-contributory X25519 key',
        );
      }
      try {
        final transcriptHash = hashes.sha256.convert(transcript).bytes;
        final dhInput = SecretKeyData(dhBytes, overwriteWhenDestroyed: true);
        late final SecretKeyData sharedKey;
        try {
          sharedKey = await Hkdf(hmac: Hmac.sha256(), outputLength: 32)
              .deriveKey(
                secretKey: dhInput,
                nonce: transcriptHash,
                info: utf8.encode('muxport-shared-secret-v1'),
              );
        } finally {
          dhInput.destroy();
        }
        try {
          return await deriveInitiatorDirectSessionKeys(
            sharedSecret: sharedKey,
            transcript: transcript,
            hostId: challenge.hostId,
            deviceId: initiator.deviceId,
          );
        } finally {
          sharedKey.destroy();
        }
      } finally {
        dhBytes.fillRange(0, dhBytes.length, 0);
      }
    } on DirectTransportProtocolException {
      rethrow;
    } on Object catch (error) {
      throw DirectTransportProtocolException(
        'could not finish the authenticated handshake',
        error,
      );
    } finally {
      _ephemeral.destroy();
    }
  }

  void abort() {
    if (!_consumed) {
      _consumed = true;
      _ephemeral.destroy();
    }
  }
}

Uint8List _initiatorClaim(InitiatorHello hello) {
  return (BytesBuilder(copy: false)
        ..add(_lengthPrefixed(utf8.encode('muxport-initiator-hello-v1')))
        ..add(_uint32(hello.protocolVersion))
        ..add(_lengthPrefixed(utf8.encode(hello.deviceId)))
        ..add(_lengthPrefixed(utf8.encode(hello.hostId)))
        ..add(_lengthPrefixed(utf8.encode(hello.challenge)))
        ..add(
          _lengthPrefixed(
            _decodeHex(hello.deviceIdentityPublicKeyHex, expectedBytes: 32),
          ),
        )
        ..add(
          _lengthPrefixed(
            _decodeHex(hello.ephemeralPublicKeyHex, expectedBytes: 32),
          ),
        )
        ..add(_lengthPrefixed(_decodeHex(hello.nonceHex, expectedBytes: 32))))
      .takeBytes();
}

Uint8List _responderClaim(ResponderHello hello) {
  return (BytesBuilder(copy: false)
        ..add(_lengthPrefixed(utf8.encode('muxport-responder-hello-v1')))
        ..add(_uint32(hello.protocolVersion))
        ..add(_lengthPrefixed(utf8.encode(hello.hostId)))
        ..add(_lengthPrefixed(utf8.encode(hello.deviceId)))
        ..add(
          _lengthPrefixed(
            _decodeHex(hello.hostIdentityPublicKeyHex, expectedBytes: 32),
          ),
        )
        ..add(
          _lengthPrefixed(
            _decodeHex(hello.ephemeralPublicKeyHex, expectedBytes: 32),
          ),
        )
        ..add(_lengthPrefixed(_decodeHex(hello.nonceHex, expectedBytes: 32)))
        ..add(
          _lengthPrefixed(
            _decodeHex(hello.initiatorHashHex, expectedBytes: 32),
          ),
        ))
      .takeBytes();
}

Uint8List _initiatorHash(InitiatorHello hello) {
  return Uint8List.fromList(
    hashes.sha256.convert([
      ..._initiatorClaim(hello),
      ..._decodeHex(hello.signatureHex, expectedBytes: 64),
    ]).bytes,
  );
}

Uint8List _authenticatedTranscript(
  InitiatorHello initiator,
  ResponderHello responder,
) {
  if (responder.initiatorHashHex != _encodeHex(_initiatorHash(initiator))) {
    throw const DirectTransportProtocolException(
      'responder transcript binding is invalid',
    );
  }
  return (BytesBuilder(copy: false)
        ..add(
          _lengthPrefixed(utf8.encode('muxport-authenticated-transcript-v1')),
        )
        ..add(_lengthPrefixed(_initiatorClaim(initiator)))
        ..add(
          _lengthPrefixed(
            _decodeHex(initiator.signatureHex, expectedBytes: 64),
          ),
        )
        ..add(_lengthPrefixed(_responderClaim(responder)))
        ..add(
          _lengthPrefixed(
            _decodeHex(responder.signatureHex, expectedBytes: 64),
          ),
        ))
      .takeBytes();
}

Uint8List _sessionAad(String hostId, String deviceId, List<int> transcript) {
  final transcriptHash = hashes.sha256.convert(transcript).bytes;
  final hostBytes = utf8.encode(hostId);
  final deviceBytes = utf8.encode(deviceId);
  return (BytesBuilder(copy: false)
        ..add(utf8.encode('muxport-secure-envelope-v1'))
        ..add(_uint64(hostBytes.length))
        ..add(hostBytes)
        ..add(_uint64(deviceBytes.length))
        ..add(deviceBytes)
        ..add(_uint64(transcriptHash.length))
        ..add(transcriptHash))
      .takeBytes();
}

Uint8List _frameNonce(List<int> prefix, int sequence) {
  if (prefix.length != 4 || sequence <= 0) {
    throw const DirectTransportProtocolException('invalid frame nonce context');
  }
  return Uint8List.fromList([...prefix, ..._uint64(sequence)]);
}

Uint8List _frameAad(int sequence, List<int> aad) {
  return Uint8List.fromList([..._uint64(sequence), ...aad]);
}

Uint8List _lengthPrefixed(List<int> value) {
  return Uint8List.fromList([..._uint32(value.length), ...value]);
}

Uint8List _randomBytes(int length, Random random) {
  final bytes = Uint8List(length);
  for (var index = 0; index < bytes.length; index += 1) {
    bytes[index] = random.nextInt(256);
  }
  return bytes;
}

Uint8List _uint32(int value) {
  if (value < 0 || value > 0xffffffff) {
    throw const DirectTransportProtocolException(
      'value does not fit in uint32',
    );
  }
  return (ByteData(4)..setUint32(0, value, Endian.big)).buffer.asUint8List();
}

Uint8List _uint64(int value) {
  if (value < 0) {
    throw const DirectTransportProtocolException(
      'value does not fit in uint64',
    );
  }
  return (ByteData(8)..setUint64(0, value, Endian.big)).buffer.asUint8List();
}

Uint8List _int64(int value) {
  return (ByteData(8)..setInt64(0, value, Endian.big)).buffer.asUint8List();
}

int _readUint32(List<int> bytes, int offset) {
  return ByteData.sublistView(
    Uint8List.fromList(bytes),
    offset,
    offset + 4,
  ).getUint32(0, Endian.big);
}

int _readUint64(List<int> bytes, int offset) {
  return ByteData.sublistView(
    Uint8List.fromList(bytes),
    offset,
    offset + 8,
  ).getUint64(0, Endian.big);
}

String _encodeHex(List<int> bytes) {
  const alphabet = '0123456789abcdef';
  final output = StringBuffer();
  for (final byte in bytes) {
    output
      ..write(alphabet[byte >> 4])
      ..write(alphabet[byte & 0x0f]);
  }
  return output.toString();
}

Uint8List _decodeHex(String value, {required int expectedBytes}) {
  if (value.length != expectedBytes * 2) {
    throw const DirectTransportProtocolException(
      'hexadecimal field has the wrong length',
    );
  }
  final output = Uint8List(expectedBytes);
  for (var index = 0; index < expectedBytes; index += 1) {
    final byte = int.tryParse(
      value.substring(index * 2, index * 2 + 2),
      radix: 16,
    );
    if (byte == null) {
      throw const DirectTransportProtocolException(
        'hexadecimal field contains invalid data',
      );
    }
    output[index] = byte;
  }
  return output;
}

Map<String, Object?> _strictJsonObject(
  Object? value,
  Set<String> expectedKeys,
  String name,
) {
  if (value is! Map) {
    throw DirectTransportProtocolException('$name must be a JSON object');
  }
  final json = Map<String, Object?>.from(value);
  if (json.keys.toSet().difference(expectedKeys).isNotEmpty ||
      expectedKeys.difference(json.keys.toSet()).isNotEmpty) {
    throw DirectTransportProtocolException(
      '$name contains missing or unexpected fields',
    );
  }
  return json;
}

String _jsonString(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! String) {
    throw DirectTransportProtocolException('$key must be a string');
  }
  return value;
}

int _jsonInt(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! int) {
    throw DirectTransportProtocolException('$key must be an integer');
  }
  return value;
}
