class RotationPoolSummary {
  const RotationPoolSummary({
    required this.poolId,
    required this.providerId,
    required this.orderedProfileIds,
    required this.mode,
    required this.cooldownMs,
    required this.maxSwitchesPerHour,
    required this.allowedHostIds,
    required this.quotaFailoverEnabled,
  });

  final String poolId;
  final String providerId;
  final List<String> orderedProfileIds;
  final String mode;
  final int cooldownMs;
  final int maxSwitchesPerHour;
  final List<String> allowedHostIds;
  final bool quotaFailoverEnabled;

  factory RotationPoolSummary.fromJson(Map<String, Object?> json) {
    String text(String key) => json[key] is String ? json[key] as String : '';
    List<String>? textList(String key) {
      final raw = json[key];
      if (raw is! List) return null;
      final values = <String>[];
      for (final value in raw) {
        if (value is! String || value.trim().isEmpty) return null;
        values.add(value);
      }
      return values;
    }

    final poolId = text('poolId');
    final providerId = text('providerId');
    final mode = text('mode');
    final profiles = textList('orderedProfileIds');
    final allowedHostIds = textList('allowedHostIds');
    final cooldownMs = json['cooldownMs'];
    final maxSwitchesPerHour = json['maxSwitchesPerHour'];
    if (poolId.isEmpty ||
        providerId.isEmpty ||
        mode.isEmpty ||
        profiles == null ||
        profiles.length < 2 ||
        profiles.toSet().length != profiles.length ||
        allowedHostIds == null ||
        cooldownMs is! int ||
        cooldownMs < 0 ||
        maxSwitchesPerHour is! int ||
        maxSwitchesPerHour <= 0) {
      throw const FormatException('rotation pool metadata is invalid');
    }
    return RotationPoolSummary(
      poolId: poolId,
      providerId: providerId,
      orderedProfileIds: List.unmodifiable(profiles),
      mode: mode,
      cooldownMs: cooldownMs,
      maxSwitchesPerHour: maxSwitchesPerHour,
      allowedHostIds: List.unmodifiable(allowedHostIds),
      quotaFailoverEnabled: json['quotaFailoverEnabled'] == true,
    );
  }
}

/// A non-secret policy draft. The connector revalidates every referenced
/// profile against its vault before persisting it.
class RotationPoolDraft {
  const RotationPoolDraft({
    required this.poolId,
    required this.providerId,
    required this.orderedProfileIds,
    required this.mode,
    required this.cooldownMs,
    required this.maxSwitchesPerHour,
    required this.allowedHostIds,
    required this.quotaFailoverEnabled,
  });

  final String poolId;
  final String providerId;
  final List<String> orderedProfileIds;
  final String mode;
  final int cooldownMs;
  final int maxSwitchesPerHour;
  final List<String> allowedHostIds;
  final bool quotaFailoverEnabled;
}
