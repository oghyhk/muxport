import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:muxport_mobile/security/sensitive_artifacts.dart';

void main() {
  test(
    'sensitive artifacts are erased recursively and can be prepared again',
    () async {
      final root = await Directory.systemTemp.createTemp(
        'muxport-sensitive-artifacts-',
      );
      final store = SensitiveArtifactStore(
        Directory.fromUri(root.uri.resolve('owned/')),
      );
      try {
        final directory = await store.prepare();
        final nested = Directory.fromUri(directory.uri.resolve('attachments/'));
        await nested.create();
        await File.fromUri(
          nested.uri.resolve('prompt.txt'),
        ).writeAsString('plaintext that must not survive', flush: true);

        await store.clear();

        expect(await directory.exists(), isFalse);
        expect(await root.exists(), isTrue);
        expect(await store.prepare(), same(store.directory));
        expect(await store.directory.exists(), isTrue);
      } finally {
        if (await root.exists()) {
          await root.delete(recursive: true);
        }
      }
    },
  );
}
