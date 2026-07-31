import 'device_identity.dart';

const _appLockPreferenceKey = 'muxport.app_lock.enabled.v1';

/// Stores only whether the optional local app lock is enabled. The preference
/// is protected by the native secure store; no app PIN, biometric template, or
/// unlock material is ever retained by Muxport.
class AppLockPreference {
  AppLockPreference({MobileSecureValueStore? secureStore})
    : _secureStore = secureStore ?? PlatformMobileSecureValueStore();

  final MobileSecureValueStore _secureStore;

  Future<bool> load() async {
    try {
      return await _secureStore.read(_appLockPreferenceKey) == 'enabled';
    } on Object {
      // A secure-store failure must never make the UI claim protection is
      // enabled. Action-level step-up checks remain in place independently.
      return false;
    }
  }

  Future<bool> setEnabled(bool enabled) async {
    try {
      await _secureStore.write(
        _appLockPreferenceKey,
        enabled ? 'enabled' : 'disabled',
      );
      return await load() == enabled;
    } on Object {
      return false;
    }
  }
}
