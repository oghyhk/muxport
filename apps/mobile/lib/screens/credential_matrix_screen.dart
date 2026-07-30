import 'package:flutter/material.dart';

class CredentialMatrixScreen extends StatelessWidget {
  const CredentialMatrixScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Account Profiles & Rotation'),
        actions: [
          IconButton(
            icon: const Icon(Icons.add),
            onPressed: () {},
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16.0),
        children: [
          _buildProfileTile(
            name: 'opencode-go-profile-1',
            provider: 'OpenCode Go',
            fingerprint: 'sha256:a1b2c3d4...',
            status: 'Active',
            statusColor: Colors.green,
          ),
          _buildProfileTile(
            name: 'opencode-go-profile-2',
            provider: 'OpenCode Go',
            fingerprint: 'sha256:e5f6g7h8...',
            status: 'Staged / Cooling Down',
            statusColor: Colors.orange,
          ),
          _buildProfileTile(
            name: 'codex-work-api-key',
            provider: 'Codex / OpenAI',
            fingerprint: 'sha256:99887766...',
            status: 'Active',
            statusColor: Colors.green,
          ),
          const SizedBox(height: 16),
          ElevatedButton.icon(
            icon: const Icon(Icons.sync_alt),
            label: const Text('One-Tap Bulk Identity Switch'),
            style: ElevatedButton.styleFrom(
              padding: const EdgeInsets.all(16),
              backgroundColor: Colors.purple,
            ),
            onPressed: () {},
          ),
        ],
      ),
    );
  }

  Widget _buildProfileTile({
    required String name,
    required String provider,
    required String fingerprint,
    required String status,
    required Color statusColor,
  }) {
    return Card(
      child: ListTile(
        leading: const Icon(Icons.vpn_key, color: Colors.purpleAccent),
        title: Text(name, style: const TextStyle(fontWeight: FontWeight.bold)),
        subtitle: Text('$provider • $fingerprint'),
        trailing: Container(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
          decoration: BoxDecoration(
            color: statusColor.withValues(alpha: 0.2),
            borderRadius: BorderRadius.circular(4),
          ),
          child: Text(
            status,
            style: TextStyle(color: statusColor, fontSize: 10),
          ),
        ),
      ),
    );
  }
}
