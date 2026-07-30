import 'dart:typed_data';

import 'package:cryptography/cryptography.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/security/device_identity.dart';

void main() {
  test('creates one identity and reopens the same key after restart', () async {
    final store = _FakeSecureValueStore();
    final first = await DeviceIdentityManager(
      secureStore: store,
    ).loadOrCreate();
    final second = await DeviceIdentityManager(
      secureStore: store,
    ).loadOrCreate();

    expect(store.writeCount, 1);
    expect(second.deviceId, first.deviceId);
    expect(second.publicKeyBytes, first.publicKeyBytes);
    expect(first.deviceId, hasLength(64));

    await first.destroy();
    await second.destroy();
  });

  test('signatures verify against the exported public identity', () async {
    final identity = await DeviceIdentityManager(
      secureStore: _FakeSecureValueStore(),
    ).loadOrCreate();
    final message = Uint8List.fromList([1, 2, 3, 4]);

    final signatureBytes = await identity.sign(message);
    final verified = await Ed25519().verify(
      message,
      signature: Signature(
        signatureBytes,
        publicKey: SimplePublicKey(
          identity.publicKeyBytes,
          type: KeyPairType.ed25519,
        ),
      ),
    );

    expect(verified, isTrue);
    await identity.destroy();
    await expectLater(identity.sign(message), throwsStateError);
  });

  test('concurrent callers share one secure-store enrollment', () async {
    final store = _FakeSecureValueStore(
      writeDelay: const Duration(milliseconds: 5),
    );
    final manager = DeviceIdentityManager(secureStore: store);

    final identities = await Future.wait([
      manager.loadOrCreate(),
      manager.loadOrCreate(),
      manager.loadOrCreate(),
    ]);

    expect(store.writeCount, 1);
    expect(identical(identities[0], identities[1]), isTrue);
    expect(identical(identities[1], identities[2]), isTrue);
    await identities.first.destroy();
  });

  test('corrupt stored identity is never silently replaced', () async {
    final store = _FakeSecureValueStore(
      initialValue: '{"version":1,"algorithm":"Ed25519","privateSeed":"bad"}',
    );

    await expectLater(
      DeviceIdentityManager(secureStore: store).loadOrCreate(),
      throwsA(isA<DeviceIdentityUnavailableException>()),
    );

    expect(store.writeCount, 0);
    expect(store.value, contains('"privateSeed":"bad"'));
  });

  test('locked secure storage fails without creating a fallback', () async {
    final store = _FakeSecureValueStore(readError: StateError('locked'));

    await expectLater(
      DeviceIdentityManager(secureStore: store).loadOrCreate(),
      throwsA(isA<DeviceIdentityUnavailableException>()),
    );

    expect(store.writeCount, 0);
    expect(store.value, isNull);
  });

  test('failed write verification does not return a new identity', () async {
    final store = _FakeSecureValueStore(corruptAfterWrite: true);

    await expectLater(
      DeviceIdentityManager(secureStore: store).loadOrCreate(),
      throwsA(isA<DeviceIdentityUnavailableException>()),
    );

    expect(store.writeCount, 1);
    expect(store.value, 'corrupt-after-write');
  });
}

class _FakeSecureValueStore implements MobileSecureValueStore {
  _FakeSecureValueStore({
    String? initialValue,
    this.readError,
    this.corruptAfterWrite = false,
    this.writeDelay = Duration.zero,
  }) : value = initialValue;

  String? value;
  final Object? readError;
  final bool corruptAfterWrite;
  final Duration writeDelay;
  int writeCount = 0;

  @override
  Future<String?> read(String key) async {
    final error = readError;
    if (error != null) {
      throw error;
    }
    return value;
  }

  @override
  Future<void> write(String key, String nextValue) async {
    writeCount += 1;
    if (writeDelay > Duration.zero) {
      await Future<void>.delayed(writeDelay);
    }
    value = corruptAfterWrite ? 'corrupt-after-write' : nextValue;
  }
}
