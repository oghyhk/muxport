import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:fixnum/fixnum.dart';
import 'package:flutter_protocol/wire_protocol.dart' as wire;

import '../security/device_identity.dart';
import 'direct_transport_protocol.dart';

const Duration _defaultConnectTimeout = Duration(seconds: 10);
const Duration _handshakeTimeout = Duration(seconds: 15);
const int _maximumHandshakeRecordBytes = 16 * 1024;
const int _maximumEncryptedRecordBytes =
    directTransportMaximumPlaintextBytes + 32;

class AuthenticatedDirectConnection {
  AuthenticatedDirectConnection._({
    required Socket socket,
    required _SocketRecordReader reader,
    required DirectSessionCipher cipher,
    required this.hostId,
    required this.deviceId,
    required this.localBootEpoch,
  }) : _socket = socket,
       _reader = reader,
       _cipher = cipher;

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
      keys.destroy();
      return AuthenticatedDirectConnection._(
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
        'could not establish the direct connector session',
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
