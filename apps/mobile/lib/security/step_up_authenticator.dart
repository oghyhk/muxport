import 'package:local_auth/local_auth.dart';

/// A device-local authorization boundary for actions that can add, export, or
/// redirect provider credentials. The connector still authorizes the paired
/// device; this protects use of an unlocked phone before any secret leaves it.
abstract interface class StepUpAuthenticator {
  Future<bool> authorize({required String reason});
}

class PlatformStepUpAuthenticator implements StepUpAuthenticator {
  PlatformStepUpAuthenticator({LocalAuthentication? localAuthentication})
    : _localAuthentication = localAuthentication ?? LocalAuthentication();

  final LocalAuthentication _localAuthentication;

  @override
  Future<bool> authorize({required String reason}) async {
    if (reason.trim().isEmpty) {
      return false;
    }
    try {
      final canAuthenticate =
          await _localAuthentication.canCheckBiometrics ||
          await _localAuthentication.isDeviceSupported();
      if (!canAuthenticate) {
        return false;
      }
      return await _localAuthentication.authenticate(
        localizedReason: reason,
        // Device credentials are an acceptable fallback when the OS has no
        // biometric enrollment. We do not provide an app-managed fallback.
        biometricOnly: false,
        sensitiveTransaction: true,
        persistAcrossBackgrounding: false,
      );
    } on LocalAuthException {
      return false;
    } on Object {
      return false;
    }
  }
}

class DenyingStepUpAuthenticator implements StepUpAuthenticator {
  const DenyingStepUpAuthenticator();

  @override
  Future<bool> authorize({required String reason}) async => false;
}
