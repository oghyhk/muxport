import 'dart:io';

import 'package:path_provider/path_provider.dart';

/// Owns plaintext artifacts that must never survive an app restart.
///
/// Callers may create transient attachments only beneath [directory]. The
/// directory is deliberately separate from the durable cache and secure
/// identity stores so it can be erased without risking either.
class SensitiveArtifactStore {
  const SensitiveArtifactStore(this.directory);

  final Directory directory;

  static Future<SensitiveArtifactStore> createDefault() async {
    final temporaryRoot = await getTemporaryDirectory();
    return SensitiveArtifactStore(
      Directory.fromUri(temporaryRoot.uri.resolve('muxport-sensitive/')),
    );
  }

  Future<void> clear() async {
    if (await directory.exists()) {
      await directory.delete(recursive: true);
    }
  }

  Future<Directory> prepare() async {
    await directory.create(recursive: true);
    return directory;
  }
}
