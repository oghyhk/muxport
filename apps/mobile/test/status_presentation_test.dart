import 'package:flutter_protocol/flutter_protocol.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/state/mobile_sync_state.dart';
import 'package:muxport_mobile/state/status_presentation.dart';

void main() {
  test('every connector protocol state has a stable presentation', () {
    for (var value = 0; value <= 6; value++) {
      final status = connectorStatusPresentation(value);
      expect(status.label, isNotEmpty);
      expect(status.explanation, isNotEmpty);
    }
    expect(connectorStatusPresentation(2).label, 'Vault locked');
    expect(connectorStatusPresentation(999).label, 'Unknown connector state');
  });

  test('every runtime protocol state has a stable presentation', () {
    for (var value = 0; value <= 12; value++) {
      final status = runtimeStatusPresentation(value);
      expect(status.label, isNotEmpty);
      expect(status.explanation, isNotEmpty);
    }
    expect(runtimeStatusPresentation(7).label, 'Waiting for approval');
    expect(runtimeStatusPresentation(11).label, 'Crash loop');
  });

  test('every sync and local operation state has a stable presentation', () {
    for (final phase in HostSyncPhase.values) {
      final status = hostSyncPresentation(phase);
      expect(status.label, isNotEmpty);
      expect(status.explanation, isNotEmpty);
    }
    for (final state in MobileOperationState.values) {
      final status = operationStatusPresentation(state);
      expect(status.label, isNotEmpty);
      expect(status.explanation, isNotEmpty);
    }
    for (final state in RemoteOpState.values) {
      final status = remoteOperationStatusPresentation(state);
      expect(status.label, isNotEmpty);
      expect(status.explanation, isNotEmpty);
    }
  });
}
