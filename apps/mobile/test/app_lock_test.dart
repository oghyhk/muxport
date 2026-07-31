import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/security/app_lock.dart';
import 'package:muxport_mobile/security/device_identity.dart';

void main() {
  test(
    'app-lock preference reads back through the secure-store boundary',
    () async {
      final store = _MemorySecureStore();
      final preference = AppLockPreference(secureStore: store);

      expect(await preference.load(), isFalse);
      expect(await preference.setEnabled(true), isTrue);
      expect(await preference.load(), isTrue);
      expect(await preference.setEnabled(false), isTrue);
      expect(await preference.load(), isFalse);
    },
  );
}

class _MemorySecureStore implements MobileSecureValueStore {
  final Map<String, String> _values = {};

  @override
  Future<String?> read(String key) async => _values[key];

  @override
  Future<void> write(String key, String value) async {
    _values[key] = value;
  }
}
