import 'dart:convert';
import 'dart:io';

import 'package:cryptography/cryptography.dart';
import 'package:fixnum/fixnum.dart';
import 'package:flutter_protocol/wire_protocol.dart' as wire;
import 'package:muxport_mobile/transport/direct_transport_protocol.dart';

Future<void> main() async {
  const hostId = 'host-fixture-1';
  const deviceId = 'device-fixture-1';
  final transcript = utf8.encode('muxport-cross-language-direct-session-v1');
  final sharedSecretBytes = List<int>.generate(32, (index) => index);
  final sharedSecret = SecretKeyData(
    sharedSecretBytes,
    overwriteWhenDestroyed: true,
  );
  final keys = await deriveInitiatorDirectSessionKeys(
    sharedSecret: sharedSecret,
    transcript: transcript,
    hostId: hostId,
    deviceId: deviceId,
  );
  sharedSecret.destroy();

  final envelope = wire.MuxportEnvelope(
    header: wire.EnvelopeHeader(
      protocolVersion: directTransportProtocolVersion,
      senderId: deviceId,
      recipientId: hostId,
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
  final plaintext = envelope.writeToBuffer();
  final cipher = DirectSessionCipher(keys);
  final frame = await cipher.encrypt(plaintext);

  final fixture = <String, Object?>{
    'schemaVersion': 1,
    'hostId': hostId,
    'deviceId': deviceId,
    'sharedSecretHex': _hex(sharedSecretBytes),
    'transcriptHex': _hex(transcript),
    'initiatorToResponderKeyHex': _hex(keys.sendKey),
    'responderToInitiatorKeyHex': _hex(keys.receiveKey),
    'initiatorNoncePrefixHex': _hex(keys.sendNoncePrefix),
    'responderNoncePrefixHex': _hex(keys.receiveNoncePrefix),
    'sessionAadHex': _hex(keys.aad),
    'envelopeHex': _hex(plaintext),
    'encryptedFrameHex': _hex(frame.encode()),
  };
  // This tool only prints. Review the diff before replacing the committed
  // fixture so a dependency update cannot silently rewrite protocol evidence.
  stdout.writeln(const JsonEncoder.withIndent('  ').convert(fixture));

  cipher.destroy();
  keys.destroy();
  sharedSecretBytes.fillRange(0, sharedSecretBytes.length, 0);
}

String _hex(List<int> bytes) {
  return bytes.map((byte) => byte.toRadixString(16).padLeft(2, '0')).join();
}
