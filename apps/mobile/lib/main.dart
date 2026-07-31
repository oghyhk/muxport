import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_protocol/flutter_protocol.dart';
import 'package:flutter_protocol/wire_protocol.dart' as wire;

import 'pairing/signed_pairing_offer.dart';
import 'security/app_lock.dart';
import 'security/device_identity.dart';
import 'security/sensitive_inputs.dart';
import 'security/step_up_authenticator.dart';
import 'screens/host_fleet_screen.dart';
import 'screens/session_timeline_screen.dart';
import 'screens/approval_inbox_screen.dart';
import 'screens/credential_matrix_screen.dart';
import 'screens/credential_provisioning_screen.dart';
import 'screens/diagnostics_screen.dart';
import 'state/app_bootstrap.dart';
import 'state/bulk_switch_plan.dart';
import 'state/command_audit.dart';
import 'state/host_sync_orchestrator.dart';
import 'state/mobile_cache_store.dart';
import 'state/mobile_lifecycle.dart';
import 'state/mobile_sync_state.dart';
import 'state/rotation_pool.dart';
import 'transport/direct_transport_client.dart';
import 'transport/direct_transport_protocol.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const MuxportApp());
}

enum _BulkAssignmentOutcome { confirmed, rejected, unknown, unavailable }

class MuxportApp extends StatelessWidget {
  const MuxportApp({super.key, this.bootstrap});

  final Future<AppBootstrapState>? bootstrap;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Muxport',
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF6200EE),
          brightness: Brightness.dark,
        ),
        useMaterial3: true,
      ),
      home: _BootstrapBoundary(
        bootstrap: bootstrap ?? PlatformAppBootstrap.load(),
      ),
      builder: (context, child) => Banner(
        message: 'UNWIRED UI',
        location: BannerLocation.topEnd,
        child: child ?? const SizedBox.shrink(),
      ),
      debugShowCheckedModeBanner: false,
    );
  }
}

class _BootstrapBoundary extends StatefulWidget {
  const _BootstrapBoundary({required this.bootstrap});

  final Future<AppBootstrapState> bootstrap;

  @override
  State<_BootstrapBoundary> createState() => _BootstrapBoundaryState();
}

class _BootstrapBoundaryState extends State<_BootstrapBoundary> {
  AppBootstrapState? _completedBootstrap;

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<AppBootstrapState>(
      future: widget.bootstrap,
      builder: (context, snapshot) {
        if (snapshot.hasError) {
          return const _StartupFailureScreen();
        }
        final bootstrap = snapshot.data;
        if (bootstrap == null) {
          return const _StartupLoadingScreen();
        }
        _completedBootstrap = bootstrap;
        return MainNavigationScreen(bootstrap: bootstrap);
      },
    );
  }

  @override
  void dispose() {
    final identity = _completedBootstrap?.identity;
    if (identity != null) {
      unawaited(identity.destroy());
    }
    super.dispose();
  }
}

class _StartupLoadingScreen extends StatelessWidget {
  const _StartupLoadingScreen();

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(
        child: Semantics(
          label: 'Loading protected Muxport state',
          child: const CircularProgressIndicator(),
        ),
      ),
    );
  }
}

class _StartupFailureScreen extends StatelessWidget {
  const _StartupFailureScreen();

  @override
  Widget build(BuildContext context) {
    return const Scaffold(
      body: Center(
        child: Padding(
          padding: EdgeInsets.all(24),
          child: Text(
            'Muxport could not initialize local state. No remote command was '
            'sent. Restart the app or open diagnostics after recovery.',
            textAlign: TextAlign.center,
          ),
        ),
      ),
    );
  }
}

class _AppLockScreen extends StatelessWidget {
  const _AppLockScreen({required this.onUnlock});

