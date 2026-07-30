import 'dart:async';

import 'package:flutter/material.dart';

import 'screens/host_fleet_screen.dart';
import 'screens/session_timeline_screen.dart';
import 'screens/approval_inbox_screen.dart';
import 'screens/credential_matrix_screen.dart';
import 'screens/diagnostics_screen.dart';
import 'state/app_bootstrap.dart';

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

class _MainNavigationScreenState extends State<MainNavigationScreen> {
  int _currentIndex = 0;

  @override
  Widget build(BuildContext context) {
    final screens = [
      HostFleetScreen(
        hosts: widget.bootstrap.cache.hosts.values.toList(growable: false),
        cacheStatus: widget.bootstrap.cacheStatus,
        identityStatus: widget.bootstrap.identityStatus,
      ),
      const SessionTimelineScreen(),
      const ApprovalInboxScreen(),
      const CredentialMatrixScreen(),
      DiagnosticsScreen(bootstrap: widget.bootstrap),
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
}
