import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/security/sensitive_inputs.dart';

void main() {
  test('lifecycle clear wipes every registered plaintext field', () {
    final inputs = SensitiveInputRegistry();
    final pairingCode = inputs.createController();
    final futureCredential = inputs.createController();
    pairingCode.text = 'signed pairing offer';
    futureCredential.text = 'provider credential';

    inputs.clear();

    expect(pairingCode.text, isEmpty);
    expect(futureCredential.text, isEmpty);
    inputs.release(pairingCode);
    inputs.dispose();
    expect(inputs.createController, throwsA(isA<StateError>()));
  });
}
