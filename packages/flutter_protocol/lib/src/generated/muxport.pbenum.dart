// This is a generated file - do not edit.
//
// Generated from muxport.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports

import 'dart:core' as $core;

import 'package:protobuf/protobuf.dart' as $pb;

/// Connector State Machine
class ConnectorState extends $pb.ProtobufEnum {
  static const ConnectorState CONNECTOR_STATE_UNSPECIFIED =
      ConnectorState._(0, _omitEnumNames ? '' : 'CONNECTOR_STATE_UNSPECIFIED');
  static const ConnectorState CONNECTOR_STATE_STARTING =
      ConnectorState._(1, _omitEnumNames ? '' : 'CONNECTOR_STATE_STARTING');
  static const ConnectorState CONNECTOR_STATE_VAULT_LOCKED =
      ConnectorState._(2, _omitEnumNames ? '' : 'CONNECTOR_STATE_VAULT_LOCKED');
  static const ConnectorState CONNECTOR_STATE_RECOVERING =
      ConnectorState._(3, _omitEnumNames ? '' : 'CONNECTOR_STATE_RECOVERING');
  static const ConnectorState CONNECTOR_STATE_READY =
      ConnectorState._(4, _omitEnumNames ? '' : 'CONNECTOR_STATE_READY');
  static const ConnectorState CONNECTOR_STATE_DEGRADED =
      ConnectorState._(5, _omitEnumNames ? '' : 'CONNECTOR_STATE_DEGRADED');
  static const ConnectorState CONNECTOR_STATE_FATAL_ERROR =
      ConnectorState._(6, _omitEnumNames ? '' : 'CONNECTOR_STATE_FATAL_ERROR');

  static const $core.List<ConnectorState> values = <ConnectorState>[
    CONNECTOR_STATE_UNSPECIFIED,
    CONNECTOR_STATE_STARTING,
    CONNECTOR_STATE_VAULT_LOCKED,
    CONNECTOR_STATE_RECOVERING,
    CONNECTOR_STATE_READY,
    CONNECTOR_STATE_DEGRADED,
    CONNECTOR_STATE_FATAL_ERROR,
  ];

  static final $core.List<ConnectorState?> _byValue =
      $pb.ProtobufEnum.$_initByValueList(values, 6);
  static ConnectorState? valueOf($core.int value) =>
      value < 0 || value >= _byValue.length ? null : _byValue[value];

  const ConnectorState._(super.value, super.name);
}

/// Runtime State Machine
class RuntimeState extends $pb.ProtobufEnum {
  static const RuntimeState RUNTIME_STATE_UNSPECIFIED =
      RuntimeState._(0, _omitEnumNames ? '' : 'RUNTIME_STATE_UNSPECIFIED');
  static const RuntimeState RUNTIME_STATE_UNKNOWN =
      RuntimeState._(1, _omitEnumNames ? '' : 'RUNTIME_STATE_UNKNOWN');
  static const RuntimeState RUNTIME_STATE_DISCOVERING =
      RuntimeState._(2, _omitEnumNames ? '' : 'RUNTIME_STATE_DISCOVERING');
  static const RuntimeState RUNTIME_STATE_STARTING =
      RuntimeState._(3, _omitEnumNames ? '' : 'RUNTIME_STATE_STARTING');
  static const RuntimeState RUNTIME_STATE_SYNCHRONIZING =
      RuntimeState._(4, _omitEnumNames ? '' : 'RUNTIME_STATE_SYNCHRONIZING');
  static const RuntimeState RUNTIME_STATE_ONLINE_IDLE =
      RuntimeState._(5, _omitEnumNames ? '' : 'RUNTIME_STATE_ONLINE_IDLE');
  static const RuntimeState RUNTIME_STATE_ONLINE_RUNNING =
      RuntimeState._(6, _omitEnumNames ? '' : 'RUNTIME_STATE_ONLINE_RUNNING');
  static const RuntimeState RUNTIME_STATE_ONLINE_WAITING_APPROVAL =
      RuntimeState._(
          7, _omitEnumNames ? '' : 'RUNTIME_STATE_ONLINE_WAITING_APPROVAL');
  static const RuntimeState RUNTIME_STATE_DEGRADED =
      RuntimeState._(8, _omitEnumNames ? '' : 'RUNTIME_STATE_DEGRADED');
  static const RuntimeState RUNTIME_STATE_STOPPED =
      RuntimeState._(9, _omitEnumNames ? '' : 'RUNTIME_STATE_STOPPED');
  static const RuntimeState RUNTIME_STATE_CRASHED =
      RuntimeState._(10, _omitEnumNames ? '' : 'RUNTIME_STATE_CRASHED');
  static const RuntimeState RUNTIME_STATE_CRASH_LOOP =
      RuntimeState._(11, _omitEnumNames ? '' : 'RUNTIME_STATE_CRASH_LOOP');
  static const RuntimeState RUNTIME_STATE_CREDENTIAL_LOCKED = RuntimeState._(
      12, _omitEnumNames ? '' : 'RUNTIME_STATE_CREDENTIAL_LOCKED');

