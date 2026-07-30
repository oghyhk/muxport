import 'package:flutter/material.dart';

class HostFleetScreen extends StatelessWidget {
  const HostFleetScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Host Fleet'),
        actions: [
          IconButton(
            icon: const Icon(Icons.qr_code_scanner),
            onPressed: () {
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('Opening QR Host Pairing...')),
              );
            },
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16.0),
        children: [
          _buildHostCard(
            context,
            hostname: 'vps-malaysia-01',
            status: 'Ready (E2EE Active)',
            statusColor: Colors.green,
            runtimes: ['OpenCode Go (prod)', 'Codex App Server (dev)'],
            activeProfile: 'opencode-go-pool-a',
          ),
          const SizedBox(height: 12),
          _buildHostCard(
            context,
            hostname: 'macbook-pro-m2',
            status: 'Offline (Last seen 10m ago)',
            statusColor: Colors.orange,
            runtimes: ['Codex Desktop', 'OpenCode Local'],
            activeProfile: 'codex-work-api-key',
          ),
        ],
      ),
    );
  }

  Widget _buildHostCard(
    BuildContext context, {
    required String hostname,
    required String status,
    required Color statusColor,
    required List<String> runtimes,
    required String activeProfile,
  }) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(Icons.dns, color: statusColor),
                const SizedBox(width: 8),
                Text(
                  hostname,
                  style: Theme.of(context).textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.bold,
                      ),
                ),
                const Spacer(),
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                  decoration: BoxDecoration(
                    color: statusColor.withOpacity(0.2),
                    borderRadius: BorderRadius.circular(4),
                  ),
                  child: Text(
                    status,
                    style: TextStyle(color: statusColor, fontSize: 12),
                  ),
                ),
              ],
            ),
            const Divider(height: 24),
            Text('Managed Runtimes:', style: TextStyle(color: Colors.grey[400])),
            const SizedBox(height: 4),
            ...runtimes.map((r) => Padding(
                  padding: const EdgeInsets.only(left: 8.0, top: 2.0),
                  child: Row(
                    children: [
                      const Icon(Icons.play_arrow, size: 14, color: Colors.blue),
                      const SizedBox(width: 4),
                      Text(r),
                    ],
                  ),
                )),
            const SizedBox(height: 8),
            Row(
              children: [
                const Icon(Icons.vpn_key, size: 14, color: Colors.purpleAccent),
                const SizedBox(width: 4),
                Text('Active Profile: $activeProfile', style: const TextStyle(fontSize: 12)),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
