import 'package:flutter/material.dart';

import '../diagnostics/redacted_diagnostic_export.dart';
import '../state/app_bootstrap.dart';
import '../state/mobile_sync_state.dart';

class DiagnosticsScreen extends StatelessWidget {
  const DiagnosticsScreen({
    required this.bootstrap,
    required this.hosts,
    super.key,
  });

  final AppBootstrapState bootstrap;
  final Iterable<HostSyncState> hosts;

  @override
  Widget build(BuildContext context) {
    final identityReady =
        bootstrap.identityStatus == IdentityBootstrapStatus.ready;
    final syncHealthy = hosts.any(
      (host) => host.phase == HostSyncPhase.synchronized,
    );

    return Scaffold(
      appBar: AppBar(title: const Text('Diagnostics & Audit Log')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text(
                    'Local startup state',
                    style: TextStyle(fontWeight: FontWeight.bold, fontSize: 16),
                  ),
                  const SizedBox(height: 12),
                  _HealthRow(
                    title: 'Host cache',
                    value: _cacheLabel(bootstrap.cacheStatus),
                    healthy:
                        bootstrap.cacheStatus == CacheBootstrapStatus.ready ||
                        bootstrap.cacheStatus ==
                            CacheBootstrapStatus.recoveredPreviousGeneration,
                  ),
                  _HealthRow(
                    title: 'Cache generation',
                    value: bootstrap.cacheGeneration.toString(),
                    healthy: true,
                  ),
                  _HealthRow(
                    title: 'Protected phone identity',
                    value: identityReady ? 'AVAILABLE' : 'LOCKED / UNAVAILABLE',
                    healthy: identityReady,
                  ),
                  _HealthRow(
                    title: 'Connector transport',
                    value: syncHealthy
                        ? 'AUTHENTICATED POLLING'
                        : 'NOT CONNECTED',
                    healthy: syncHealthy,
                  ),
                  const _HealthRow(
                    title: 'Secret leak audit',
                    value: 'NOT RUN',
                    healthy: false,
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          FilledButton.icon(
            icon: const Icon(Icons.download),
            label: const Text('Export redacted diagnostics'),
            onPressed: bootstrap.sensitiveArtifactStore == null
                ? null
                : () => _confirmAndExport(context),
          ),
        ],
      ),
    );
  }

  Future<void> _confirmAndExport(BuildContext context) async {
    final store = bootstrap.sensitiveArtifactStore;
    if (store == null) {
      return;
    }
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: const Text('Export redacted diagnostics?'),
        content: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text('Included files:'),
              for (final file in redactedDiagnosticFileNames) Text('• $file'),
              const SizedBox(height: 12),
              const Text('Always excluded:'),
              for (final exclusion in redactedDiagnosticExclusions)
                Text('• $exclusion'),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(dialogContext, false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(dialogContext, true),
            child: const Text('Create export'),
          ),
        ],
      ),
    );
    if (confirmed != true || !context.mounted) {
      return;
    }
    try {
      final export = await RedactedDiagnosticExporter(
        store,
      ).create(bootstrap: bootstrap, hosts: hosts);
      if (!context.mounted) {
        return;
      }
      await showDialog<void>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: const Text('Diagnostics export created'),
          content: SelectableText(export.directory.path),
          actions: [
            FilledButton(
              onPressed: () => Navigator.pop(dialogContext),
              child: const Text('Done'),
            ),
          ],
        ),
      );
    } on Object {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Diagnostics export failed without deleting cache.'),
          ),
        );
      }
    }
  }

  static String _cacheLabel(CacheBootstrapStatus status) {
    return switch (status) {
      CacheBootstrapStatus.ready => 'READY',
      CacheBootstrapStatus.recoveredPreviousGeneration =>
        'RECOVERED PREVIOUS GENERATION',
      CacheBootstrapStatus.corrupt => 'CORRUPT / WRITE BLOCKED',
      CacheBootstrapStatus.unavailable => 'UNAVAILABLE',
    };
  }
}

class _HealthRow extends StatelessWidget {
  const _HealthRow({
    required this.title,
    required this.value,
    required this.healthy,
  });

  final String title;
  final String value;
  final bool healthy;

  @override
  Widget build(BuildContext context) {
    final color = healthy ? Colors.green : Colors.orange;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(
            healthy ? Icons.check_circle : Icons.info,
            color: color,
            size: 16,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              title,
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
          ),
          const SizedBox(width: 8),
          Flexible(
            child: Text(
              value,
              textAlign: TextAlign.end,
              style: TextStyle(color: color, fontSize: 12),
            ),
          ),
        ],
      ),
    );
  }
}
