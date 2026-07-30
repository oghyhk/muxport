import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/state/mobile_cache_store.dart';
import 'package:muxport_mobile/state/mobile_sync_state.dart';

void main() {
  late Directory temporaryDirectory;
  late GenerationMobileCacheStore store;

  setUp(() async {
    temporaryDirectory = await Directory.systemTemp.createTemp(
      'muxport-mobile-cache-test-',
    );
    store = GenerationMobileCacheStore(temporaryDirectory);
    await store.loadLatest();
  });

  tearDown(() async {
    if (await temporaryDirectory.exists()) {
      await temporaryDirectory.delete(recursive: true);
    }
  });

  test('round trip restores host projection as stale', () async {
    final generation = await store.save(
      MobileCacheSnapshot(hosts: [_host(session: 'live')]),
    );

    final loaded = await store.loadLatest();

    expect(generation, 1);
    expect(loaded.generation, 1);
    expect(loaded.recoveredFromPreviousGeneration, isFalse);
    expect(loaded.snapshot.hosts.keys, ['host-1']);
    expect(loaded.snapshot.hosts['host-1']?.phase, HostSyncPhase.cachedStale);
    expect(loaded.snapshot.hosts['host-1']?.snapshot['session'], 'live');
  });

  test('partial newest generation falls back to last valid cache', () async {
    await store.save(MobileCacheSnapshot(hosts: [_host(session: 'first')]));
    await store.save(MobileCacheSnapshot(hosts: [_host(session: 'second')]));
    final partial = File(
      '${temporaryDirectory.path}${Platform.pathSeparator}'
      'host-cache-00000000000000000003.json',
    );
    await partial.writeAsString('{"envelopeVersion":1', flush: true);

    final loaded = await store.loadLatest();

    expect(loaded.generation, 2);
    expect(loaded.recoveredFromPreviousGeneration, isTrue);
    expect(loaded.snapshot.hosts['host-1']?.snapshot['session'], 'second');
    expect(await partial.exists(), isTrue);
  });

  test('all corrupt generations fail without deleting evidence', () async {
    final corrupt = File(
      '${temporaryDirectory.path}${Platform.pathSeparator}'
      'host-cache-00000000000000000001.json',
    );
    await corrupt.writeAsString('not-json', flush: true);

    await expectLater(
      store.loadLatest(),
      throwsA(isA<MobileCacheRecoveryException>()),
    );
    await expectLater(
      store.save(MobileCacheSnapshot(hosts: [_host(session: 'replacement')])),
      throwsA(isA<MobileCacheRecoveryException>()),
    );
    expect(await corrupt.exists(), isTrue);
  });

  test('save requires recovery to run first', () async {
    final unopenedStore = GenerationMobileCacheStore(temporaryDirectory);

    await expectLater(
      unopenedStore.save(
        MobileCacheSnapshot(hosts: [_host(session: 'replacement')]),
      ),
      throwsStateError,
    );
  });

  test('verified save retains current and previous generation only', () async {
    await store.save(MobileCacheSnapshot(hosts: [_host(session: 'one')]));
    await store.save(MobileCacheSnapshot(hosts: [_host(session: 'two')]));
    await store.save(MobileCacheSnapshot(hosts: [_host(session: 'three')]));

    final files = await temporaryDirectory
        .list()
        .where((entity) => entity is File)
        .toList();
    final loaded = await store.loadLatest();

    expect(files, hasLength(2));
    expect(loaded.generation, 3);
    expect(loaded.snapshot.hosts['host-1']?.snapshot['session'], 'three');
  });

  test('concurrent saves serialize into unique generations', () async {
    final generations = await Future.wait([
      store.save(MobileCacheSnapshot(hosts: [_host(session: 'one')])),
      store.save(MobileCacheSnapshot(hosts: [_host(session: 'two')])),
      store.save(MobileCacheSnapshot(hosts: [_host(session: 'three')])),
    ]);

    final loaded = await store.loadLatest();

    expect(generations, [1, 2, 3]);
    expect(loaded.generation, 3);
    expect(loaded.snapshot.hosts['host-1']?.snapshot['session'], 'three');
  });

  test('secret-shaped fields are rejected before writing', () async {
    final unsafeHost = _host(
      session: 'cached',
      extraSnapshot: const {'api_key': 'must-not-reach-disk'},
    );

    await expectLater(
      store.save(MobileCacheSnapshot(hosts: [unsafeHost])),
      throwsA(isA<FormatException>()),
    );

    expect(await temporaryDirectory.list().isEmpty, isTrue);
  });
}

HostSyncState _host({
  required String session,
  Map<String, Object?> extraSnapshot = const {},
}) {
  return HostSyncState(
    hostId: 'host-1',
    pinnedHostKey: 'public-host-key',
    displayName: 'Development host',
    protocolVersion: mobileProtocolVersion,
    phase: HostSyncPhase.synchronized,
    snapshot: {'session': session, ...extraSnapshot},
    cursor: const SyncCursor(hostEpoch: 'epoch-a', sequence: 4),
    sourceVersions: const {'session-1': 1},
    recentEventIds: const [],
    pendingOperations: const {},
  );
}