  static const $core.List<RuntimeState> values = <RuntimeState>[
    RUNTIME_STATE_UNSPECIFIED,
    RUNTIME_STATE_UNKNOWN,
    RUNTIME_STATE_DISCOVERING,
    RUNTIME_STATE_STARTING,
    RUNTIME_STATE_SYNCHRONIZING,
    RUNTIME_STATE_ONLINE_IDLE,
    RUNTIME_STATE_ONLINE_RUNNING,
    RUNTIME_STATE_ONLINE_WAITING_APPROVAL,
    RUNTIME_STATE_DEGRADED,
    RUNTIME_STATE_STOPPED,
    RUNTIME_STATE_CRASHED,
    RUNTIME_STATE_CRASH_LOOP,
    RUNTIME_STATE_CREDENTIAL_LOCKED,
  ];

  static final $core.List<RuntimeState?> _byValue =
      $pb.ProtobufEnum.$_initByValueList(values, 12);
  static RuntimeState? valueOf($core.int value) =>
      value < 0 || value >= _byValue.length ? null : _byValue[value];

  const RuntimeState._(super.value, super.name);
}

/// Operation State Machine
class RemoteOpState extends $pb.ProtobufEnum {
  static const RemoteOpState REMOTE_OP_STATE_UNSPECIFIED =
      RemoteOpState._(0, _omitEnumNames ? '' : 'REMOTE_OP_STATE_UNSPECIFIED');
  static const RemoteOpState REMOTE_OP_STATE_CREATED =
      RemoteOpState._(1, _omitEnumNames ? '' : 'REMOTE_OP_STATE_CREATED');
  static const RemoteOpState REMOTE_OP_STATE_PERSISTED =
      RemoteOpState._(2, _omitEnumNames ? '' : 'REMOTE_OP_STATE_PERSISTED');
  static const RemoteOpState REMOTE_OP_STATE_DISPATCHED =
      RemoteOpState._(3, _omitEnumNames ? '' : 'REMOTE_OP_STATE_DISPATCHED');
  static const RemoteOpState REMOTE_OP_STATE_SOURCE_ACKNOWLEDGED =
      RemoteOpState._(
          4, _omitEnumNames ? '' : 'REMOTE_OP_STATE_SOURCE_ACKNOWLEDGED');
  static const RemoteOpState REMOTE_OP_STATE_RECONCILED =
      RemoteOpState._(5, _omitEnumNames ? '' : 'REMOTE_OP_STATE_RECONCILED');
  static const RemoteOpState REMOTE_OP_STATE_SUCCEEDED =
      RemoteOpState._(6, _omitEnumNames ? '' : 'REMOTE_OP_STATE_SUCCEEDED');
  static const RemoteOpState REMOTE_OP_STATE_EXPIRED =
      RemoteOpState._(7, _omitEnumNames ? '' : 'REMOTE_OP_STATE_EXPIRED');
  static const RemoteOpState REMOTE_OP_STATE_CANCELLED =
      RemoteOpState._(8, _omitEnumNames ? '' : 'REMOTE_OP_STATE_CANCELLED');
  static const RemoteOpState REMOTE_OP_STATE_REJECTED_OFFLINE = RemoteOpState._(
      9, _omitEnumNames ? '' : 'REMOTE_OP_STATE_REJECTED_OFFLINE');
  static const RemoteOpState REMOTE_OP_STATE_FAILED =
      RemoteOpState._(10, _omitEnumNames ? '' : 'REMOTE_OP_STATE_FAILED');
  static const RemoteOpState REMOTE_OP_STATE_OUTCOME_UNKNOWN = RemoteOpState._(
      11, _omitEnumNames ? '' : 'REMOTE_OP_STATE_OUTCOME_UNKNOWN');
  static const RemoteOpState REMOTE_OP_STATE_RECONCILIATION_REQUIRED =
      RemoteOpState._(
          12, _omitEnumNames ? '' : 'REMOTE_OP_STATE_RECONCILIATION_REQUIRED');

