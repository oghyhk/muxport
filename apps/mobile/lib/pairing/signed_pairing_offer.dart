import 'dart:convert';
import 'dart:typed_data';

import 'package:cryptography/cryptography.dart';

import '../transport/direct_transport_protocol.dart';

const int _maximumPairingLifetimeMs = 10 * 60 * 1000;

class SignedPairingOffer {
  const SignedPairingOffer._({
    required this.protocolVersion,
    required this.hostId,
    required this.hostname,
    required this.hostIdentityPublicKeyHex,
    required this.directEndpoint,
    required this.rendezvousToken,
    required this.ephemeralPublicKeyHex,
    required this.issuedAtMs,
    required this.expiresAtMs,
    required this.signatureHex,
  });

  static Future<SignedPairingOffer> parseAndVerify(
    String encoded, {
    required int nowMs,
    Ed25519? ed25519,
  }) async {
    Object? decoded;
    try {
      decoded = jsonDecode(encoded);
    } on Object catch (error) {
      throw DirectTransportProtocolException(
        'pairing offer is not valid JSON',
        error,
      );
    }
    if (decoded is! Map) {
      throw const DirectTransportProtocolException(
        'pairing offer must be a JSON object',
      );
    }
    final json = Map<String, Object?>.from(decoded);
    const expectedKeys = {
      'protocolVersion',
      'hostId',
      'hostname',
      'hostIdentityPublicKeyHex',
      'directEndpoint',
      'rendezvousToken',
      'ephemeralPublicKeyHex',
      'issuedAtMs',
      'expiresAtMs',
      'signatureHex',
    };
    if (json.keys.toSet().difference(expectedKeys).isNotEmpty ||
        expectedKeys.difference(json.keys.toSet()).isNotEmpty) {
      throw const DirectTransportProtocolException(
        'pairing offer contains missing or unexpected fields',
      );
    }
    final offer = SignedPairingOffer._(
      protocolVersion: _integer(json, 'protocolVersion'),
      hostId: _string(json, 'hostId'),
      hostname: _string(json, 'hostname'),
      hostIdentityPublicKeyHex: _string(json, 'hostIdentityPublicKeyHex'),
      directEndpoint: _string(json, 'directEndpoint'),
      rendezvousToken: _string(json, 'rendezvousToken'),
      ephemeralPublicKeyHex: _string(json, 'ephemeralPublicKeyHex'),
      issuedAtMs: _integer(json, 'issuedAtMs'),
      expiresAtMs: _integer(json, 'expiresAtMs'),
      signatureHex: _string(json, 'signatureHex'),
    );
    await offer._verify(nowMs, ed25519 ?? Ed25519());
    return offer;
  }

  final int protocolVersion;
  final String hostId;
  final String hostname;
  final String hostIdentityPublicKeyHex;
  final String directEndpoint;
  final String rendezvousToken;
  final String ephemeralPublicKeyHex;
  final int issuedAtMs;
  final int expiresAtMs;
  final String signatureHex;

  PinnedHostIdentity get candidateHostPin {
    return PinnedHostIdentity(
      hostId: hostId,
      publicKeyHex: hostIdentityPublicKeyHex,
    );
  }

  String get directAddress => _endpointUri.host;

  int get directPort => _endpointUri.port;

  Uri get _endpointUri => Uri.parse('tcp://$directEndpoint');

  Future<void> _verify(int nowMs, Ed25519 algorithm) async {
    final identity = _hex(hostIdentityPublicKeyHex, expectedBytes: 32);
    _hex(rendezvousToken, expectedBytes: 32);
    _hex(ephemeralPublicKeyHex, expectedBytes: 32);
    final signature = _hex(signatureHex, expectedBytes: 64);
    final lifetime = expiresAtMs - issuedAtMs;
    final endpoint = _endpointUri;
    if (protocolVersion != directTransportProtocolVersion ||
        hostId.trim().isEmpty ||
        hostId.length > 256 ||
        hostname.trim().isEmpty ||
        hostname.length > 256 ||
        directEndpoint.length > 512 ||
        endpoint.scheme != 'tcp' ||
        endpoint.host.isEmpty ||
        !endpoint.hasPort ||
        endpoint.port < 1 ||
        endpoint.port > 65535 ||
        endpoint.userInfo.isNotEmpty ||
        endpoint.path.isNotEmpty ||
        endpoint.hasQuery ||
        endpoint.hasFragment ||
        lifetime <= 0 ||
        lifetime > _maximumPairingLifetimeMs ||
        nowMs < issuedAtMs - directTransportClockSkewMs) {
      throw const DirectTransportProtocolException(
        'pairing offer fields or validity window are invalid',
      );
    }
    if (nowMs > expiresAtMs + directTransportClockSkewMs) {
      throw const DirectTransportProtocolException('pairing offer has expired');
    }
    final verified = await algorithm.verify(
      _signedBytes(),
      signature: Signature(
        signature,
        publicKey: SimplePublicKey(identity, type: KeyPairType.ed25519),
      ),
    );
    if (!verified) {
      throw const DirectTransportProtocolException(
        'pairing offer signature is invalid',
      );
    }
  }

  Uint8List _signedBytes() {
    return (BytesBuilder(copy: false)
          ..add(_field(utf8.encode('muxport-pairing-offer-v1')))
          ..add(_uint32(protocolVersion))
          ..add(_field(utf8.encode(hostId)))
          ..add(_field(utf8.encode(hostname)))
          ..add(_field(_hex(hostIdentityPublicKeyHex, expectedBytes: 32)))
          ..add(_field(utf8.encode(directEndpoint)))
          ..add(_field(_hex(rendezvousToken, expectedBytes: 32)))
          ..add(_field(_hex(ephemeralPublicKeyHex, expectedBytes: 32)))
          ..add(_int64(issuedAtMs))
          ..add(_int64(expiresAtMs)))
        .takeBytes();
  }
}

String _string(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! String) {
    throw DirectTransportProtocolException('$key must be a string');
  }
  return value;
}

int _integer(Map<String, Object?> json, String key) {
  final value = json[key];
  if (value is! int) {
    throw DirectTransportProtocolException('$key must be an integer');
  }
  return value;
}

Uint8List _field(List<int> value) {
  return Uint8List.fromList([..._uint32(value.length), ...value]);
}

Uint8List _uint32(int value) {
  if (value < 0 || value > 0xffffffff) {
    throw const DirectTransportProtocolException(
      'pairing value does not fit in uint32',
    );
  }
  return (ByteData(4)..setUint32(0, value, Endian.big)).buffer.asUint8List();
}

Uint8List _int64(int value) {
  return (ByteData(8)..setInt64(0, value, Endian.big)).buffer.asUint8List();
}

Uint8List _hex(String value, {required int expectedBytes}) {
  if (value.length != expectedBytes * 2 || value != value.toLowerCase()) {
    throw const DirectTransportProtocolException(
      'pairing hexadecimal field is not canonical',
    );
  }
  final output = Uint8List(expectedBytes);
  for (var index = 0; index < expectedBytes; index += 1) {
    final parsed = int.tryParse(
      value.substring(index * 2, index * 2 + 2),
      radix: 16,
    );
    if (parsed == null) {
      throw const DirectTransportProtocolException(
        'pairing hexadecimal field contains invalid data',
      );
    }
    output[index] = parsed;
  }
  return output;
}
