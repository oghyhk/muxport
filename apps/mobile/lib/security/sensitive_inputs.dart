import 'package:flutter/widgets.dart';

/// Tracks in-memory plaintext fields so lifecycle suspension can wipe them.
class SensitiveInputRegistry {
  final Set<TextEditingController> _controllers = {};
  bool _disposed = false;

  TextEditingController createController() {
    if (_disposed) {
      throw StateError('sensitive input registry is disposed');
    }
    final controller = TextEditingController();
    _controllers.add(controller);
    return controller;
  }

  void clear() {
    if (_disposed) {
      return;
    }
    for (final controller in _controllers) {
      controller.clear();
    }
  }

  void release(TextEditingController controller) {
    if (_controllers.remove(controller)) {
      controller.clear();
      controller.dispose();
    }
  }

  void dispose() {
    if (_disposed) {
      return;
    }
    clear();
    for (final controller in _controllers) {
      controller.dispose();
    }
    _controllers.clear();
    _disposed = true;
  }
}
