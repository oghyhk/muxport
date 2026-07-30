import 'dart:convert';
import 'dart:math';
import 'dart:typed_data';

import 'package:crypto/crypto.dart' as hashes;
import 'package:cryptography/cryptography.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

const String _deviceIdentityStorageKey = 'muxport.device.identity.v1';

abstract interface class MobileSecureValueStore {
  Future<String?> read(String key);

  Future<void> write(String key, String value);
}

class PlatformMobileSecureValueStore implements MobileSecureValueStore {
  PlatformMobileSecureValueStore({FlutterSecureStorage? storage})
    : _storage =
          storage ??
          const FlutterSecureStorage(
            aOptions: AndroidOptions(
              storageNamespace: 'muxport_device_identity_v1',
              resetOnError: false,
              migrateOnAlgorithmChange: true,
              migrateWithBackup: true,
            ),
            iOptions: IOSOptions(
              accountName: _deviceIdentityStorageKey,
              accessibility: KeychainAccessibility.first_unlock_this_device,
              synchronizable: false,
              useSecureEnclave: true,
            ),
          );

  final FlutterSecureStorage _storage;

  @override
  Future<String?> read(String key) => _storage.read(key: key);

  @override
  Future<void> write(String key, String value) {
    return _storage.write(key: key, value: value);
  }
}

class DeviceIdentityUnavailableException implements Exception {
  const DeviceIdentityUnavailableException(this.message, [this.cause]);

  final String message;
  final Object? cause;

  @override
  String toString() {
    final detail = cause == null ? '' : ': $cause';
    return 'DeviceIdentityUnavailableException: $message$detail';
  }
}

class MobileDeviceIdentity {
  MobileDeviceIdentity._({
    required SimpleKeyPair keyPair,
    required this.deviceId,
    required List<int> publicKeyBytes,
    required Ed25519 algorithm,
  }) : _keyPair = keyPair,
       publicKeyBytes = List.unmodifiable(publicKeyBytes),
       _algorithm = algorithm;

  final SimpleKeyPair _keyPair;
  final Ed25519 _algorithm;
  final String deviceId;
  final List<int> publicKeyBytes;
  bool _destroyed = false;

  Future<Uint8List> sign(List<int> message) async {
    if (_destroyed) {
      throw StateError('device identity has been destroyed');
    }
    final signature = await _algorithm.sign(
      Uint8List.fromList(message),
      keyPair: _keyPair,
    );
    return Uint8List.fromList(signature.bytes);
  }

  Future<void> destroy() async {
    if (!_destroyed) {
      _destroyed = true;
      _keyPair.destroy();
    }
  }
}

class DeviceIdentityManager {
  DeviceIdentityManager({
    required MobileSecureValueStore secureStore,
    Ed25519? algorithm,
    Random? random,
  }) : _secureStore = secureStore,
       _algorithm = algorithm ?? Ed25519(),
       _random = random ?? Random.secure();

  final MobileSecureValueStore _secureStore;
  final Ed25519 _algorithm;
  final Random _random;
  Future<MobileDeviceIdentity>? _loadingIdentity;

  Future<MobileDeviceIdentity> loadOrCreate() async {
    final existingLoad = _loadingIdentity;
    if (existingLoad != null) {
      return existingLoad;
    }

    final pending = _loadOrCreate();
    _loadingIdentity = pending;
    try {
      return await pending;
    } on Object {
      if (identical(_loadingIdentity, pending)) {
        _loadingIdentity = null;
      }
      rethrow;
    }
  }

  Future<MobileDeviceIdentity> _loadOrCreate() async {
    final stored = await _readStoredIdentity();
    if (stored != null) {
      return _identityFromRecord(stored);
    }

    final seed = Uint8List(32);
    for (var index = 0; index < seed.length; index += 1) {
      seed[index] = _random.nextInt(256);
    }
    final record = _encodeRecord(seed);
    try {
      await _secureStore.write(_deviceIdentityStorageKey, record);
      final readBack = await _readStoredIdentity();
      if (readBack == null ||
          !_constantTimeBytesEqual(
            utf8.encode(record),
            utf8.encode(readBack),
          )) {
        throw const DeviceIdentityUnavailableException(
          'native secure store did not preserve the new identity',
        );
      }
      return await _identityFromRecord(readBack);
    } on DeviceIdentityUnavailableException {
      rethrow;
    } on Object catch (error) {
      throw DeviceIdentityUnavailableException(
        'could not create the native device identity',
        error,
      );
    } finally {
      seed.fillRange(0, seed.length, 0);
    }
  }

  Future<String?> _readStoredIdentity() async {
    try {
      return await _secureStore.read(_deviceIdentityStorageKey);
    } on Object catch (error) {
      throw DeviceIdentityUnavailableException(
        'native secure storage is locked or unavailable',
        error,
      );
    }
  }

  Future<MobileDeviceIdentity> _identityFromRecord(String record) async {
    Uint8List? seed;
    try {
      final decoded = jsonDecode(record);
      if (decoded is! Map) {
        throw const FormatException('identity record must be an object');
      }
      final json = Map<String, Object?>.from(decoded);
      if (json.length != 3 ||
          json['version'] != 1 ||
          json['algorithm'] != 'Ed25519' ||
          json['privateSeed'] is! String) {
        throw const FormatException('unsupported device identity record');
      }
      seed = base64Decode(json['privateSeed']! as String);
      if (seed.length != 32) {
        throw const FormatException('Ed25519 seed must contain 32 bytes');
      }

      final keyPair = await _algorithm.newKeyPairFromSeed(seed);
      final publicKey = await keyPair.extractPublicKey();
      final deviceId = hashes.sha256.convert(publicKey.bytes).toString();
      return MobileDeviceIdentity._(
        keyPair: keyPair,
        deviceId: deviceId,
        publicKeyBytes: publicKey.bytes,
        algorithm: _algorithm,
      );
    } on DeviceIdentityUnavailableException {
      rethrow;
    } on Object catch (error) {
      throw DeviceIdentityUnavailableException(
        'stored device identity is invalid; refusing silent replacement',
        error,
      );
    } finally {
      seed?.fillRange(0, seed.length, 0);
    }
  }

  String _encodeRecord(List<int> seed) {
    return jsonEncode({
      'version': 1,
      'algorithm': 'Ed25519',
      'privateSeed': base64Encode(seed),
    });
  }
}

bool _constantTimeBytesEqual(List<int> left, List<int> right) {
  var difference = left.length ^ right.length;
  final comparedLength = max(left.length, right.length);
  for (var index = 0; index < comparedLength; index += 1) {
    final leftByte = index < left.length ? left[index] : 0;
    final rightByte = index < right.length ? right[index] : 0;
    difference |= leftByte ^ rightByte;
  }
  return difference == 0;
}
