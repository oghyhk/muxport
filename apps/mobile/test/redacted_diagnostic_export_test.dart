import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/diagnostics/redacted_diagnostic_export.dart';
import 'package:muxport_mobile/security/sensitive_artifacts.dart';
import 'package:muxport_mobile/state/app_bootstrap.dart';
import 'package:muxport_mobile/state/mobile_cache_store.dart';
import 'package:muxport_mobile/state/mobile_sync_state.dart';

void main() {
  test('export enumerates files and omits sensitive host state', () async {
    final root = await Directory.systemTemp.createTemp(
      'muxport-redacted-export-',
    );
    final store = SensitiveArtifactStore(
      Directory.fromUri(root.uri.resolve('sensitive/')),
    );
    const secrets = [
      'secret-host-id',
      'private-host-pin',
      'Personal workstation',
      '10.0.0.7',
      '/private/repository',
      'sensitive session title',
      'operation-secret-id',
      'account-secret-fingerprint',
    ];
    final host = HostSyncState(
      hostId: secrets[0],
      pinnedHostKey: secrets[1],
      displayName: secrets[2],
      protocolVersion: mobileProtocolVersion,
      phase: HostSyncPhase.synchronized,
      directAddress: secrets[3],
      directPort: 45821,
      snapshot: {
        'runtimes': const [{}],
        'activeSessions': [
          {'projectPath': secrets[4], 'title': secrets[5]},
        ],
        'credentialProfiles': [
          {'accountFingerprint': secrets[7]},
        ],
      },
      cursor: const SyncCursor(hostEpoch: 'secret-epoch', sequence: 42),
      sourceVersions: const {},
      recentEventIds: const ['secret-event-id'],
      pendingOperations: {
        secrets[6]: PendingOperation.local(
          idempotencyKey: secrets[6],
          kind: 'approval:secret-approval-id',
          createdAtMs: 1,
          deadlineMs: 100,
        ),
      },
    );
    final bootstrap = AppBootstrapState(
      cache: MobileCacheSnapshot(hosts: [host]),
      cacheGeneration: 3,
      cacheStatus: CacheBootstrapStatus.ready,
      identity: null,
      identityStatus: IdentityBootstrapStatus.unavailable,
      sensitiveArtifactStore: store,
    );
    try {
      final export = await RedactedDiagnosticExporter(
        store,
      ).create(bootstrap: bootstrap, hosts: [host], generatedAtMs: 1234);
      expect(
        export.files.map((file) => file.uri.pathSegments.last),
        redactedDiagnosticFileNames,
      );
      final combined = (await Future.wait(
        export.files.map((file) => file.readAsString()),
      )).join();
      for (final secret in secrets) {
        expect(combined, isNot(contains(secret)));
      }
      expect(combined, contains('"includedFiles"'));
      expect(combined, contains('"activeSessionCount": 1'));
      expect(combined, contains('"pending": 1'));
    } finally {
      if (await root.exists()) {
        await root.delete(recursive: true);
      }
    }
  });
}