  final Future<void> Function() onUnlock;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(Icons.lock_outline, size: 56),
              const SizedBox(height: 16),
              const Text(
                'Muxport is locked',
                style: TextStyle(fontSize: 22, fontWeight: FontWeight.bold),
              ),
              const SizedBox(height: 8),
              const Text(
                'Use your device authentication to view remote hosts and sessions.',
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 24),
              FilledButton.icon(
                icon: const Icon(Icons.lock_open),
                label: const Text('Unlock Muxport'),
                onPressed: () => unawaited(onUnlock()),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class MainNavigationScreen extends StatefulWidget {
  const MainNavigationScreen({required this.bootstrap, super.key});

  final AppBootstrapState bootstrap;

  @override
  State<MainNavigationScreen> createState() => _MainNavigationScreenState();
}

class _MainNavigationScreenState extends State<MainNavigationScreen>
    with WidgetsBindingObserver {
  int _currentIndex = 0;
  late Map<String, HostSyncState> _hosts;
  bool _pairingInProgress = false;
  bool _syncInProgress = false;
  bool _appLockEnabled = false;
  bool _appLocked = false;
  Timer? _syncTimer;
  bool _isForeground = true;
  final SensitiveInputRegistry _sensitiveInputs = SensitiveInputRegistry();
  final HostSyncOrchestrator _syncOrchestrator = const HostSyncOrchestrator();
  final StepUpAuthenticator _stepUpAuthenticator =
      PlatformStepUpAuthenticator();
  final AppLockPreference _appLockPreference = AppLockPreference();

  @override
  void initState() {
    super.initState();
    _hosts = Map.of(widget.bootstrap.cache.hosts);
    WidgetsBinding.instance.addObserver(this);
    _markReconnectableHostsReconnecting();
    unawaited(_loadAppLockPreference());
    WidgetsBinding.instance.addPostFrameCallback((_) {
      unawaited(_syncAllHosts());
    });
    _startSyncTimer();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _syncTimer?.cancel();
    _sensitiveInputs.dispose();
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    switch (state) {
      case AppLifecycleState.resumed:
        _isForeground = true;
        if (_appLockEnabled && mounted) {
          setState(() => _appLocked = true);
        }
        _markReconnectableHostsReconnecting(notify: true);
        _startSyncTimer();
        unawaited(_syncAllHosts());
        break;
      case AppLifecycleState.inactive:
      case AppLifecycleState.hidden:
      case AppLifecycleState.paused:
      case AppLifecycleState.detached:
        _isForeground = false;
        _syncTimer?.cancel();
        _syncTimer = null;
        _sensitiveInputs.clear();
        if (_appLockEnabled && mounted) {
          setState(() => _appLocked = true);
        }
        unawaited(_clearSensitiveArtifacts());
        unawaited(_persistBeforeSuspension());
        break;
    }
  }

  @override
  void didHaveMemoryPressure() {
    // Treat memory pressure like a privacy boundary as well as a durability
    // signal: no short-lived credential input or diagnostics artifact should
    // survive simply because the OS may soon terminate the process.
    _sensitiveInputs.clear();
    unawaited(_clearSensitiveArtifacts());
    unawaited(_persistCurrentCache());
  }

  void _startSyncTimer() {
    if (!_isForeground || _syncTimer?.isActive == true) {
      return;
    }
    _syncTimer = Timer.periodic(const Duration(seconds: 5), (_) {
      unawaited(_syncAllHosts());
    });
  }

  void _markReconnectableHostsReconnecting({bool notify = false}) {
    final nextHosts = MobileLifecycleReducer.prepareForeground(
      hosts: _hosts.values,
      canAuthenticateTransport: widget.bootstrap.canAuthenticateTransport,
    );
    if (notify && mounted) {
      setState(() {
        _hosts = nextHosts;
      });
    } else {
      _hosts = nextHosts;
    }
  }

  Future<void> _persistBeforeSuspension() async {
    final nextHosts = MobileLifecycleReducer.prepareSuspension(_hosts.values);
    if (mounted) {
      setState(() {
        _hosts = nextHosts;
      });
    } else {
      _hosts = nextHosts;
    }
    await _persistCurrentCache();
  }

  Future<void> _persistCurrentCache() async {
    final cacheStore = widget.bootstrap.cacheStore;
    if (cacheStore == null) {
      return;
    }
    await cacheStore.save(MobileCacheSnapshot(hosts: _hosts.values));
  }

  Future<void> _clearSensitiveArtifacts() async {
    try {
      await widget.bootstrap.sensitiveArtifactStore?.clear();
    } on Object {
      // Cleanup is best-effort during OS suspension. Startup repeats it before
      // any durable state is loaded.
    }
  }

  Future<void> _loadAppLockPreference() async {
    final enabled = await _appLockPreference.load();
    if (mounted) {
      setState(() => _appLockEnabled = enabled);
    }
  }

  Future<bool> _setAppLock(bool enabled) async {
    if (enabled) {
      final authorized = await _stepUpAuthenticator.authorize(
        reason: 'Authorize enabling Muxport app lock',
      );
      if (!authorized) return false;
    }
    final saved = await _appLockPreference.setEnabled(enabled);
    if (saved && mounted) {
      setState(() {
        _appLockEnabled = enabled;
        if (!enabled) _appLocked = false;
      });
    }
    return saved;
  }

  Future<void> _unlockApp() async {
    final authorized = await _stepUpAuthenticator.authorize(
      reason: 'Unlock Muxport',
    );
    if (authorized && mounted) {
      setState(() => _appLocked = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_appLocked) {
      return _AppLockScreen(onUnlock: _unlockApp);
    }
    final screens = [
      HostFleetScreen(
        hosts: _hosts.values.toList(growable: false),
        cacheStatus: widget.bootstrap.cacheStatus,
        identityStatus: widget.bootstrap.identityStatus,
        onPairHost: _canPairHost ? _pairHost : null,
      ),
      SessionTimelineScreen(
        hosts: _hosts.values,
        onStartSession: _canOperateSessions ? _startRemoteSession : null,
        onSendInput: _canOperateSessions ? _sendRemoteInput : null,
        onSteerSession: _canOperateSessions ? _steerRemoteSession : null,
        onInterruptSession: _canOperateSessions
            ? _interruptRemoteSession
            : null,
      ),
      ApprovalInboxScreen(
        hosts: _hosts.values,
        onDecision: widget.bootstrap.canAuthenticateTransport
            ? _respondToApproval
            : null,
      ),
      CredentialMatrixScreen(
        hosts: _hosts.values,
        onProvisionCredential: _canProvisionCredential
            ? _openCredentialProvisioning
            : null,
        onAssignCredential: _canAssignCredential
            ? _assignCredentialToRuntime
            : null,
        onRotateCredential: _canAssignCredential
            ? _rotateCredentialForRuntime
            : null,
        onBulkAssignCredentials: _canAssignCredential
            ? _bulkAssignCredentials
            : null,
        onRetryBulkSwitch: _canAssignCredential
            ? _retryBulkCredentialSwitch
            : null,
        onListRotationPools: _canAssignCredential ? _listRotationPools : null,
        onUpsertRotationPool: _canAssignCredential ? _upsertRotationPool : null,
      ),
      DiagnosticsScreen(
        bootstrap: widget.bootstrap,
        hosts: _hosts.values,
        onLoadCommandAudit: widget.bootstrap.canAuthenticateTransport
            ? _listCommandAudit
            : null,
        appLockEnabled: _appLockEnabled,
        onSetAppLock: _setAppLock,
      ),
    ];
    return Scaffold(
      body: IndexedStack(index: _currentIndex, children: screens),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _currentIndex,
        onDestinationSelected: (index) {
          setState(() {
            _currentIndex = index;
          });
        },
        destinations: const [
          NavigationDestination(icon: Icon(Icons.computer), label: 'Hosts'),
          NavigationDestination(icon: Icon(Icons.forum), label: 'Sessions'),
          NavigationDestination(icon: Icon(Icons.gavel), label: 'Approvals'),
          NavigationDestination(icon: Icon(Icons.key), label: 'Accounts'),
          NavigationDestination(
            icon: Icon(Icons.bug_report),
            label: 'Diagnostics',
          ),
        ],
      ),
    );
  }

  bool get _canPairHost =>
      !_pairingInProgress &&
      widget.bootstrap.canAuthenticateTransport &&
      widget.bootstrap.cacheStore != null;

  bool get _canProvisionCredential =>
      widget.bootstrap.identity != null &&
      widget.bootstrap.canAuthenticateTransport &&
      _hosts.values.any(
        (host) =>
            host.canMutate &&
            host.directAddress != null &&
            host.directPort != null,
      );

  bool get _canAssignCredential =>
      widget.bootstrap.identity != null &&
      widget.bootstrap.canAuthenticateTransport &&
      widget.bootstrap.cacheStore != null;

  bool get _canOperateSessions => _canAssignCredential;

  Future<void> _openCredentialProvisioning() async {
    final identity = widget.bootstrap.identity;
    if (identity == null || !_canProvisionCredential) {
      _showMessage(
        'Connect to an authenticated host before adding a credential.',
      );
      return;
    }
    await Navigator.of(context).push<void>(
      MaterialPageRoute(
        builder: (context) => CredentialProvisioningScreen(
          hosts: _hosts.values,
          identity: identity,
          sensitiveInputs: _sensitiveInputs,
          stepUpAuthenticator: _stepUpAuthenticator,
          onProvisioned: (host) async {
            await _syncAllHosts();
          },
        ),
      ),
    );
  }

  Future<void> _assignCredentialToRuntime(
    HostSyncState host,
    String runtimeId,
    String credentialProfileId,
  ) async {
    final identity = widget.bootstrap.identity;
    final cacheStore = widget.bootstrap.cacheStore;
    if (!host.canMutate ||
        identity == null ||
        cacheStore == null ||
        runtimeId.trim().isEmpty ||
        credentialProfileId.trim().isEmpty ||
        host.directAddress == null ||
        host.directPort == null) {
      _showMessage(
        'This assignment is not safely routable from current host state.',
      );
      return;
    }
    final authorized = await _stepUpAuthenticator.authorize(
      reason:
          'Authorize assigning a credential to $runtimeId on ${host.displayName}',
    );
    if (!authorized || !mounted) {
      if (mounted) {
        _showMessage(
          'Device authentication is required. No assignment was sent.',
        );
      }
      return;
    }

    final now = DateTime.now();
    final operationId =
        'assignment-${identity.deviceId}-${now.microsecondsSinceEpoch}';
    final pending = PendingOperation.local(
      idempotencyKey: operationId,
      kind: 'assignment:$runtimeId',
      createdAtMs: now.millisecondsSinceEpoch,
      deadlineMs: now.add(const Duration(seconds: 30)).millisecondsSinceEpoch,
    );
    final pendingHost = host.addPendingOperation(pending);
    await _persistHost(pendingHost, cacheStore);

    AuthenticatedDirectConnection? connection;
    try {
      connection = await AuthenticatedDirectConnection.connect(
        address: host.directAddress!,
        port: host.directPort!,
        pinnedHost: PinnedHostIdentity(
          hostId: host.hostId,
          publicKeyHex: host.pinnedHostKey,
        ),
        identity: identity,
      );
      final result = await connection.changeRuntimeAssignment(
        commandId: operationId,
        idempotencyKey: operationId,
        runtimeId: runtimeId,
        credentialProfileId: credentialProfileId,
      );
      final state =
          result.state.value >= 0 &&
              result.state.value < RemoteOpState.values.length
          ? RemoteOpState.values[result.state.value]
          : RemoteOpState.reconciliationRequired;
      final resolved = (_hosts[host.hostId] ?? pendingHost).resolveOperation(
        operationId,
        state,
      );
      await _persistHost(resolved, cacheStore);
      _showMessage(
        result.success
            ? 'Credential assigned to future work on $runtimeId.'
            : 'Connector did not confirm assignment: ${result.errorMessage}',
      );
      unawaited(_syncAllHosts());
    } on Object catch (error) {
      final current = _hosts[host.hostId] ?? pendingHost;
      final unresolved = current.resolveOperation(
        operationId,
        RemoteOpState.reconciliationRequired,
      );
      await _persistHost(unresolved, cacheStore);
      _showMessage(
        'Assignment outcome is unknown; verify host state before retrying: $error',
      );
    } finally {
      await connection?.close();
    }
  }

  Future<void> _bulkAssignCredentials(
    List<BulkCredentialSwitchTarget> targets, {
    String? originalGroupId,
    bool isRetry = false,
  }) async {
    final identity = widget.bootstrap.identity;
    final cacheStore = widget.bootstrap.cacheStore;
    if (targets.isEmpty || identity == null || cacheStore == null) {
      return;
    }
    final groupId =
        originalGroupId ??
        'switch-${identity.deviceId}-${DateTime.now().microsecondsSinceEpoch}';
    final authorized = await _stepUpAuthenticator.authorize(
      reason:
          '${isRetry ? 'Authorize retrying' : 'Authorize changing'} credentials '
          'for ${targets.length} remote runtimes',
    );
    if (!authorized || !mounted) {
      if (mounted) {
        _showMessage(
          'Device authentication is required. No changes were sent.',
        );
      }
      return;
    }

    var confirmed = 0;
    var rejected = 0;
    var unknown = 0;
    var unavailable = 0;
    for (var index = 0; index < targets.length; index += 1) {
      final target = targets[index];
      final outcome = await _dispatchBulkCredentialAssignment(
        target,
        identity,
        cacheStore,
        index,
        groupId,
      );
      switch (outcome) {
        case _BulkAssignmentOutcome.confirmed:
          confirmed += 1;
        case _BulkAssignmentOutcome.rejected:
          rejected += 1;
        case _BulkAssignmentOutcome.unknown:
          unknown += 1;
        case _BulkAssignmentOutcome.unavailable:
          unavailable += 1;
      }
    }
    if (mounted) {
      _showMessage(
        '${isRetry ? 'Bulk-switch retry' : 'Multi-host switch'}: '
        '$confirmed confirmed, $rejected rejected, '
        '$unknown need reconciliation, $unavailable unavailable. '
        'No global success is claimed until every selected runtime reconciles.',
      );
    }
    unawaited(_syncAllHosts());
  }

  Future<void> _retryBulkCredentialSwitch(BulkSwitchRetryGroup group) {
    return _bulkAssignCredentials(
      group.failedTargets,
      originalGroupId: group.groupId,
      isRetry: true,
    );
  }

  Future<_BulkAssignmentOutcome> _dispatchBulkCredentialAssignment(
    BulkCredentialSwitchTarget target,
    MobileDeviceIdentity identity,
    GenerationMobileCacheStore cacheStore,
    int index,
    String groupId,
  ) async {
    final host = _hosts[target.host.hostId];
    if (host == null ||
        !host.canMutate ||
        host.directAddress == null ||
        host.directPort == null ||
        target.runtimeId.trim().isEmpty ||
        target.credentialProfileId.trim().isEmpty) {
      return _BulkAssignmentOutcome.unavailable;
    }
    final now = DateTime.now();
    final operationId =
        'bulk-assignment-${identity.deviceId}-${now.microsecondsSinceEpoch}-$index';
    final pending = PendingOperation.local(
      idempotencyKey: operationId,
      kind: bulkSwitchOperationKind(
        groupId: groupId,
        runtimeId: target.runtimeId,
        credentialProfileId: target.credentialProfileId,
      ),
      createdAtMs: now.millisecondsSinceEpoch,
      deadlineMs: now.add(const Duration(seconds: 30)).millisecondsSinceEpoch,
    );
    final pendingHost = host.addPendingOperation(pending);
    await _persistHost(pendingHost, cacheStore);

    AuthenticatedDirectConnection? connection;
    try {
      connection = await AuthenticatedDirectConnection.connect(
        address: host.directAddress!,
        port: host.directPort!,
        pinnedHost: PinnedHostIdentity(
          hostId: host.hostId,
          publicKeyHex: host.pinnedHostKey,
        ),
        identity: identity,
      );
      final result = await connection.changeRuntimeAssignment(
        commandId: operationId,
        idempotencyKey: operationId,
        runtimeId: target.runtimeId,
        credentialProfileId: target.credentialProfileId,
      );
      final state =
          result.state.value >= 0 &&
              result.state.value < RemoteOpState.values.length
          ? RemoteOpState.values[result.state.value]
          : RemoteOpState.reconciliationRequired;
      final resolved = (_hosts[host.hostId] ?? pendingHost).resolveOperation(
        operationId,
        state,
      );
      await _persistHost(resolved, cacheStore);
      if (state == RemoteOpState.succeeded) {
        return _BulkAssignmentOutcome.confirmed;
      }
      if (state == RemoteOpState.failed ||
          state == RemoteOpState.expired ||
          state == RemoteOpState.cancelled ||
          state == RemoteOpState.rejectedOffline) {
        return _BulkAssignmentOutcome.rejected;
      }
      // A transport-level acknowledgement or a non-final command result is
      // deliberately not counted as a completed switch. The persisted
      // operation remains available for authoritative reconciliation.
      return _BulkAssignmentOutcome.unknown;
    } on Object {
      final current = _hosts[host.hostId] ?? pendingHost;
      final unresolved = current.resolveOperation(
        operationId,
        RemoteOpState.reconciliationRequired,
      );
      await _persistHost(unresolved, cacheStore);
      return _BulkAssignmentOutcome.unknown;
    } finally {
      await connection?.close();
    }
  }

  Future<void> _rotateCredentialForRuntime(
    HostSyncState host,
    String runtimeId,
    String rotationPoolId,
  ) async {
    final identity = widget.bootstrap.identity;
    final cacheStore = widget.bootstrap.cacheStore;
    if (!host.canMutate ||
        identity == null ||
        cacheStore == null ||
        runtimeId.trim().isEmpty ||
        rotationPoolId.trim().isEmpty ||
        host.directAddress == null ||
        host.directPort == null) {
      _showMessage(
        'This rotation is not safely routable from current host state.',
      );
      return;
    }
    final authorized = await _stepUpAuthenticator.authorize(
      reason:
          'Authorize credential rotation for $runtimeId on ${host.displayName}',
    );
    if (!authorized || !mounted) {
      if (mounted) {
        _showMessage(
          'Device authentication is required. No rotation was sent.',
        );
      }
      return;
    }

    final now = DateTime.now();
    final operationId =
        'rotation-${identity.deviceId}-${now.microsecondsSinceEpoch}';
    final pending = PendingOperation.local(
      idempotencyKey: operationId,
      kind: 'rotation:$runtimeId',
      createdAtMs: now.millisecondsSinceEpoch,
      deadlineMs: now.add(const Duration(seconds: 30)).millisecondsSinceEpoch,
    );
    final pendingHost = host.addPendingOperation(pending);
    await _persistHost(pendingHost, cacheStore);

    AuthenticatedDirectConnection? connection;
    try {
      connection = await AuthenticatedDirectConnection.connect(
        address: host.directAddress!,
        port: host.directPort!,
        pinnedHost: PinnedHostIdentity(
          hostId: host.hostId,
          publicKeyHex: host.pinnedHostKey,
        ),
        identity: identity,
      );
      final result = await connection.rotateCredential(
        commandId: operationId,
        idempotencyKey: operationId,
        rotationPoolId: rotationPoolId,
        runtimeId: runtimeId,
      );
      final state =
          result.state.value >= 0 &&
              result.state.value < RemoteOpState.values.length
          ? RemoteOpState.values[result.state.value]
          : RemoteOpState.reconciliationRequired;
      final resolved = (_hosts[host.hostId] ?? pendingHost).resolveOperation(
        operationId,
        state,
      );
      await _persistHost(resolved, cacheStore);
      _showMessage(
        result.success
            ? 'Credential rotation completed for future work on $runtimeId.'
            : 'Connector did not confirm rotation: ${result.errorMessage}',
      );
      unawaited(_syncAllHosts());
    } on Object catch (error) {
      final current = _hosts[host.hostId] ?? pendingHost;
      final unresolved = current.resolveOperation(
        operationId,
        RemoteOpState.reconciliationRequired,
      );
      await _persistHost(unresolved, cacheStore);
      _showMessage(
        'Rotation outcome is unknown; verify host state before retrying: $error',
      );
    } finally {
      await connection?.close();
    }
  }

  Future<List<RotationPoolSummary>> _listRotationPools(
    HostSyncState host,
  ) async {
    final identity = widget.bootstrap.identity;
    if (!host.canMutate ||
        identity == null ||
        host.directAddress == null ||
        host.directPort == null) {
      throw StateError('rotation pools are not safely routable for this host');
    }
    final commandId =
        'list-rotation-pools-${identity.deviceId}-${DateTime.now().microsecondsSinceEpoch}';
    AuthenticatedDirectConnection? connection;
    try {
      connection = await AuthenticatedDirectConnection.connect(
        address: host.directAddress!,
        port: host.directPort!,
        pinnedHost: PinnedHostIdentity(
          hostId: host.hostId,
          publicKeyHex: host.pinnedHostKey,
        ),
        identity: identity,
      );
      final result = await connection.listRotationPools(
        commandId: commandId,
        idempotencyKey: commandId,
      );
      if (!result.success ||
          result.state != wire.RemoteOpState.REMOTE_OP_STATE_SUCCEEDED) {
        throw StateError('connector did not confirm the pool list');
      }
      final decoded = jsonDecode(result.resultJson);
      if (decoded is! Map || decoded['pools'] is! List) {
        throw const FormatException('connector returned invalid pool metadata');
      }
      return [
        for (final raw in decoded['pools']! as List)
          if (raw is Map)
            RotationPoolSummary.fromJson(Map<String, Object?>.from(raw)),
      ];
    } finally {
      await connection?.close();
    }
  }

  Future<List<CommandAuditEntry>> _listCommandAudit(HostSyncState host) async {
    final identity = widget.bootstrap.identity;
    if (!host.canMutate ||
        identity == null ||
        host.directAddress == null ||
        host.directPort == null) {
      throw StateError('command audit is not safely routable for this host');
    }
    final commandId =
        'list-command-audit-${identity.deviceId}-${DateTime.now().microsecondsSinceEpoch}';
    AuthenticatedDirectConnection? connection;
    try {
      connection = await AuthenticatedDirectConnection.connect(
        address: host.directAddress!,
        port: host.directPort!,
        pinnedHost: PinnedHostIdentity(
          hostId: host.hostId,
          publicKeyHex: host.pinnedHostKey,
        ),
        identity: identity,
      );
      final result = await connection.listCommandAudit(
        commandId: commandId,
        idempotencyKey: commandId,
      );
      if (!result.success ||
          result.state != wire.RemoteOpState.REMOTE_OP_STATE_SUCCEEDED) {
        throw StateError('connector did not confirm the audit history');
      }
      final decoded = jsonDecode(result.resultJson);
      if (decoded is! Map || decoded['records'] is! List) {
        throw const FormatException('connector returned invalid audit history');
      }
      return [
        for (final raw in decoded['records']! as List)
          if (raw is Map)
            CommandAuditEntry.fromJson(Map<String, Object?>.from(raw)),
      ];
    } finally {
      await connection?.close();
    }
  }

  Future<void> _upsertRotationPool(
    HostSyncState host,
    RotationPoolDraft draft,
  ) async {
    final identity = widget.bootstrap.identity;
    final cacheStore = widget.bootstrap.cacheStore;
    if (!host.canMutate ||
        identity == null ||
        cacheStore == null ||
        host.directAddress == null ||
        host.directPort == null) {
      _showMessage('Rotation policy is not safely routable for this host.');
      return;
    }
    final authorized = await _stepUpAuthenticator.authorize(
      reason:
          'Authorize saving rotation policy ${draft.poolId} on ${host.displayName}',
    );
    if (!authorized || !mounted) {
      if (mounted) {
        _showMessage('Device authentication is required. No policy was saved.');
      }
      return;
    }
    final now = DateTime.now();
    final operationId =
        'rotation-policy-${identity.deviceId}-${now.microsecondsSinceEpoch}';
    final pending = PendingOperation.local(
      idempotencyKey: operationId,
      kind: 'rotation-policy:${draft.poolId}',
      createdAtMs: now.millisecondsSinceEpoch,
      deadlineMs: now.add(const Duration(seconds: 30)).millisecondsSinceEpoch,
    );
    final pendingHost = host.addPendingOperation(pending);
    await _persistHost(pendingHost, cacheStore);
    AuthenticatedDirectConnection? connection;
    try {
      connection = await AuthenticatedDirectConnection.connect(
        address: host.directAddress!,
        port: host.directPort!,
        pinnedHost: PinnedHostIdentity(
          hostId: host.hostId,
          publicKeyHex: host.pinnedHostKey,
        ),
        identity: identity,
      );
      final result = await connection.upsertRotationPool(
        commandId: operationId,
        idempotencyKey: operationId,
        poolId: draft.poolId,
        providerId: draft.providerId,
        orderedProfileIds: draft.orderedProfileIds,
        mode: draft.mode,
        cooldownMs: draft.cooldownMs,
        maxSwitchesPerHour: draft.maxSwitchesPerHour,
        allowedHostIds: draft.allowedHostIds,
        quotaFailoverEnabled: draft.quotaFailoverEnabled,
      );
      final state =
          result.state.value >= 0 &&
              result.state.value < RemoteOpState.values.length
          ? RemoteOpState.values[result.state.value]
          : RemoteOpState.reconciliationRequired;
      final resolved = (_hosts[host.hostId] ?? pendingHost).resolveOperation(
        operationId,
        state,
      );
      await _persistHost(resolved, cacheStore);
      _showMessage(
        state == RemoteOpState.succeeded
            ? 'Rotation policy saved. The connector will enforce its safeguards.'
            : 'Rotation policy was not confirmed; verify before retrying.',
      );
    } on Object {
      final current = _hosts[host.hostId] ?? pendingHost;
      await _persistHost(
        current.resolveOperation(
          operationId,
          RemoteOpState.reconciliationRequired,
        ),
        cacheStore,
      );
      _showMessage(
        'Policy outcome is unknown; reconcile the host before retrying.',
      );
    } finally {
      await connection?.close();
    }
  }

  Future<void> _startRemoteSession(
    HostSyncState host,
    String runtimeId,
    String projectPath,
    String prompt,
    String credentialProfileId,
  ) {
    return _dispatchSessionOperation(
      host: host,
      kind: 'start-session:$runtimeId',
      successMessage: 'Remote session started. Synchronizing its timeline now.',
      dispatch: (connection, operationId) => connection.startSession(
        commandId: operationId,
        idempotencyKey: operationId,
        runtimeId: runtimeId,
        projectPath: projectPath,
        prompt: prompt,
        credentialProfileId: credentialProfileId,
      ),
    );
  }

  Future<void> _sendRemoteInput(
    HostSyncState host,
    String runtimeId,
    String sessionId,
    String text,
  ) {
    return _dispatchSessionOperation(
      host: host,
      kind: 'send-input:$sessionId',
      successMessage: 'Input was sent to the remote session.',
      dispatch: (connection, operationId) => connection.sendInput(
        commandId: operationId,
        idempotencyKey: operationId,
        runtimeId: runtimeId,
        sessionId: sessionId,
        text: text,
      ),
    );
  }

  Future<void> _steerRemoteSession(
    HostSyncState host,
    String runtimeId,
    String sessionId,
    String instruction,
  ) {
    return _dispatchSessionOperation(
      host: host,
      kind: 'steer:$sessionId',
      successMessage: 'Steering instruction was sent to the remote session.',
      dispatch: (connection, operationId) => connection.steer(
        commandId: operationId,
        idempotencyKey: operationId,
        runtimeId: runtimeId,
        sessionId: sessionId,
        instruction: instruction,
      ),
    );
  }

  Future<void> _interruptRemoteSession(
    HostSyncState host,
    String runtimeId,
    String sessionId,
  ) {
    return _dispatchSessionOperation(
      host: host,
      kind: 'interrupt:$sessionId',
      successMessage:
          'Interrupt was sent. The connector will reconcile the turn.',
      dispatch: (connection, operationId) => connection.interrupt(
        commandId: operationId,
        idempotencyKey: operationId,
        runtimeId: runtimeId,
        sessionId: sessionId,
      ),
    );
  }

  Future<void> _dispatchSessionOperation({
    required HostSyncState host,
    required String kind,
    required String successMessage,
    required Future<wire.CommandResult> Function(
      AuthenticatedDirectConnection connection,
      String operationId,
    )
    dispatch,
  }) async {
    final identity = widget.bootstrap.identity;
    final cacheStore = widget.bootstrap.cacheStore;
    if (!host.canMutate ||
        identity == null ||
        cacheStore == null ||
        host.directAddress == null ||
        host.directPort == null) {
      _showMessage(
        'This session command is not safely routable from current host state.',
      );
      return;
    }
    final now = DateTime.now();
    final operationId =
        'session-${identity.deviceId}-${now.microsecondsSinceEpoch}';
    final pending = PendingOperation.local(
      idempotencyKey: operationId,
      kind: kind,
      createdAtMs: now.millisecondsSinceEpoch,
      deadlineMs: now.add(const Duration(seconds: 30)).millisecondsSinceEpoch,
    );
    final pendingHost = host.addPendingOperation(pending);
    await _persistHost(pendingHost, cacheStore);

    AuthenticatedDirectConnection? connection;
    try {
      connection = await AuthenticatedDirectConnection.connect(
        address: host.directAddress!,
        port: host.directPort!,
        pinnedHost: PinnedHostIdentity(
          hostId: host.hostId,
          publicKeyHex: host.pinnedHostKey,
        ),
        identity: identity,
      );
      final result = await dispatch(connection, operationId);
      final state =
          result.state.value >= 0 &&
              result.state.value < RemoteOpState.values.length
          ? RemoteOpState.values[result.state.value]
          : RemoteOpState.reconciliationRequired;
      final resolved = (_hosts[host.hostId] ?? pendingHost).resolveOperation(
        operationId,
        state,
      );
      await _persistHost(resolved, cacheStore);
      _showMessage(
        result.success
            ? successMessage
            : 'Connector did not confirm this session command: ${result.errorMessage}',
      );
      unawaited(_syncAllHosts());
    } on Object catch (error) {
      final current = _hosts[host.hostId] ?? pendingHost;
      final unresolved = current.resolveOperation(
        operationId,
        RemoteOpState.reconciliationRequired,
      );
      await _persistHost(unresolved, cacheStore);
      _showMessage(
        'Session command outcome is unknown; verify host state before retrying: $error',
      );
    } finally {
      await connection?.close();
    }
  }

  Future<void> _pairHost() async {
    final identity = widget.bootstrap.identity;
    final cacheStore = widget.bootstrap.cacheStore;
    if (identity == null || cacheStore == null || !_canPairHost) {
      return;
    }
    final encoded = await _showPairingCodeDialog();
    if (!mounted || encoded == null) {
      return;
    }
    setState(() {
      _pairingInProgress = true;
    });
    PendingDirectPairing? pairing;
    try {
      final offer = await SignedPairingOffer.parseAndVerify(
        encoded,
        nowMs: DateTime.now().millisecondsSinceEpoch,
      );
      final existing = _hosts[offer.hostId];
      if (existing != null &&
          existing.pinnedHostKey != offer.hostIdentityPublicKeyHex) {
        throw const DirectTransportProtocolException(
          'this host id is already pinned to a different identity',
        );
      }
      pairing = await PendingDirectPairing.connect(
        offer: offer,
        identity: identity,
        deviceName: 'Muxport mobile',
      );
      if (!mounted) {
        await pairing.close();
        return;
      }
      final confirmed = await _showSasConfirmationDialog(
        hostname: offer.hostname,
        sas: pairing.sas,
      );
      if (!mounted || confirmed != true) {
        await pairing.close();
        return;
      }
      final enrollment = await pairing.confirm();
      pairing = null;
      final enrolledHost = HostSyncState(
        hostId: enrollment.hostId,
        pinnedHostKey: enrollment.hostIdentityPublicKeyHex,
        displayName: enrollment.hostname,
        protocolVersion: mobileProtocolVersion,
        phase: enrollment.awaitingHostConfirmation
            ? HostSyncPhase.pairingPending
            : HostSyncPhase.cachedStale,
        directAddress: enrollment.address,
        directPort: enrollment.port,
        pairingPending: enrollment.awaitingHostConfirmation,
        snapshot: const {},
        cursor: null,
        sourceVersions: const {},
        recentEventIds: const [],
        pendingOperations: const {},
      );
      final nextHosts = {..._hosts, enrolledHost.hostId: enrolledHost};
      await cacheStore.save(MobileCacheSnapshot(hosts: nextHosts.values));
      if (!mounted) {
        return;
      }
      setState(() {
        _hosts = nextHosts;
      });
      _showMessage(
        enrollment.awaitingHostConfirmation
            ? 'Phone confirmed. Confirm the same SAS on the host to finish.'
            : 'Host paired. State sync will begin when the connector stream is available.',
      );
      unawaited(_syncAllHosts());
    } on Object catch (error) {
      await pairing?.close();
      if (mounted) {
        _showMessage('Pairing failed safely: $error');
      }
    } finally {
      if (mounted) {
        setState(() {
          _pairingInProgress = false;
        });
      }
    }
  }

  Future<String?> _showPairingCodeDialog() async {
    final controller = _sensitiveInputs.createController();
    try {
      return await showDialog<String>(
        context: context,
        builder: (context) => AlertDialog(
          title: const Text('Pair a host'),
          content: TextField(
            controller: controller,
            autofocus: true,
            minLines: 4,
            maxLines: 10,
            decoration: const InputDecoration(
              labelText: 'Signed pairing code',
              hintText: 'Paste the JSON pairing code from the host',
              border: OutlineInputBorder(),
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () {
                final value = controller.text.trim();
                if (value.isNotEmpty) {
                  Navigator.pop(context, value);
                }
              },
              child: const Text('Connect'),
            ),
          ],
        ),
      );
    } finally {
      _sensitiveInputs.release(controller);
    }
  }

  Future<bool?> _showSasConfirmationDialog({
    required String hostname,
    required String sas,
  }) {
    return showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (context) => AlertDialog(
        title: const Text('Compare security code'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              'Check that this code exactly matches the trusted display on $hostname.',
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 20),
            SelectableText(
              sas,
              style: Theme.of(context).textTheme.displaySmall?.copyWith(
                letterSpacing: 8,
                fontWeight: FontWeight.bold,
              ),
            ),
            const SizedBox(height: 12),
            const Text(
              'Do not continue if the codes differ. This confirmation window closes after two minutes.',
              textAlign: TextAlign.center,
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Codes differ'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Codes match'),
          ),
        ],
      ),
    );
  }

  void _showMessage(String message) {
    ScaffoldMessenger.of(
      context,
    ).showSnackBar(SnackBar(content: Text(message)));
  }

  Future<void> _syncAllHosts() async {
    final identity = widget.bootstrap.identity;
    final cacheStore = widget.bootstrap.cacheStore;
    if (!mounted ||
        !_isForeground ||
        _syncInProgress ||
        identity == null ||
        cacheStore == null ||
        !widget.bootstrap.canAuthenticateTransport) {
      return;
    }
    _syncInProgress = true;
    try {
      for (final hostId in _hosts.keys.toList(growable: false)) {
        final host = _hosts[hostId];
        if (host == null || !host.canReconnect) {
          continue;
        }
        try {
          final synchronized = await _syncOrchestrator.synchronizeOnce(
            state: host,
            identity: identity,
            cacheStore: cacheStore,
            allHosts: _hosts.values.toList(growable: false),
          );
          final reconciled = await _reconcilePendingOperations(
            synchronized,
            identity,
            cacheStore,
          );
          if (!mounted) {
            return;
          }
          setState(() {
            _hosts = {..._hosts, hostId: reconciled};
          });
        } on Object {
          if (!mounted) {
            return;
          }
          final current = _hosts[hostId];
          if (current != null &&
              current.phase != HostSyncPhase.pairingPending) {
            setState(() {
              _hosts = {
                ..._hosts,
                hostId: current.withPhase(HostSyncPhase.offline),
              };
            });
          }
        }
      }
    } finally {
      _syncInProgress = false;
    }
  }

  Future<void> _respondToApproval(
    HostSyncState host,
    Map<String, Object?> event,
    bool approved,
  ) async {
    final identity = widget.bootstrap.identity;
    final cacheStore = widget.bootstrap.cacheStore;
    final approvalId = event['approvalId'];
    final sessionId = event['sessionId'];
    final runtimeId = _runtimeForSession(host, sessionId);
    if (!host.canMutate ||
        identity == null ||
        cacheStore == null ||
        approvalId is! String ||
        approvalId.isEmpty ||
        sessionId is! String ||
        sessionId.isEmpty ||
        runtimeId == null ||
        host.directAddress == null ||
        host.directPort == null) {
      _showMessage('Approval is not safely routable from current host state.');
      return;
    }

    final now = DateTime.now();
    final operationId =
        'approval-${identity.deviceId}-${now.microsecondsSinceEpoch}';
    final pending = PendingOperation.local(
      idempotencyKey: operationId,
      kind: 'approval:$approvalId',
      createdAtMs: now.millisecondsSinceEpoch,
      deadlineMs: now.add(const Duration(seconds: 30)).millisecondsSinceEpoch,
    );
    final pendingHost = host.addPendingOperation(pending);
    await _persistHost(pendingHost, cacheStore);

    AuthenticatedDirectConnection? connection;
    try {
      connection = await AuthenticatedDirectConnection.connect(
        address: host.directAddress!,
        port: host.directPort!,
        pinnedHost: PinnedHostIdentity(
          hostId: host.hostId,
          publicKeyHex: host.pinnedHostKey,
        ),
        identity: identity,
      );
      final result = await connection.respondToApproval(
        commandId: operationId,
        idempotencyKey: operationId,
        runtimeId: runtimeId,
        sessionId: sessionId,
        approvalId: approvalId,
        approved: approved,
        decisionReason: approved
            ? 'Approved from paired Muxport device'
            : 'Rejected from paired Muxport device',
      );
      final state =
          result.state.value >= 0 &&
              result.state.value < RemoteOpState.values.length
          ? RemoteOpState.values[result.state.value]
          : RemoteOpState.reconciliationRequired;
      final resolved = (_hosts[host.hostId] ?? pendingHost).resolveOperation(
        operationId,
        state,
      );
      await _persistHost(resolved, cacheStore);
      _showMessage(
        result.success
            ? (approved ? 'Approval accepted.' : 'Approval rejected.')
            : 'Connector did not confirm the decision: ${result.errorMessage}',
      );
      unawaited(_syncAllHosts());
    } on Object catch (error) {
      final current = _hosts[host.hostId] ?? pendingHost;
      final unresolved = current.resolveOperation(
        operationId,
        RemoteOpState.reconciliationRequired,
      );
      await _persistHost(unresolved, cacheStore);
      _showMessage(
        'Approval outcome is unknown; verify source state before retrying: $error',
      );
    } finally {
      await connection?.close();
    }
  }

  Future<HostSyncState> _reconcilePendingOperations(
    HostSyncState host,
    MobileDeviceIdentity identity,
    GenerationMobileCacheStore cacheStore,
  ) async {
    final operationIds = host.operationIdsRequiringStatusQuery;
    if (operationIds.isEmpty ||
        host.directAddress == null ||
        host.directPort == null) {
      return host;
    }
    final connection = await AuthenticatedDirectConnection.connect(
      address: host.directAddress!,
      port: host.directPort!,
      pinnedHost: PinnedHostIdentity(
        hostId: host.hostId,
        publicKeyHex: host.pinnedHostKey,
      ),
      identity: identity,
    );
    var current = host;
    try {
      for (var index = 0; index < operationIds.length; index += 1) {
        final operationId = operationIds[index];
        final queryId =
            'query-${identity.deviceId}-${DateTime.now().microsecondsSinceEpoch}-$index';
        final result = await connection.queryOperation(
          commandId: queryId,
          idempotencyKey: queryId,
          targetIdempotencyKey: operationId,
        );
        if (!result.success) {
          current = current.resolveOperation(
            operationId,
            RemoteOpState.reconciliationRequired,
          );
          continue;
        }
        final decoded = jsonDecode(result.resultJson);
        if (decoded is! Map || decoded['state'] is! int) {
          current = current.resolveOperation(
            operationId,
            RemoteOpState.reconciliationRequired,
          );
          continue;
        }
        final stateCode = decoded['state']! as int;
        final remoteState =
            stateCode >= 0 && stateCode < RemoteOpState.values.length
            ? RemoteOpState.values[stateCode]
            : RemoteOpState.reconciliationRequired;
        current = current.resolveOperation(operationId, remoteState);
      }
      await cacheStore.save(
        MobileCacheSnapshot(
          hosts: [
            for (final cachedHost in _hosts.values)
              if (cachedHost.hostId == current.hostId) current else cachedHost,
          ],
        ),
      );
      return current;
    } finally {
      await connection.close();
    }
  }

  String? _runtimeForSession(HostSyncState host, Object? sessionId) {
    if (sessionId is! String) {
      return null;
    }
    final sessions = host.snapshot['activeSessions'] as List? ?? const [];
    for (final value in sessions) {
      if (value is! Map) {
        continue;
      }
      final session = Map<String, Object?>.from(value);
      if (session['sessionId'] == sessionId) {
        final runtimeId = session['runtimeId'];
        return runtimeId is String && runtimeId.isNotEmpty ? runtimeId : null;
      }
    }
    return null;
  }

  Future<void> _persistHost(
    HostSyncState host,
    GenerationMobileCacheStore cacheStore,
  ) async {
    final nextHosts = {..._hosts, host.hostId: host};
    await cacheStore.save(MobileCacheSnapshot(hosts: nextHosts.values));
    if (mounted) {
      setState(() {
        _hosts = nextHosts;
      });
    }
  }
}
