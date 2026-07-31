class CommandAuditEntry {
  const CommandAuditEntry({
    required this.actorFingerprint,
    required this.action,
    required this.target,
    required this.outcome,
    required this.completedAtMs,
  });

  final String actorFingerprint;
  final String action;
  final String target;
  final String outcome;
  final int completedAtMs;

  factory CommandAuditEntry.fromJson(Map<String, Object?> json) {
    String text(String key) => json[key] is String ? json[key] as String : '';
    final actorFingerprint = text('actorFingerprint');
    final action = text('action');
    final target = text('target');
    final outcome = text('outcome');
    final completedAtMs = json['completedAtMs'];
    if (!actorFingerprint.startsWith('audit:') ||
        actorFingerprint.length != 'audit:'.length + 64 ||
        action.isEmpty ||
        target.isEmpty ||
        outcome.isEmpty ||
        completedAtMs is! int ||
        completedAtMs < 0) {
      throw const FormatException('command audit entry is invalid');
    }
    return CommandAuditEntry(
      actorFingerprint: actorFingerprint,
      action: action,
      target: target,
      outcome: outcome,
      completedAtMs: completedAtMs,
    );
  }
}
