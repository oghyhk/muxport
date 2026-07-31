import 'dart:convert';

import 'package:crypto/crypto.dart' as hashes;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../security/device_identity.dart';
import '../security/sensitive_inputs.dart';
import '../security/step_up_authenticator.dart';
import '../state/mobile_sync_state.dart';
import '../transport/direct_transport_client.dart';
import '../transport/direct_transport_protocol.dart';

class CredentialProvisioningScreen extends StatefulWidget {
  const CredentialProvisioningScreen({
    required this.hosts,
    required this.identity,
    required this.sensitiveInputs,
    required this.stepUpAuthenticator,
    required this.onProvisioned,
    super.key,
  });

  final Iterable<HostSyncState> hosts;
  final MobileDeviceIdentity identity;
  final SensitiveInputRegistry sensitiveInputs;
  final StepUpAuthenticator stepUpAuthenticator;
  final Future<void> Function(HostSyncState host) onProvisioned;

  @override
  State<CredentialProvisioningScreen> createState() =>
      _CredentialProvisioningScreenState();
}

class _CredentialProvisioningScreenState
    extends State<CredentialProvisioningScreen> {
  late final TextEditingController _nameController;
  late final TextEditingController _providerController;
  late final TextEditingController _typeController;
  late final TextEditingController _secretController;
  final Set<String> _selectedHostIds = <String>{};
  bool _submitting = false;

  List<HostSyncState> get _eligibleHosts => [
    for (final host in widget.hosts)
      if (host.canMutate &&
          host.directAddress != null &&
          host.directPort != null)
        host,
  ];

  @override
  void initState() {
    super.initState();
    _nameController = widget.sensitiveInputs.createController();
    _providerController = widget.sensitiveInputs.createController();
    _typeController = widget.sensitiveInputs.createController();
    _secretController = widget.sensitiveInputs.createController();
    _providerController.text = 'openai';
    _typeController.text = 'api_key';
    final first = _eligibleHosts.firstOrNull;
    if (first != null) {
      _selectedHostIds.add(first.hostId);
    }
  }

  @override
  void dispose() {
    widget.sensitiveInputs.release(_nameController);
    widget.sensitiveInputs.release(_providerController);
    widget.sensitiveInputs.release(_typeController);
    widget.sensitiveInputs.release(_secretController);
    super.dispose();
  }

  Future<void> _submit() async {
    final hosts = [
      for (final host in _eligibleHosts)
        if (_selectedHostIds.contains(host.hostId)) host,
    ];
    final displayName = _nameController.text.trim();
    final provider = _providerController.text.trim();
    final credentialType = _typeController.text.trim();
    final originalSecret = _secretController.text;
    if (_submitting ||
        hosts.isEmpty ||
        displayName.isEmpty ||
        provider.isEmpty ||
        credentialType.isEmpty ||
        originalSecret.isEmpty) {
      _show('Choose a connected host and complete every field.');
      return;
    }
    final authorized = await widget.stepUpAuthenticator.authorize(
      reason: 'Authorize adding a provider credential to ${hosts.length} selected host${hosts.length == 1 ? '' : 's'}',
    );
    if (!mounted) {
      return;
    }
    if (!authorized) {
      _show('Device authentication is required. No credential was sent.');
      return;
    }

    final secret = Uint8List.fromList(utf8.encode(originalSecret));
    final digest = Uint8List.fromList(hashes.sha256.convert(secret).bytes);
    final accountFingerprint = 'key-${_hex(digest.sublist(0, 8))}';
    digest.fillRange(0, digest.length, 0);
    _secretController.clear();
    // Clipboard clearing is intentionally best-effort: it may have been used
    // to paste the credential, and we should not leave it available after a
    // sensitive successful action.
    try {
      await Clipboard.setData(const ClipboardData(text: ''));
    } on Object {
      // The field has still been cleared; platform clipboard access is optional.
    }

    setState(() {
      _submitting = true;
    });
    final provisioned = <HostSyncState>[];
    final failedHosts = <HostSyncState>[];
    try {
      final now = DateTime.now();
      for (var index = 0; index < hosts.length; index++) {
        final host = hosts[index];
        final operationId =
            'provision-${widget.identity.deviceId}-${now.microsecondsSinceEpoch}-$index';
        AuthenticatedDirectConnection? connection;
        try {
          connection = await AuthenticatedDirectConnection.connect(
            address: host.directAddress!,
            port: host.directPort!,
            pinnedHost: PinnedHostIdentity(
              hostId: host.hostId,
              publicKeyHex: host.pinnedHostKey,
            ),
            identity: widget.identity,
          );
          final result = await connection.provisionCredential(
            commandId: operationId,
            idempotencyKey: operationId,
            profileId: operationId,
            displayName: displayName,
            provider: provider,
            credentialType: credentialType,
            accountFingerprint: accountFingerprint,
            secret: secret,
          );
          if (result.success) {
            await widget.onProvisioned(host);
            provisioned.add(host);
          } else {
            failedHosts.add(host);
          }
        } on Object {
          failedHosts.add(host);
        } finally {
          await connection?.close();
        }
      }
      if (!mounted) {
        return;
      }
      if (failedHosts.isEmpty) {
        _show(
          'Credential sealed on ${provisioned.length} host${provisioned.length == 1 ? '' : 's'}; it is not assigned yet.',
        );
        Navigator.of(context).pop();
      } else {
        _show(
          'Sealed on ${provisioned.length}; ${failedHosts.length} host${failedHosts.length == 1 ? '' : 's'} could not confirm it. Verify those hosts before retrying.',
        );
      }
    } finally {
      secret.fillRange(0, secret.length, 0);
      if (mounted) {
        setState(() {
          _submitting = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final hosts = _eligibleHosts;
    return Scaffold(
      appBar: AppBar(title: const Text('Add provider credential')),
      body: ListView(
        padding: const EdgeInsets.all(20),
        children: [
          const Text(
            'The key is encrypted directly for the selected host after device authentication. It is not saved on this phone or sent to the relay.',
          ),
          const SizedBox(height: 20),
          const Text('Target hosts'),
          const SizedBox(height: 4),
          Card(
            child: Column(
              children: [
                for (final host in hosts)
                  CheckboxListTile(
                    value: _selectedHostIds.contains(host.hostId),
                    title: Text(host.displayName),
                    subtitle: Text(host.hostId),
                    controlAffinity: ListTileControlAffinity.leading,
                    onChanged: _submitting
                        ? null
                        : (selected) => setState(() {
                            if (selected ?? false) {
                              _selectedHostIds.add(host.hostId);
                            } else {
                              _selectedHostIds.remove(host.hostId);
                            }
                          }),
                  ),
              ],
            ),
          ),
          if (hosts.isEmpty) ...[
            const SizedBox(height: 12),
            const Text(
              'No currently authenticated direct host can receive a credential.',
              style: TextStyle(color: Colors.amber),
            ),
          ],
          const SizedBox(height: 16),
          TextField(
            controller: _nameController,
            enabled: !_submitting,
            autocorrect: false,
            enableSuggestions: false,
            decoration: const InputDecoration(
              labelText: 'Account label',
              hintText: 'Personal OpenAI',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _providerController,
            enabled: !_submitting,
            autocorrect: false,
            enableSuggestions: false,
            decoration: const InputDecoration(
              labelText: 'Provider',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _typeController,
            enabled: !_submitting,
            autocorrect: false,
            enableSuggestions: false,
            decoration: const InputDecoration(
              labelText: 'Credential type',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: _secretController,
            enabled: !_submitting,
            obscureText: true,
            autocorrect: false,
            enableSuggestions: false,
            keyboardType: TextInputType.visiblePassword,
            decoration: const InputDecoration(
              labelText: 'Provider key',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 20),
          FilledButton.icon(
            onPressed: _submitting || hosts.isEmpty ? null : _submit,
            icon: _submitting
                ? const SizedBox.square(
                    dimension: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.lock_outline),
            label: Text(_submitting ? 'Encrypting…' : 'Authenticate and add'),
          ),
        ],
      ),
    );
  }

  void _show(String text) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(text)));
  }
}

String _hex(List<int> bytes) {
  const alphabet = '0123456789abcdef';
  final output = StringBuffer();
  for (final byte in bytes) {
    output
      ..write(alphabet[(byte >> 4) & 0x0f])
      ..write(alphabet[byte & 0x0f]);
  }
  return output.toString();
}

extension _FirstOrNull<T> on Iterable<T> {
  T? get firstOrNull {
    final iterator = this.iterator;
    return iterator.moveNext() ? iterator.current : null;
  }
}
