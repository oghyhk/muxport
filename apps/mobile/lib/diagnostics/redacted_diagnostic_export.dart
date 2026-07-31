import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:crypto/crypto.dart';

import '../security/sensitive_artifacts.dart';
import '../state/app_bootstrap.dart';
import '../state/mobile_sync_state.dart';

const List<String> redactedDiagnosticFileNames = [
  'manifest.json',
  'mobile-status.json',
];

const List<String> redactedDiagnosticExclusions = [
  'provider credentials and authentication material',
  'device and host private or public identity keys',
  'host labels, addresses, and project paths',
  'session titles, prompts, output, and approval payloads',
  'raw snapshots, events, operation IDs, and vendor state',
];

class RedactedDiagnosticExport {
  const RedactedDiagnosticExport({
    required this.directory,
    required this.files,
  });

  final Directory directory;
  final List<File> files;
}

class RedactedDiagnosticExporter {
  const RedactedDiagnosticExporter(this.artifactStore);

  final SensitiveArtifactStore artifactStore;

  Future<RedactedDiagnosticExport> create({
    required AppBootstrapState bootstrap,
    required Iterable<HostSyncState> hosts,
    int? generatedAtMs,
  }) async {
    await artifactStore.clear();
    final root = await artifactStore.prepare();
    final timestamp = generatedAtMs ?? DateTime.now().millisecondsSinceEpoch;
    final directory = Directory.fromUri(
      root.uri.resolve('diagnostic-export-$timestamp/'),
    );
    await directory.create();

    final orderedHosts = hosts.toList(growable: false)
      ..sort((left, right) => left.hostId.compareTo(right.hostId));
    final random = Random.secure();
    final anonymousIdSalt = List<int>.generate(
      32,
      (_) => random.nextInt(256),
      growable: false,
    );
    final status = <String, Object?>{
      'schemaVersion': 1,
      'generatedAtMs': timestamp,
      'mobileProtocolVersion': mobileProtocolVersion,
      'cacheStatus': bootstrap.cacheStatus.name,
      'cacheGeneration': bootstrap.cacheGeneration,
      'identityStatus': bootstrap.identityStatus.name,
      'hosts': orderedHosts
          .map((host) => _redactedHostStatus(host, anonymousIdSalt))
          .toList(growable: false),
    };
    final manifest = <String, Object?>{
      'schemaVersion': 1,
      'generatedAtMs': timestamp,
      'includedFiles': redactedDiagnosticFileNames,
      'excludedData': redactedDiagnosticExclusions,
    };

    final manifestFile = File.fromUri(directory.uri.resolve('manifest.json'));
    final statusFile = File.fromUri(
      directory.uri.resolve('mobile-status.json'),
    );
    await manifestFile.writeAsString(
      const JsonEncoder.withIndent('  ').convert(manifest),
      flush: true,
    );
    await statusFile.writeAsString(
      const JsonEncoder.withIndent('  ').convert(status),
      flush: true,
    );
    return RedactedDiagnosticExport(
      directory: directory,
      files: [manifestFile, statusFile],
    );
  }

  static Map<String, Object?> _redactedHostStatus(
    HostSyncState host,
    List<int> anonymousIdSalt,
  ) {
    final operationCounts = <String, int>{};
    for (final operation in host.pendingOperations.values) {
      operationCounts.update(
        operation.state.name,
        (count) => count + 1,
        ifAbsent: () => 1,
      );
    }
    return {
      'anonymousHostId': sha256
          .convert([
            ...anonymousIdSalt,
            ...utf8.encode('muxport-diagnostic-host-v1:${host.hostId}'),
          ])
          .toString()
          .substring(0, 16),
      'phase': host.phase.name,
      'protocolVersion': host.protocolVersion,
      'hasReconnectEndpoint': host.canReconnect,
      'cursorSequence': host.cursor?.sequence,
      'runtimeCount': (host.snapshot['runtimes'] as List?)?.length ?? 0,
      'activeSessionCount':
          (host.snapshot['activeSessions'] as List?)?.length ?? 0,
      'credentialProfileCount':
          (host.snapshot['credentialProfiles'] as List?)?.length ?? 0,
      'operationCountsByState': operationCounts,
    };
  }
}
