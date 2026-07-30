import 'dart:typed_data';

abstract interface class DirectTransportIdentity {
  String get deviceId;

  List<int> get publicKeyBytes;

  Future<Uint8List> sign(List<int> message);
}
