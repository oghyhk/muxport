import 'package:flutter/material.dart';

import '../diagnostics/redacted_diagnostic_export.dart';
import '../state/app_bootstrap.dart';
import '../state/command_audit.dart';
import '../state/mobile_sync_state.dart';

class DiagnosticsScreen extends StatefulWidget {
  const DiagnosticsScreen({
    required this.bootstrap,
    required this.hosts,
    this.onLoadCommandAudit,
    this.appLockEnabled = false,
    this.onSetAppLock,
    super.key,
  });

  final AppBootstrapState bootstrap;
  final Iterable<HostSyncState> hosts;
  final Future<List<CommandAuditEntry>> Function(HostSyncState host)?
  onLoadCommandAudit;
  final bool appLockEnabled;
  final Future<bool> Function(bool enabled)? onSetAppLock;

  @override
  State<DiagnosticsScreen> createState() => _DiagnosticsScreenState();
}

class _DiagnosticsScreenState extends State<DiagnosticsScreen> {
  final Map<String, List<CommandAuditEntry>> _auditByHost = {};
  var _auditLoading = false;
  var _auditLoadFailed = false;
  var _appLockUpdating = false;

  Future<void> _loadCommandAudit() async {
    final load = widget.onLoadCommandAudit;
    if (load == null || _auditLoading) return;
    final hosts = widget.hosts.where((host) => host.canMutate).toList();
    if (hosts.isEmpty) return;
    setState(() {
      _auditLoading = true;
      _auditLoadFailed = false;
    });
    final next = <String, List<CommandAuditEntry>>{};
    var failed = false;
    for (final host in hosts) {
      try {
        next[host.hostId] = await load(host);
      } on Object {
        failed = true;
      }
    }
    if (!mounted) return;
    setState(() {
      _auditByHost
        ..clear()
        ..addAll(next);
      _auditLoading = false;
      _auditLoadFailed = failed;
    });
  }

  Future<void> _setAppLock(bool enabled) async {
    final update = widget.onSetAppLock;
    if (update == null || _appLockUpdating) return;
    setState(() => _appLockUpdating = true);
    final saved = await update(enabled);
    if (!mounted) return;
    setState(() => _appLockUpdating = false);
    if (!saved) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('Could not update the protected app-lock preference.'),
        ),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final identityReady =
        widget.bootstrap.identityStatus == IdentityBootstrapStatus.ready;
    final syncHealthy = widget.hosts.any(
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
                    value: _cacheLabel(widget.bootstrap.cacheStatus),
                    healthy:
                        widget.bootstrap.cacheStatus ==
                            CacheBootstrapStatus.ready ||
                        widget.bootstrap.cacheStatus ==
                            CacheBootstrapStatus.recoveredPreviousGeneration,
                  ),
                  _HealthRow(
                    title: 'Cache generation',
                    value: widget.bootstrap.cacheGeneration.toString(),
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
                  _HealthRow(
                    title: 'Connector security log',
                    value: _auditStatus(),
                    healthy: !_auditLoadFailed,
                  ),
                  SwitchListTile.adaptive(
                    contentPadding: EdgeInsets.zero,
                    title: const Text('Optional app lock'),
                    subtitle: const Text(
                      'Require device authentication after the app backgrounds. Uses biometrics or the OS device credential.',
                    ),
                    value: widget.appLockEnabled,
                    onChanged: widget.onSetAppLock == null || _appLockUpdating
                        ? null
                        : _setAppLock,
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          FilledButton.icon(
            icon: _auditLoading
                ? const SizedBox.square(
                    dimension: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.history),
            label: const Text('Refresh connector security log'),
            onPressed: widget.onLoadCommandAudit == null || _auditLoading
                ? null
                : _loadCommandAudit,
          ),
          for (final host in widget.hosts)
            if (_auditByHost.containsKey(host.hostId))
              _AuditHostCard(host: host, entries: _auditByHost[host.hostId]!),
          const SizedBox(height: 16),
          FilledButton.icon(
            icon: const Icon(Icons.download),
            label: const Text('Export redacted diagnostics'),
            onPressed: widget.bootstrap.sensitiveArtifactStore == null
                ? null
                : () => _confirmAndExport(context),
          ),
        ],
      ),
    );
  }

  Future<void> _confirmAndExport(BuildContext context) async {
    final store = widget.bootstrap.sensitiveArtifactStore;
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
      ).create(bootstrap: widget.bootstrap, hosts: widget.hosts);
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

  String _auditStatus() {
    if (widget.onLoadCommandAudit == null) {
      return 'NO AUTHENTICATED HOST';
    }
    if (_auditLoading) return 'REFRESHING';
    if (_auditLoadFailed) return 'PARTIAL READ FAILURE';
    final count = _auditByHost.values.fold<int>(
      0,
      (total, entries) => total + entries.length,
    );
    return _auditByHost.isEmpty ? 'NOT LOADED' : '$count REDACTED RECORDS';
  }
}

class _AuditHostCard extends StatelessWidget {
  const _AuditHostCard({required this.host, required this.entries});

  final HostSyncState host;
  final List<CommandAuditEntry> entries;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: ExpansionTile(
        leading: const Icon(Icons.shield_outlined),
        title: Text('${host.displayName} security log'),
        subtitle: Text('${entries.length} redacted record(s)'),
        children: [
          if (entries.isEmpty)
            const ListTile(title: Text('No recent connector records')),
          for (final entry in entries)
            ListTile(
              title: Text('${entry.action} · ${entry.outcome}'),
              subtitle: Text(
                '${entry.target} · device ${entry.actorFingerprint.substring(entry.actorFingerprint.length - 12)}',
              ),
              trailing: Text(
                DateTime.fromMillisecondsSinceEpoch(
                  entry.completedAtMs,
                ).toLocal().toIso8601String().substring(0, 16),
                textAlign: TextAlign.end,
                style: Theme.of(context).textTheme.labelSmall,
              ),
            ),
        ],
      ),
    );
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
