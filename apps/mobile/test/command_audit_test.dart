import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/state/command_audit.dart';

void main() {
  const fingerprint =
      'audit:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

  test('accepts complete redacted command audit metadata', () {
    final entry = CommandAuditEntry.fromJson(const {
      'actorFingerprint': fingerprint,
      'action': 'change_assignment',
      'target': 'assignment_target',
      'outcome': 'succeeded',
      'completedAtMs': 100,
    });
    expect(entry.action, 'change_assignment');
    expect(entry.actorFingerprint, fingerprint);
  });

  test('rejects a raw or malformed audit actor identifier', () {
    expect(
      () => CommandAuditEntry.fromJson(const {
        'actorFingerprint': 'device-public-id',
        'action': 'change_assignment',
        'target': 'assignment_target',
        'outcome': 'succeeded',
        'completedAtMs': 100,
      }),
      throwsFormatException,
    );
  });
}
