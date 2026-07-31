import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_protocol/flutter_protocol.dart';

import 'pairing/signed_pairing_offer.dart';
import 'screens/host_fleet_screen.dart';
import 'screens/session_timeline_screen.dart';
import 'screens/approval_inbox_screen.dart';
import 'screens/credential_matrix_screen.dart';
import 'screens/diagnostics_screen.dart';
import 'state/app_bootstrap.dart';
import 'state/host_sync_orchestrator.dart';
import 'state/mobile_cache_store.dart';
import 'state/mobile_lifecycle.dart';
import 'state/mobile_sync_state.dart';
import 'transport/direct_transport_client.dart';
import 'transport/direct_transport_protocol.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const MuxportApp());
}

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
  Timer? _syncTimer;
  bool _isForeground = true;
  final HostSyncOrchestrator _syncOrchestrator = const HostSyncOrchestrator();

  @override
  void initState() {
    super.initState();
    _hosts = Map.of(widget.bootstrap.cache.hosts);
    WidgetsBinding.instance.addObserver(this);
    _markReconnectableHostsReconnecting();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      unawaited(_syncAllHosts());
    });
    _startSyncTimer();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _syncTimer?.cancel();
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    switch (state) {
      case AppLifecycleState.resumed:
        _isForeground = true;
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
        unawaited(_persistBeforeSuspension());
        break;
    }
  }

  @override
  void didHaveMemoryPressure() {
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

  @override
  Widget build(BuildContext context) {
    final screens = [
      HostFleetScreen(
        hosts: _hosts.values.toList(growable: false),
        cacheStatus: widget.bootstrap.cacheStatus,
        identityStatus: widget.bootstrap.identityStatus,
        onPairHost: _canPairHost ? _pairHost : null,
      ),
      SessionTimelineScreen(hosts: _hosts.values),
      ApprovalInboxScreen(
        hosts: _hosts.values,
        onDecision: widget.bootstrap.canAuthenticateTransport
            ? _respondToApproval
            : null,
      ),
      CredentialMatrixScreen(hosts: _hosts.values),
      DiagnosticsScreen(bootstrap: widget.bootstrap, hosts: _hosts.values),
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

  Future<String?> _showPairingCodeDialog() {
    final controller = TextEditingController();
    return showDialog<String>(
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
    ).whenComplete(controller.dispose);
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
          if (!mounted) {
            return;
          }
          setState(() {
            _hosts = {..._hosts, hostId: synchronized};
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
