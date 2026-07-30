import 'dart:convert';
import 'dart:typed_data';

import 'package:cryptography/cryptography.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/pairing/signed_pairing_offer.dart';
import 'package:muxport_mobile/transport/direct_transport_protocol.dart';

void main() {
  test('signed offer verifies and exposes a candidate host pin', () async {
    final hostIdentity = await Ed25519().newKeyPairFromSeed(
      List<int>.generate(32, (index) => index + 1),
    );
    final hostPublic = await hostIdentity.extractPublicKey();
    final now = DateTime.now().millisecondsSinceEpoch;
    final offer = <String, Object?>{
      'protocolVersion': directTransportProtocolVersion,
      'hostId': 'host-1',
      'hostname': 'Developer workstation',
      'hostIdentityPublicKeyHex': _encodeHex(hostPublic.bytes),
      'directEndpoint': '192.0.2.10:45821',
      'rendezvousToken': _encodeHex(List<int>.filled(32, 7)),
      'ephemeralPublicKeyHex': _encodeHex(List<int>.filled(32, 9)),
      'issuedAtMs': now,
      'expiresAtMs': now + 60000,
      'signatureHex': '',
    };
    final signature = await Ed25519().sign(
      _signedBytes(offer),
      keyPair: hostIdentity,
    );
    offer['signatureHex'] = _encodeHex(signature.bytes);

    final verified = await SignedPairingOffer.parseAndVerify(
      jsonEncode(offer),
      nowMs: now,
    );

    expect(verified.hostId, 'host-1');
    expect(verified.directAddress, '192.0.2.10');
    expect(verified.directPort, 45821);
    expect(
      verified.candidateHostPin.publicKeyHex,
      _encodeHex(hostPublic.bytes),
    );
    hostIdentity.destroy();
  });

  test('signed offer rejects endpoint tampering and expiration', () async {
    final hostIdentity = await Ed25519().newKeyPair();
    final hostPublic = await hostIdentity.extractPublicKey();
    const now = 1700000000000;
    final offer = <String, Object?>{
      'protocolVersion': directTransportProtocolVersion,
      'hostId': 'host-1',
      'hostname': 'Developer workstation',
      'hostIdentityPublicKeyHex': _encodeHex(hostPublic.bytes),
      'directEndpoint': '192.0.2.10:45821',
      'rendezvousToken': _encodeHex(List<int>.filled(32, 7)),
      'ephemeralPublicKeyHex': _encodeHex(List<int>.filled(32, 9)),
      'issuedAtMs': now,
      'expiresAtMs': now + 60000,
      'signatureHex': '',
    };
    final signature = await Ed25519().sign(
      _signedBytes(offer),
      keyPair: hostIdentity,
    );
    offer['signatureHex'] = _encodeHex(signature.bytes);

    final tampered = Map<String, Object?>.from(offer)
      ..['directEndpoint'] = 'attacker.example:45821';
    await expectLater(
      SignedPairingOffer.parseAndVerify(jsonEncode(tampered), nowMs: now),
      throwsA(isA<DirectTransportProtocolException>()),
    );
    await expectLater(
      SignedPairingOffer.parseAndVerify(
        jsonEncode(offer),
        nowMs: now + 60000 + directTransportClockSkewMs + 1,
      ),
      throwsA(isA<DirectTransportProtocolException>()),
    );
    hostIdentity.destroy();
  });
}

Uint8List _signedBytes(Map<String, Object?> offer) {
  return (BytesBuilder(copy: false)
        ..add(_field(utf8.encode('muxport-pairing-offer-v1')))
        ..add(_uint32(offer['protocolVersion']! as int))
        ..add(_field(utf8.encode(offer['hostId']! as String)))
        ..add(_field(utf8.encode(offer['hostname']! as String)))
        ..add(_field(_decodeHex(offer['hostIdentityPublicKeyHex']! as String)))
        ..add(_field(utf8.encode(offer['directEndpoint']! as String)))
        ..add(_field(_decodeHex(offer['rendezvousToken']! as String)))
        ..add(_field(_decodeHex(offer['ephemeralPublicKeyHex']! as String)))
        ..add(_int64(offer['issuedAtMs']! as int))
        ..add(_int64(offer['expiresAtMs']! as int)))
      .takeBytes();
}

Uint8List _field(List<int> value) {
  return Uint8List.fromList([..._uint32(value.length), ...value]);
}

Uint8List _uint32(int value) {
  return (ByteData(4)..setUint32(0, value, Endian.big)).buffer.asUint8List();
}

Uint8List _int64(int value) {
  return (ByteData(8)..setInt64(0, value, Endian.big)).buffer.asUint8List();
}

String _encodeHex(List<int> bytes) {
  return bytes.map((byte) => byte.toRadixString(16).padLeft(2, '0')).join();
}

Uint8List _decodeHex(String value) {
  final output = Uint8List(value.length ~/ 2);
  for (var index = 0; index < output.length; index += 1) {
    output[index] = int.parse(
      value.substring(index * 2, index * 2 + 2),
      radix: 16,
    );
  }
  return output;
}