  static const $core.List<RemoteOpState> values = <RemoteOpState>[
    REMOTE_OP_STATE_UNSPECIFIED,
    REMOTE_OP_STATE_CREATED,
    REMOTE_OP_STATE_PERSISTED,
    REMOTE_OP_STATE_DISPATCHED,
    REMOTE_OP_STATE_SOURCE_ACKNOWLEDGED,
    REMOTE_OP_STATE_RECONCILED,
    REMOTE_OP_STATE_SUCCEEDED,
    REMOTE_OP_STATE_EXPIRED,
    REMOTE_OP_STATE_CANCELLED,
    REMOTE_OP_STATE_REJECTED_OFFLINE,
    REMOTE_OP_STATE_FAILED,
    REMOTE_OP_STATE_OUTCOME_UNKNOWN,
    REMOTE_OP_STATE_RECONCILIATION_REQUIRED,
  ];

  static final $core.List<RemoteOpState?> _byValue =
      $pb.ProtobufEnum.$_initByValueList(values, 12);
  static RemoteOpState? valueOf($core.int value) =>
      value < 0 || value >= _byValue.length ? null : _byValue[value];

  const RemoteOpState._(super.value, super.name);
}

/// Credential Status
class CredentialStatus extends $pb.ProtobufEnum {
  static const CredentialStatus CREDENTIAL_STATUS_UNSPECIFIED =
      CredentialStatus._(
          0, _omitEnumNames ? '' : 'CREDENTIAL_STATUS_UNSPECIFIED');
  static const CredentialStatus CREDENTIAL_STATUS_STAGED =
      CredentialStatus._(1, _omitEnumNames ? '' : 'CREDENTIAL_STATUS_STAGED');
  static const CredentialStatus CREDENTIAL_STATUS_ACTIVE =
      CredentialStatus._(2, _omitEnumNames ? '' : 'CREDENTIAL_STATUS_ACTIVE');
  static const CredentialStatus CREDENTIAL_STATUS_COOLING_DOWN =
      CredentialStatus._(
          3, _omitEnumNames ? '' : 'CREDENTIAL_STATUS_COOLING_DOWN');
  static const CredentialStatus CREDENTIAL_STATUS_INVALID =
      CredentialStatus._(4, _omitEnumNames ? '' : 'CREDENTIAL_STATUS_INVALID');
  static const CredentialStatus CREDENTIAL_STATUS_REVOKED =
      CredentialStatus._(5, _omitEnumNames ? '' : 'CREDENTIAL_STATUS_REVOKED');

  /// Disabled is a local Muxport state. It must never be presented as a claim
  /// that the provider has revoked the remote credential.
  static const CredentialStatus CREDENTIAL_STATUS_DISABLED =
      CredentialStatus._(6, _omitEnumNames ? '' : 'CREDENTIAL_STATUS_DISABLED');

  static const $core.List<CredentialStatus> values = <CredentialStatus>[
    CREDENTIAL_STATUS_UNSPECIFIED,
    CREDENTIAL_STATUS_STAGED,
    CREDENTIAL_STATUS_ACTIVE,
    CREDENTIAL_STATUS_COOLING_DOWN,
    CREDENTIAL_STATUS_INVALID,
    CREDENTIAL_STATUS_REVOKED,
    CREDENTIAL_STATUS_DISABLED,
  ];

  static final $core.List<CredentialStatus?> _byValue =
      $pb.ProtobufEnum.$_initByValueList(values, 6);
  static CredentialStatus? valueOf($core.int value) =>
      value < 0 || value >= _byValue.length ? null : _byValue[value];

  const CredentialStatus._(super.value, super.name);
}

/// Agent Type
class AgentType extends $pb.ProtobufEnum {
  static const AgentType AGENT_TYPE_UNSPECIFIED =
      AgentType._(0, _omitEnumNames ? '' : 'AGENT_TYPE_UNSPECIFIED');
  static const AgentType AGENT_TYPE_OPENCODE =
      AgentType._(1, _omitEnumNames ? '' : 'AGENT_TYPE_OPENCODE');
  static const AgentType AGENT_TYPE_CODEX =
      AgentType._(2, _omitEnumNames ? '' : 'AGENT_TYPE_CODEX');

  static const $core.List<AgentType> values = <AgentType>[
    AGENT_TYPE_UNSPECIFIED,
    AGENT_TYPE_OPENCODE,
    AGENT_TYPE_CODEX,
  ];

  static final $core.List<AgentType?> _byValue =
      $pb.ProtobufEnum.$_initByValueList(values, 2);
  static AgentType? valueOf($core.int value) =>
      value < 0 || value >= _byValue.length ? null : _byValue[value];

  const AgentType._(super.value, super.name);
}

const $core.bool _omitEnumNames =
    $core.bool.fromEnvironment('protobuf.omit_enum_names');
