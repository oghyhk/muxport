import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:path_provider/path_provider.dart';

import 'mobile_sync_state.dart';

const int _cacheEnvelopeVersion = 1;
const int _maxCachePayloadBytes = 8 * 1024 * 1024;
const int _maxCacheEnvelopeBytes = 12 * 1024 * 1024;
const Set<String> _forbiddenCacheKeys = {
  'secret',
  'token',
  'accesstoken',
  'refreshtoken',
  'apikey',
  'password',
  'authorization',
  'privatekey',
  'credentialvalue',
  'bearer',
  'plaintext',
};

class MobileCacheSnapshot {
  MobileCacheSnapshot({required Iterable<HostSyncState> hosts})
    : hosts = Map.unmodifiable({for (final host in hosts) host.hostId: host}) {
    if (this.hosts.length != hosts.length) {
      throw ArgumentError('cache contains duplicate host IDs');
    }
  }

  factory MobileCacheSnapshot.empty() {
    return MobileCacheSnapshot(hosts: const []);
  }

  final Map<String, HostSyncState> hosts;

  Map<String, Object?> toJson() {
    final orderedHosts = hosts.values.toList(growable: false)
      ..sort((left, right) => left.hostId.compareTo(right.hostId));
    return {
      'schemaVersion': 1,
      'hosts': orderedHosts
          .map((host) => host.toCacheJson())
          .toList(growable: false),
    };
  }

  factory MobileCacheSnapshot.fromJson(Map<String, Object?> json) {
    if (json['schemaVersion'] != 1) {
      throw const FormatException('unsupported host-cache schema');
    }
    final rawHosts = json['hosts'];
    if (rawHosts is! List) {
      throw const FormatException('host-cache hosts must be a list');
    }
    return MobileCacheSnapshot(
      hosts: rawHosts.map(
        (rawHost) => HostSyncState.fromCacheJson(
          Map<String, Object?>.from(rawHost as Map),
        ),
      ),
    );
  }
}

class MobileCacheLoadResult {
  const MobileCacheLoadResult({
    required this.snapshot,
    required this.generation,
    required this.recoveredFromPreviousGeneration,
  });

  final MobileCacheSnapshot snapshot;
  final int generation;
  final bool recoveredFromPreviousGeneration;
}

class MobileCacheRecoveryException implements Exception {
  const MobileCacheRecoveryException(this.message);

  final String message;

  @override
  String toString() => 'MobileCacheRecoveryException: $message';
}

class GenerationMobileCacheStore {
  GenerationMobileCacheStore(this.directory, {this.retainedGenerations = 2}) {
    if (retainedGenerations < 2) {
      throw ArgumentError.value(
        retainedGenerations,
        'retainedGenerations',
        'must retain the current and previous generation',
      );
    }
  }

  static Future<GenerationMobileCacheStore> createDefault() async {
    final applicationSupport = await getApplicationSupportDirectory();
    final separator = Platform.pathSeparator;
    return GenerationMobileCacheStore(
      Directory(
        '${applicationSupport.path}${separator}muxport'
        '${separator}mobile-cache',
      ),
    );
  }

  final Directory directory;
  final int retainedGenerations;
  Future<void> _writeTail = Future.value();
  bool _hasLoaded = false;
  bool _recoveryBlocked = false;

  Future<MobileCacheLoadResult> loadLatest() async {
    final files = await _generationFiles();
    if (files.isEmpty) {
      _hasLoaded = true;
      _recoveryBlocked = false;
      return MobileCacheLoadResult(
        snapshot: MobileCacheSnapshot.empty(),
        generation: 0,
        recoveredFromPreviousGeneration: false,
      );
    }

    Object? lastError;
    var invalidNewerGeneration = false;
    for (final entry in files.reversed) {
      try {
        final snapshot = await _readGeneration(entry);
        _hasLoaded = true;
        _recoveryBlocked = false;
        return MobileCacheLoadResult(
          snapshot: snapshot,
          generation: entry.generation,
          recoveredFromPreviousGeneration: invalidNewerGeneration,
        );
      } on Object catch (error) {
        lastError = error;
        invalidNewerGeneration = true;
      }
    }

    _hasLoaded = true;
    _recoveryBlocked = true;
    throw MobileCacheRecoveryException(
      'all ${files.length} cache generations are invalid; '
      'preserving them for diagnostics ($lastError)',
    );
  }

  Future<int> save(MobileCacheSnapshot snapshot) {
    final result = _writeTail.then((_) => _saveNow(snapshot));
    _writeTail = result.then<void>((_) {}, onError: (_, __) {});
    return result;
  }

