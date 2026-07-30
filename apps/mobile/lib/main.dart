import 'package:flutter/material.dart';
import 'screens/host_fleet_screen.dart';
import 'screens/session_timeline_screen.dart';
import 'screens/approval_inbox_screen.dart';
import 'screens/credential_matrix_screen.dart';
import 'screens/diagnostics_screen.dart';

void main() {
  runApp(const MuxportApp());
}

class MuxportApp extends StatelessWidget {
  const MuxportApp({super.key});

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
      home: const MainNavigationScreen(),
      debugShowCheckedModeBanner: false,
    );
  }
}

class MainNavigationScreen extends StatefulWidget {
  const MainNavigationScreen({super.key});

  @override
  State<MainNavigationScreen> createState() => _MainNavigationScreenState();
}

class _MainNavigationScreenState extends State<MainNavigationScreen> {
  int _currentIndex = 0;

  final List<Widget> _screens = const [
    HostFleetScreen(),
    SessionTimelineScreen(),
    ApprovalInboxScreen(),
    CredentialMatrixScreen(),
    DiagnosticsScreen(),
  ];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: IndexedStack(
        index: _currentIndex,
        children: _screens,
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _currentIndex,
        onDestinationSelected: (index) {
          setState(() {
            _currentIndex = index;
          });
        },
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.computer),
            label: 'Hosts',
          ),
          NavigationDestination(
            icon: Icon(Icons.forum),
            label: 'Sessions',
          ),
          NavigationDestination(
            icon: Icon(Icons.gavel),
            label: 'Approvals',
          ),
          NavigationDestination(
            icon: Icon(Icons.key),
            label: 'Accounts',
          ),
          NavigationDestination(
            icon: Icon(Icons.bug_report),
            label: 'Diagnostics',
          ),
        ],
      ),
    );
  }
}
