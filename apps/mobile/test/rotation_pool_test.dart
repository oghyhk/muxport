import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/state/rotation_pool.dart';

void main() {
  test('accepts only complete non-secret rotation policy metadata', () {
    final pool = RotationPoolSummary.fromJson(const {
      'poolId': 'go-accounts',
      'providerId': 'opencode-go',
      'orderedProfileIds': ['account-a', 'account-b'],
      'mode': 'round_robin',
      'cooldownMs': 60000,
      'maxSwitchesPerHour': 3,
      'allowedHostIds': ['host-1'],
      'quotaFailoverEnabled': false,
    });
    expect(pool.poolId, 'go-accounts');
    expect(pool.orderedProfileIds, ['account-a', 'account-b']);
    expect(pool.allowedHostIds, ['host-1']);
  });

  test('rejects malformed pool metadata', () {
    expect(
      () => RotationPoolSummary.fromJson(const {
        'poolId': 'bad',
        'providerId': 'opencode-go',
        'orderedProfileIds': ['only-one'],
        'mode': 'manual',
      }),
      throwsFormatException,
    );
    expect(
      () => RotationPoolSummary.fromJson(const {
        'poolId': 'duplicate-accounts',
        'providerId': 'opencode-go',
        'orderedProfileIds': ['account-a', 'account-a'],
        'mode': 'round_robin',
        'cooldownMs': 60000,
        'maxSwitchesPerHour': 3,
        'allowedHostIds': ['host-1'],
        'quotaFailoverEnabled': false,
      }),
      throwsFormatException,
    );
  });
}
