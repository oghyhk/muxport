import '../security/device_identity.dart';
import '../security/sensitive_artifacts.dart';
import 'mobile_cache_store.dart';

enum CacheBootstrapStatus {
  ready,
  recoveredPreviousGeneration,
  corrupt,
  unavailable,
}

enum IdentityBootstrapStatus { ready, unavailable }

class AppBootstrapState {
  const AppBootstrapState({
    required this.cache,
    required this.cacheGeneration,
    required this.cacheStatus,
    required this.identity,
    required this.identityStatus,
    this.cacheStore,
    this.sensitiveArtifactStore,
  });

  factory AppBootstrapState.emptyForTest() {
    return AppBootstrapState(
      cache: MobileCacheSnapshot.empty(),
      cacheGeneration: 0,
      cacheStatus: CacheBootstrapStatus.ready,
      identity: null,
      identityStatus: IdentityBootstrapStatus.unavailable,
      cacheStore: null,
      sensitiveArtifactStore: null,
    );
  }

  final MobileCacheSnapshot cache;
  final int cacheGeneration;
  final CacheBootstrapStatus cacheStatus;
  final MobileDeviceIdentity? identity;
  final IdentityBootstrapStatus identityStatus;
  final GenerationMobileCacheStore? cacheStore;
  final SensitiveArtifactStore? sensitiveArtifactStore;

  bool get canAuthenticateTransport =>
      (cacheStatus == CacheBootstrapStatus.ready ||
          cacheStatus == CacheBootstrapStatus.recoveredPreviousGeneration) &&
      identityStatus == IdentityBootstrapStatus.ready &&
      identity != null;
}

class PlatformAppBootstrap {
  const PlatformAppBootstrap._();

  static Future<AppBootstrapState> load() async {
    SensitiveArtifactStore? sensitiveArtifactStore;
    try {
      sensitiveArtifactStore = await SensitiveArtifactStore.createDefault();
      await sensitiveArtifactStore.clear();
    } on Object {
      sensitiveArtifactStore = null;
    }

    var cache = MobileCacheSnapshot.empty();
    var cacheGeneration = 0;
    var cacheStatus = CacheBootstrapStatus.ready;
    GenerationMobileCacheStore? cacheStore;

    try {
      cacheStore = await GenerationMobileCacheStore.createDefault();
      final result = await cacheStore.loadLatest();
      cache = result.snapshot;
      cacheGeneration = result.generation;
      cacheStatus = result.recoveredFromPreviousGeneration
          ? CacheBootstrapStatus.recoveredPreviousGeneration
          : CacheBootstrapStatus.ready;
    } on MobileCacheRecoveryException {
      cacheStatus = CacheBootstrapStatus.corrupt;
    } on Object {
      cacheStatus = CacheBootstrapStatus.unavailable;
    }

    MobileDeviceIdentity? identity;
    var identityStatus = IdentityBootstrapStatus.unavailable;
    try {
      identity = await DeviceIdentityManager(
        secureStore: PlatformMobileSecureValueStore(),
      ).loadOrCreate();
      identityStatus = IdentityBootstrapStatus.ready;
    } on DeviceIdentityUnavailableException {
      identityStatus = IdentityBootstrapStatus.unavailable;
    } on Object {
      identityStatus = IdentityBootstrapStatus.unavailable;
    }

    return AppBootstrapState(
      cache: cache,
      cacheGeneration: cacheGeneration,
      cacheStatus: cacheStatus,
      identity: identity,
      identityStatus: identityStatus,
      cacheStore: cacheStore,
      sensitiveArtifactStore: sensitiveArtifactStore,
    );
  }
}
