import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

void main() {
  test('Android excludes all app data from backup and device transfer', () {
    final manifest = File(
      'android/app/src/main/AndroidManifest.xml',
    ).readAsStringSync();
    expect(manifest, contains('android:allowBackup="false"'));
    expect(
      manifest,
      contains('android:dataExtractionRules="@xml/data_extraction_rules"'),
    );
    expect(manifest, contains('android:fullBackupContent="@xml/backup_rules"'));

    final extractionRules = File(
      'android/app/src/main/res/xml/data_extraction_rules.xml',
    ).readAsStringSync();
    for (final section in ['cloud-backup', 'device-transfer']) {
      expect(extractionRules, contains('<$section>'));
    }
    for (final domain in [
      'root',
      'file',
      'database',
      'sharedpref',
      'external',
      'device_root',
      'device_file',
      'device_database',
      'device_sharedpref',
    ]) {
      expect(
        RegExp(
          '<exclude domain="$domain" path="\\." />',
        ).allMatches(extractionRules).length,
        2,
        reason: '$domain must be excluded from cloud and device transfer',
      );
    }

    final legacyRules = File(
      'android/app/src/main/res/xml/backup_rules.xml',
    ).readAsStringSync();
    for (final domain in [
      'root',
      'file',
      'database',
      'sharedpref',
      'external',
    ]) {
      expect(legacyRules, contains('<exclude domain="$domain" path="." />'));
    }
  });

  test('iOS device identity is device-bound and never iCloud-synchronized', () {
    final source = File('lib/security/device_identity.dart').readAsStringSync();
    expect(source, contains('KeychainAccessibility.first_unlock_this_device'));
    expect(source, contains('synchronizable: false'));
  });
}