  Future<int> _saveNow(MobileCacheSnapshot snapshot) async {
    if (!_hasLoaded) {
      throw StateError('loadLatest must run before the first cache save');
    }
    if (_recoveryBlocked) {
      throw const MobileCacheRecoveryException(
        'cache recovery is blocked; corrupt generations are preserved',
      );
    }

    final payload = snapshot.toJson();
    _rejectSensitiveCacheValues(payload);
    final payloadBytes = utf8.encode(jsonEncode(payload));
    if (payloadBytes.length > _maxCachePayloadBytes) {
      throw StateError(
        'mobile cache exceeds $_maxCachePayloadBytes payload bytes',
      );
    }

    await directory.create(recursive: true);
    final existing = await _generationFiles();
    final generation = existing.isEmpty ? 1 : existing.last.generation + 1;
    final file = File(_generationPath(generation));
    if (await file.exists()) {
      throw StateError('cache generation collision at $generation');
    }

    final envelope = <String, Object?>{
      'envelopeVersion': _cacheEnvelopeVersion,
      'generation': generation,
      'payloadBase64': base64Encode(payloadBytes),
      'payloadSha256': sha256.convert(payloadBytes).toString(),
    };
    await file.writeAsBytes(utf8.encode(jsonEncode(envelope)), flush: true);

    // Do not remove the previous generation until the new bytes have been
    // reopened and fully validated through the same recovery path.
    await _readGeneration(_GenerationFile(generation: generation, file: file));
    await _pruneOldGenerations(generation);
    return generation;
  }

  Future<MobileCacheSnapshot> _readGeneration(_GenerationFile entry) async {
    final length = await entry.file.length();
    if (length <= 0 || length > _maxCacheEnvelopeBytes) {
      throw const FormatException('invalid cache envelope length');
    }

    final decodedEnvelope = jsonDecode(await entry.file.readAsString());
    if (decodedEnvelope is! Map) {
      throw const FormatException('cache envelope must be an object');
    }
    final envelope = Map<String, Object?>.from(decodedEnvelope);
    if (envelope['envelopeVersion'] != _cacheEnvelopeVersion ||
        envelope['generation'] != entry.generation) {
      throw const FormatException('cache envelope metadata mismatch');
    }

    final encodedPayload = envelope['payloadBase64'];
    final expectedDigest = envelope['payloadSha256'];
    if (encodedPayload is! String || expectedDigest is! String) {
      throw const FormatException('cache envelope payload is malformed');
    }
    final payloadBytes = base64Decode(encodedPayload);
    if (payloadBytes.length > _maxCachePayloadBytes ||
        sha256.convert(payloadBytes).toString() != expectedDigest) {
      throw const FormatException('cache payload checksum mismatch');
    }

    final decodedPayload = jsonDecode(utf8.decode(payloadBytes));
    if (decodedPayload is! Map) {
      throw const FormatException('cache payload must be an object');
    }
    final payload = Map<String, Object?>.from(decodedPayload);
    _rejectSensitiveCacheValues(payload);
    return MobileCacheSnapshot.fromJson(payload);
  }

  Future<List<_GenerationFile>> _generationFiles() async {
    if (!await directory.exists()) {
      return [];
    }

    final files = <_GenerationFile>[];
    await for (final entity in directory.list(followLinks: false)) {
      if (entity is! File) {
        continue;
      }
      final name = entity.path.split(Platform.pathSeparator).last;
      final match = RegExp(r'^host-cache-(\d{20})\.json$').firstMatch(name);
      if (match == null) {
        continue;
      }
      final generation = int.tryParse(match.group(1)!);
      if (generation == null || generation < 1) {
        continue;
      }
      files.add(_GenerationFile(generation: generation, file: entity));
    }
    files.sort((left, right) => left.generation.compareTo(right.generation));
    return files;
  }

  Future<void> _pruneOldGenerations(int committedGeneration) async {
    final files = await _generationFiles();
    final valid = <_GenerationFile>[];
    for (final entry in files.reversed) {
      if (entry.generation > committedGeneration) {
        continue;
      }
      try {
        await _readGeneration(entry);
        valid.add(entry);
      } on Object {
        await entry.file.delete();
      }
    }

    for (final entry in valid.skip(retainedGenerations)) {
      await entry.file.delete();
    }
  }

  String _generationPath(int generation) {
    final separator = Platform.pathSeparator;
    final name = generation.toString().padLeft(20, '0');
    return '${directory.path}${separator}host-cache-$name.json';
  }
}

class _GenerationFile {
  const _GenerationFile({required this.generation, required this.file});

  final int generation;
  final File file;
}

void _rejectSensitiveCacheValues(Object? value, [String path = r'$']) {
  if (value is Map) {
    for (final entry in value.entries) {
      final key = entry.key;
      if (key is! String) {
        throw FormatException('cache key at $path is not a string');
      }
      final normalizedKey = key.toLowerCase().replaceAll(
        RegExp('[^a-z0-9]'),
        '',
      );
      if (_forbiddenCacheKeys.contains(normalizedKey)) {
        throw FormatException('sensitive field is forbidden at $path.$key');
      }
      _rejectSensitiveCacheValues(entry.value, '$path.$key');
    }
    return;
  }
  if (value is List) {
    for (var index = 0; index < value.length; index += 1) {
      _rejectSensitiveCacheValues(value[index], '$path[$index]');
    }
  }
}
