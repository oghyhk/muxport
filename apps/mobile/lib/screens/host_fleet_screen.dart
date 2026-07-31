import 'package:flutter/material.dart';

import '../state/app_bootstrap.dart';
import '../state/mobile_sync_state.dart';

class HostFleetScreen extends StatelessWidget {
  const HostFleetScreen({
    required this.hosts,
    required this.cacheStatus,
    required this.identityStatus,
    required this.onPairHost,
    super.key,
  });

  final List<HostSyncState> hosts;
  final CacheBootstrapStatus cacheStatus;
  final IdentityBootstrapStatus identityStatus;
  final VoidCallback? onPairHost;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Host Fleet'),
        actions: [
          IconButton(
            tooltip: onPairHost == null
                ? 'Pairing is unavailable until protected local state is ready'
                : 'Pair a host from a signed pairing code',
            icon: const Icon(Icons.qr_code_scanner),
            onPressed: onPairHost,
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          if (cacheStatus == CacheBootstrapStatus.corrupt)
            const _WarningCard(
              icon: Icons.storage,
              message:
                  'The local host cache is corrupt. Existing generations were '
                  'preserved and remote mutations are blocked.',
            ),
          if (cacheStatus == CacheBootstrapStatus.unavailable)
            const _WarningCard(
              icon: Icons.storage,
              message:
                  'The local host cache is unavailable. Muxport did not assume '
                  'that any cached host state is current.',
            ),
          if (cacheStatus == CacheBootstrapStatus.recoveredPreviousGeneration)
            const _WarningCard(
              icon: Icons.restore,
              message:
                  'Muxport recovered the previous verified cache generation. '
                  'Hosts remain stale until connector replay completes.',
            ),
          if (identityStatus == IdentityBootstrapStatus.unavailable)
            const _WarningCard(
              icon: Icons.phonelink_lock,
              message:
                  'The protected phone identity is locked or unavailable. '
                  'Pairing, reconnect, and remote commands are disabled.',
            ),
          if (hosts.isEmpty)
            _EmptyHostCard(
              cacheBlocked:
                  cacheStatus == CacheBootstrapStatus.corrupt ||
                  cacheStatus == CacheBootstrapStatus.unavailable,
            )
          else
            for (final host in hosts) ...[
              _HostCard(host: host),
              const SizedBox(height: 12),
            ],
        ],
      ),
    );
  }
}

class _WarningCard extends StatelessWidget {
  const _WarningCard({required this.icon, required this.message});

  final IconData icon;
  final String message;

  @override
  Widget build(BuildContext context) {
    return Card(
      color: Theme.of(context).colorScheme.errorContainer,
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(icon, color: Theme.of(context).colorScheme.onErrorContainer),
            const SizedBox(width: 12),
            Expanded(
              child: Text(
                message,
                style: TextStyle(
                  color: Theme.of(context).colorScheme.onErrorContainer,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _EmptyHostCard extends StatelessWidget {
  const _EmptyHostCard({required this.cacheBlocked});

  final bool cacheBlocked;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          children: [
            const Icon(Icons.devices_other, size: 40),
            const SizedBox(height: 12),
            Text(
              cacheBlocked ? 'Host cache unavailable' : 'No cached hosts',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 8),
            Text(
              cacheBlocked
                  ? 'Recover the local cache before pairing or controlling a '
                        'host.'
                  : 'Pairing will appear here after the authenticated mobile '
                        'transport is connected.',
              textAlign: TextAlign.center,
            ),
          ],
        ),
      ),
    );
  }
}

class _HostCard extends StatelessWidget {
  const _HostCard({required this.host});

  final HostSyncState host;

  @override
  Widget build(BuildContext context) {
    final cursor = host.cursor;
    final pendingCount = host.operationIdsRequiringStatusQuery.length;
    final statusColor = _statusColor(host.phase);

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(Icons.dns, color: statusColor),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    host.displayName,
                    style: Theme.of(context).textTheme.titleMedium?.copyWith(
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                ),
                _StatusChip(
                  label: _statusLabel(host.phase),
                  color: statusColor,
                ),
              ],
            ),
            const Divider(height: 24),
            Text('Host ID: ${host.hostId}'),
            if (host.canReconnect) ...[
              const SizedBox(height: 4),
              Text('Direct endpoint: ${host.directAddress}:${host.directPort}'),
            ],
            const SizedBox(height: 4),
            Text(
              cursor == null
                  ? 'No acknowledged replay cursor'
                  : 'Cached cursor: ${cursor.hostEpoch} / ${cursor.sequence}',
            ),
            const SizedBox(height: 4),
            Text(
              pendingCount == 0
                  ? 'No locally pending operations'
                  : '$pendingCount operation(s) require reconciliation',
            ),
            const SizedBox(height: 12),
            const Text(
              'Remote controls remain disabled until host identity '
              'verification and replay or snapshot recovery completes.',
            ),
          ],
        ),
      ),
    );
  }

  static String _statusLabel(HostSyncPhase phase) {
    return switch (phase) {
      HostSyncPhase.pairingPending => 'Awaiting host confirmation',
      HostSyncPhase.cachedStale => 'Cached • stale',
      HostSyncPhase.reconnecting => 'Reconnecting',
      HostSyncPhase.replaying => 'Replaying',
      HostSyncPhase.snapshotRequired => 'Snapshot required',
      HostSyncPhase.synchronized => 'Synchronized',
      HostSyncPhase.offline => 'Offline',
      HostSyncPhase.identityMismatch => 'Identity mismatch',
      HostSyncPhase.protocolIncompatible => 'Upgrade required',
    };
  }

  static Color _statusColor(HostSyncPhase phase) {
    return switch (phase) {
      HostSyncPhase.pairingPending => Colors.blue,
      HostSyncPhase.synchronized => Colors.green,
      HostSyncPhase.reconnecting ||
      HostSyncPhase.replaying ||
      HostSyncPhase.cachedStale => Colors.orange,
      HostSyncPhase.snapshotRequired ||
      HostSyncPhase.offline ||
      HostSyncPhase.protocolIncompatible => Colors.amber,
      HostSyncPhase.identityMismatch => Colors.red,
    };
  }
}

class _StatusChip extends StatelessWidget {
  const _StatusChip({required this.label, required this.color});

  final String label;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.2),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(label, style: TextStyle(color: color, fontSize: 12)),
    );
  }
}
