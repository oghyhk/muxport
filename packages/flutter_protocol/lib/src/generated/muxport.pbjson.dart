// This is a generated file - do not edit.
//
// Generated from muxport.proto.

// @dart = 3.3

// ignore_for_file: annotate_overrides, camel_case_types, comment_references
// ignore_for_file: constant_identifier_names
// ignore_for_file: curly_braces_in_flow_control_structures
// ignore_for_file: deprecated_member_use_from_same_package, library_prefixes
// ignore_for_file: non_constant_identifier_names, prefer_relative_imports
// ignore_for_file: unused_import

import 'dart:convert' as $convert;
import 'dart:core' as $core;
import 'dart:typed_data' as $typed_data;

@$core.Deprecated('Use connectorStateDescriptor instead')
const ConnectorState$json = {
  '1': 'ConnectorState',
  '2': [
    {'1': 'CONNECTOR_STATE_UNSPECIFIED', '2': 0},
    {'1': 'CONNECTOR_STATE_STARTING', '2': 1},
    {'1': 'CONNECTOR_STATE_VAULT_LOCKED', '2': 2},
    {'1': 'CONNECTOR_STATE_RECOVERING', '2': 3},
    {'1': 'CONNECTOR_STATE_READY', '2': 4},
    {'1': 'CONNECTOR_STATE_DEGRADED', '2': 5},
    {'1': 'CONNECTOR_STATE_FATAL_ERROR', '2': 6},
  ],
};

/// Descriptor for `ConnectorState`. Decode as a `google.protobuf.EnumDescriptorProto`.
final $typed_data.Uint8List connectorStateDescriptor = $convert.base64Decode(
    'Cg5Db25uZWN0b3JTdGF0ZRIfChtDT05ORUNUT1JfU1RBVEVfVU5TUEVDSUZJRUQQABIcChhDT0'
    '5ORUNUT1JfU1RBVEVfU1RBUlRJTkcQARIgChxDT05ORUNUT1JfU1RBVEVfVkFVTFRfTE9DS0VE'
    'EAISHgoaQ09OTkVDVE9SX1NUQVRFX1JFQ09WRVJJTkcQAxIZChVDT05ORUNUT1JfU1RBVEVfUk'
    'VBRFkQBBIcChhDT05ORUNUT1JfU1RBVEVfREVHUkFERUQQBRIfChtDT05ORUNUT1JfU1RBVEVf'
    'RkFUQUxfRVJST1IQBg==');

@$core.Deprecated('Use runtimeStateDescriptor instead')
const RuntimeState$json = {
  '1': 'RuntimeState',
  '2': [
    {'1': 'RUNTIME_STATE_UNSPECIFIED', '2': 0},
    {'1': 'RUNTIME_STATE_UNKNOWN', '2': 1},
    {'1': 'RUNTIME_STATE_DISCOVERING', '2': 2},
    {'1': 'RUNTIME_STATE_STARTING', '2': 3},
    {'1': 'RUNTIME_STATE_SYNCHRONIZING', '2': 4},
    {'1': 'RUNTIME_STATE_ONLINE_IDLE', '2': 5},
    {'1': 'RUNTIME_STATE_ONLINE_RUNNING', '2': 6},
    {'1': 'RUNTIME_STATE_ONLINE_WAITING_APPROVAL', '2': 7},
    {'1': 'RUNTIME_STATE_DEGRADED', '2': 8},
    {'1': 'RUNTIME_STATE_STOPPED', '2': 9},
    {'1': 'RUNTIME_STATE_CRASHED', '2': 10},
    {'1': 'RUNTIME_STATE_CRASH_LOOP', '2': 11},
    {'1': 'RUNTIME_STATE_CREDENTIAL_LOCKED', '2': 12},
  ],
};

/// Descriptor for `RuntimeState`. Decode as a `google.protobuf.EnumDescriptorProto`.
final $typed_data.Uint8List runtimeStateDescriptor = $convert.base64Decode(
    'CgxSdW50aW1lU3RhdGUSHQoZUlVOVElNRV9TVEFURV9VTlNQRUNJRklFRBAAEhkKFVJVTlRJTU'
    'VfU1RBVEVfVU5LTk9XThABEh0KGVJVTlRJTUVfU1RBVEVfRElTQ09WRVJJTkcQAhIaChZSVU5U'
    'SU1FX1NUQVRFX1NUQVJUSU5HEAMSHwobUlVOVElNRV9TVEFURV9TWU5DSFJPTklaSU5HEAQSHQ'
    'oZUlVOVElNRV9TVEFURV9PTkxJTkVfSURMRRAFEiAKHFJVTlRJTUVfU1RBVEVfT05MSU5FX1JV'
    'Tk5JTkcQBhIpCiVSVU5USU1FX1NUQVRFX09OTElORV9XQUlUSU5HX0FQUFJPVkFMEAcSGgoWUl'
    'VOVElNRV9TVEFURV9ERUdSQURFRBAIEhkKFVJVTlRJTUVfU1RBVEVfU1RPUFBFRBAJEhkKFVJV'
    'TlRJTUVfU1RBVEVfQ1JBU0hFRBAKEhwKGFJVTlRJTUVfU1RBVEVfQ1JBU0hfTE9PUBALEiMKH1'
    'JVTlRJTUVfU1RBVEVfQ1JFREVOVElBTF9MT0NLRUQQDA==');

@$core.Deprecated('Use remoteOpStateDescriptor instead')
const RemoteOpState$json = {
  '1': 'RemoteOpState',
  '2': [
    {'1': 'REMOTE_OP_STATE_UNSPECIFIED', '2': 0},
    {'1': 'REMOTE_OP_STATE_CREATED', '2': 1},
    {'1': 'REMOTE_OP_STATE_PERSISTED', '2': 2},
    {'1': 'REMOTE_OP_STATE_DISPATCHED', '2': 3},
    {'1': 'REMOTE_OP_STATE_SOURCE_ACKNOWLEDGED', '2': 4},
    {'1': 'REMOTE_OP_STATE_RECONCILED', '2': 5},
    {'1': 'REMOTE_OP_STATE_SUCCEEDED', '2': 6},
    {'1': 'REMOTE_OP_STATE_EXPIRED', '2': 7},
    {'1': 'REMOTE_OP_STATE_CANCELLED', '2': 8},
    {'1': 'REMOTE_OP_STATE_REJECTED_OFFLINE', '2': 9},
    {'1': 'REMOTE_OP_STATE_FAILED', '2': 10},
    {'1': 'REMOTE_OP_STATE_OUTCOME_UNKNOWN', '2': 11},
    {'1': 'REMOTE_OP_STATE_RECONCILIATION_REQUIRED', '2': 12},
  ],
};

/// Descriptor for `RemoteOpState`. Decode as a `google.protobuf.EnumDescriptorProto`.
final $typed_data.Uint8List remoteOpStateDescriptor = $convert.base64Decode(
    'Cg1SZW1vdGVPcFN0YXRlEh8KG1JFTU9URV9PUF9TVEFURV9VTlNQRUNJRklFRBAAEhsKF1JFTU'
    '9URV9PUF9TVEFURV9DUkVBVEVEEAESHQoZUkVNT1RFX09QX1NUQVRFX1BFUlNJU1RFRBACEh4K'
    'GlJFTU9URV9PUF9TVEFURV9ESVNQQVRDSEVEEAMSJwojUkVNT1RFX09QX1NUQVRFX1NPVVJDRV'
    '9BQ0tOT1dMRURHRUQQBBIeChpSRU1PVEVfT1BfU1RBVEVfUkVDT05DSUxFRBAFEh0KGVJFTU9U'
    'RV9PUF9TVEFURV9TVUNDRUVERUQQBhIbChdSRU1PVEVfT1BfU1RBVEVfRVhQSVJFRBAHEh0KGV'
    'JFTU9URV9PUF9TVEFURV9DQU5DRUxMRUQQCBIkCiBSRU1PVEVfT1BfU1RBVEVfUkVKRUNURURf'
    'T0ZGTElORRAJEhoKFlJFTU9URV9PUF9TVEFURV9GQUlMRUQQChIjCh9SRU1PVEVfT1BfU1RBVE'
    'VfT1VUQ09NRV9VTktOT1dOEAsSKwonUkVNT1RFX09QX1NUQVRFX1JFQ09OQ0lMSUFUSU9OX1JF'
    'UVVJUkVEEAw=');

@$core.Deprecated('Use credentialStatusDescriptor instead')
const CredentialStatus$json = {
  '1': 'CredentialStatus',
  '2': [
    {'1': 'CREDENTIAL_STATUS_UNSPECIFIED', '2': 0},
    {'1': 'CREDENTIAL_STATUS_STAGED', '2': 1},
    {'1': 'CREDENTIAL_STATUS_ACTIVE', '2': 2},
    {'1': 'CREDENTIAL_STATUS_COOLING_DOWN', '2': 3},
    {'1': 'CREDENTIAL_STATUS_INVALID', '2': 4},
    {'1': 'CREDENTIAL_STATUS_REVOKED', '2': 5},
    {'1': 'CREDENTIAL_STATUS_DISABLED', '2': 6},
  ],
};

/// Descriptor for `CredentialStatus`. Decode as a `google.protobuf.EnumDescriptorProto`.
final $typed_data.Uint8List credentialStatusDescriptor = $convert.base64Decode(
    'ChBDcmVkZW50aWFsU3RhdHVzEiEKHUNSRURFTlRJQUxfU1RBVFVTX1VOU1BFQ0lGSUVEEAASHA'
    'oYQ1JFREVOVElBTF9TVEFUVVNfU1RBR0VEEAESHAoYQ1JFREVOVElBTF9TVEFUVVNfQUNUSVZF'
    'EAISIgoeQ1JFREVOVElBTF9TVEFUVVNfQ09PTElOR19ET1dOEAMSHQoZQ1JFREVOVElBTF9TVE'
    'FUVVNfSU5WQUxJRBAEEh0KGUNSRURFTlRJQUxfU1RBVFVTX1JFVk9LRUQQBRIeChpDUkVERU5U'
    'SUFMX1NUQVRVU19ESVNBQkxFRBAG');

@$core.Deprecated('Use agentTypeDescriptor instead')
const AgentType$json = {
  '1': 'AgentType',
  '2': [
    {'1': 'AGENT_TYPE_UNSPECIFIED', '2': 0},
    {'1': 'AGENT_TYPE_OPENCODE', '2': 1},
    {'1': 'AGENT_TYPE_CODEX', '2': 2},
  ],
};

/// Descriptor for `AgentType`. Decode as a `google.protobuf.EnumDescriptorProto`.
final $typed_data.Uint8List agentTypeDescriptor = $convert.base64Decode(
    'CglBZ2VudFR5cGUSGgoWQUdFTlRfVFlQRV9VTlNQRUNJRklFRBAAEhcKE0FHRU5UX1RZUEVfT1'
    'BFTkNPREUQARIUChBBR0VOVF9UWVBFX0NPREVYEAI=');

@$core.Deprecated('Use envelopeHeaderDescriptor instead')
const EnvelopeHeader$json = {
  '1': 'EnvelopeHeader',
  '2': [
    {'1': 'protocol_version', '3': 1, '4': 1, '5': 13, '10': 'protocolVersion'},
    {'1': 'sender_id', '3': 2, '4': 1, '5': 9, '10': 'senderId'},
    {'1': 'recipient_id', '3': 3, '4': 1, '5': 9, '10': 'recipientId'},
    {'1': 'boot_epoch', '3': 4, '4': 1, '5': 4, '10': 'bootEpoch'},
    {'1': 'sequence', '3': 5, '4': 1, '5': 4, '10': 'sequence'},
    {'1': 'timestamp_ms', '3': 6, '4': 1, '5': 3, '10': 'timestampMs'},
    {'1': 'idempotency_key', '3': 7, '4': 1, '5': 9, '10': 'idempotencyKey'},
    {
      '1': 'capability_version',
      '3': 8,
      '4': 1,
      '5': 13,
      '10': 'capabilityVersion'
    },
    {'1': 'request_id', '3': 9, '4': 1, '5': 9, '10': 'requestId'},
    {'1': 'expires_at_ms', '3': 10, '4': 1, '5': 3, '10': 'expiresAtMs'},
  ],
};

/// Descriptor for `EnvelopeHeader`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List envelopeHeaderDescriptor = $convert.base64Decode(
    'Cg5FbnZlbG9wZUhlYWRlchIpChBwcm90b2NvbF92ZXJzaW9uGAEgASgNUg9wcm90b2NvbFZlcn'
    'Npb24SGwoJc2VuZGVyX2lkGAIgASgJUghzZW5kZXJJZBIhCgxyZWNpcGllbnRfaWQYAyABKAlS'
    'C3JlY2lwaWVudElkEh0KCmJvb3RfZXBvY2gYBCABKARSCWJvb3RFcG9jaBIaCghzZXF1ZW5jZR'
    'gFIAEoBFIIc2VxdWVuY2USIQoMdGltZXN0YW1wX21zGAYgASgDUgt0aW1lc3RhbXBNcxInCg9p'
    'ZGVtcG90ZW5jeV9rZXkYByABKAlSDmlkZW1wb3RlbmN5S2V5Ei0KEmNhcGFiaWxpdHlfdmVyc2'
    'lvbhgIIAEoDVIRY2FwYWJpbGl0eVZlcnNpb24SHQoKcmVxdWVzdF9pZBgJIAEoCVIJcmVxdWVz'
    'dElkEiIKDWV4cGlyZXNfYXRfbXMYCiABKANSC2V4cGlyZXNBdE1z');

@$core.Deprecated('Use muxportEnvelopeDescriptor instead')
const MuxportEnvelope$json = {
  '1': 'MuxportEnvelope',
  '2': [
    {
      '1': 'header',
      '3': 1,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.EnvelopeHeader',
      '10': 'header'
    },
    {
      '1': 'command',
      '3': 2,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.Command',
      '9': 0,
      '10': 'command'
    },
    {
      '1': 'command_result',
      '3': 3,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.CommandResult',
      '9': 0,
      '10': 'commandResult'
    },
    {
      '1': 'event',
      '3': 4,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.Event',
      '9': 0,
      '10': 'event'
    },
    {
      '1': 'snapshot',
      '3': 5,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.HostSnapshot',
      '9': 0,
      '10': 'snapshot'
    },
    {
      '1': 'ack',
      '3': 6,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.Ack',
      '9': 0,
      '10': 'ack'
    },
    {
      '1': 'error',
      '3': 7,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.ErrorPayload',
      '9': 0,
      '10': 'error'
    },
  ],
  '8': [
    {'1': 'payload'},
  ],
};

/// Descriptor for `MuxportEnvelope`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List muxportEnvelopeDescriptor = $convert.base64Decode(
    'Cg9NdXhwb3J0RW52ZWxvcGUSOwoGaGVhZGVyGAEgASgLMiMubXV4cG9ydC5wcm90b2NvbC52MS'
    '5FbnZlbG9wZUhlYWRlclIGaGVhZGVyEjgKB2NvbW1hbmQYAiABKAsyHC5tdXhwb3J0LnByb3Rv'
    'Y29sLnYxLkNvbW1hbmRIAFIHY29tbWFuZBJLCg5jb21tYW5kX3Jlc3VsdBgDIAEoCzIiLm11eH'
    'BvcnQucHJvdG9jb2wudjEuQ29tbWFuZFJlc3VsdEgAUg1jb21tYW5kUmVzdWx0EjIKBWV2ZW50'
    'GAQgASgLMhoubXV4cG9ydC5wcm90b2NvbC52MS5FdmVudEgAUgVldmVudBI/CghzbmFwc2hvdB'
    'gFIAEoCzIhLm11eHBvcnQucHJvdG9jb2wudjEuSG9zdFNuYXBzaG90SABSCHNuYXBzaG90EiwK'
    'A2FjaxgGIAEoCzIYLm11eHBvcnQucHJvdG9jb2wudjEuQWNrSABSA2FjaxI5CgVlcnJvchgHIA'
    'EoCzIhLm11eHBvcnQucHJvdG9jb2wudjEuRXJyb3JQYXlsb2FkSABSBWVycm9yQgkKB3BheWxv'
    'YWQ=');

@$core.Deprecated('Use commandDescriptor instead')
const Command$json = {
  '1': 'Command',
  '2': [
    {'1': 'command_id', '3': 1, '4': 1, '5': 9, '10': 'commandId'},
    {'1': 'deadline_ms', '3': 2, '4': 1, '5': 3, '10': 'deadlineMs'},
    {
      '1': 'start_session',
      '3': 3,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.StartSessionCmd',
      '9': 0,
      '10': 'startSession'
    },
    {
      '1': 'send_input',
      '3': 4,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.SendInputCmd',
      '9': 0,
      '10': 'sendInput'
    },
    {
      '1': 'steer',
      '3': 5,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.SteerCmd',
      '9': 0,
      '10': 'steer'
    },
    {
      '1': 'interrupt',
      '3': 6,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.InterruptCmd',
      '9': 0,
      '10': 'interrupt'
    },
    {
      '1': 'approve_action',
      '3': 7,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.ApproveActionCmd',
      '9': 0,
      '10': 'approveAction'
    },
    {
      '1': 'change_assignment',
      '3': 8,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.ChangeAssignmentCmd',
      '9': 0,
      '10': 'changeAssignment'
    },
    {
      '1': 'rotate_credential',
      '3': 9,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.RotateCredentialCmd',
      '9': 0,
      '10': 'rotateCredential'
    },
    {
      '1': 'probe_host',
      '3': 10,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.ProbeHostCmd',
      '9': 0,
      '10': 'probeHost'
    },
    {
      '1': 'query_operation',
      '3': 11,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.QueryOperationCmd',
      '9': 0,
      '10': 'queryOperation'
    },
    {
      '1': 'provision_credential',
      '3': 12,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.ProvisionCredentialCmd',
      '9': 0,
      '10': 'provisionCredential'
    },
    {
      '1': 'list_rotation_pools',
      '3': 13,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.ListRotationPoolsCmd',
      '9': 0,
      '10': 'listRotationPools'
    },
    {
      '1': 'upsert_rotation_pool',
      '3': 14,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.UpsertRotationPoolCmd',
      '9': 0,
      '10': 'upsertRotationPool'
    },
  ],
  '8': [
    {'1': 'inner'},
  ],
};

/// Descriptor for `Command`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List commandDescriptor = $convert.base64Decode(
    'CgdDb21tYW5kEh0KCmNvbW1hbmRfaWQYASABKAlSCWNvbW1hbmRJZBIfCgtkZWFkbGluZV9tcx'
    'gCIAEoA1IKZGVhZGxpbmVNcxJLCg1zdGFydF9zZXNzaW9uGAMgASgLMiQubXV4cG9ydC5wcm90'
    'b2NvbC52MS5TdGFydFNlc3Npb25DbWRIAFIMc3RhcnRTZXNzaW9uEkIKCnNlbmRfaW5wdXQYBC'
    'ABKAsyIS5tdXhwb3J0LnByb3RvY29sLnYxLlNlbmRJbnB1dENtZEgAUglzZW5kSW5wdXQSNQoF'
    'c3RlZXIYBSABKAsyHS5tdXhwb3J0LnByb3RvY29sLnYxLlN0ZWVyQ21kSABSBXN0ZWVyEkEKCW'
    'ludGVycnVwdBgGIAEoCzIhLm11eHBvcnQucHJvdG9jb2wudjEuSW50ZXJydXB0Q21kSABSCWlu'
    'dGVycnVwdBJOCg5hcHByb3ZlX2FjdGlvbhgHIAEoCzIlLm11eHBvcnQucHJvdG9jb2wudjEuQX'
    'Bwcm92ZUFjdGlvbkNtZEgAUg1hcHByb3ZlQWN0aW9uElcKEWNoYW5nZV9hc3NpZ25tZW50GAgg'
    'ASgLMigubXV4cG9ydC5wcm90b2NvbC52MS5DaGFuZ2VBc3NpZ25tZW50Q21kSABSEGNoYW5nZU'
    'Fzc2lnbm1lbnQSVwoRcm90YXRlX2NyZWRlbnRpYWwYCSABKAsyKC5tdXhwb3J0LnByb3RvY29s'
    'LnYxLlJvdGF0ZUNyZWRlbnRpYWxDbWRIAFIQcm90YXRlQ3JlZGVudGlhbBJCCgpwcm9iZV9ob3'
    'N0GAogASgLMiEubXV4cG9ydC5wcm90b2NvbC52MS5Qcm9iZUhvc3RDbWRIAFIJcHJvYmVIb3N0'
    'ElEKD3F1ZXJ5X29wZXJhdGlvbhgLIAEoCzImLm11eHBvcnQucHJvdG9jb2wudjEuUXVlcnlPcG'
    'VyYXRpb25DbWRIAFIOcXVlcnlPcGVyYXRpb24SYAoUcHJvdmlzaW9uX2NyZWRlbnRpYWwYDCAB'
    'KAsyKy5tdXhwb3J0LnByb3RvY29sLnYxLlByb3Zpc2lvbkNyZWRlbnRpYWxDbWRIAFITcHJvdm'
    'lzaW9uQ3JlZGVudGlhbBJbChNsaXN0X3JvdGF0aW9uX3Bvb2xzGA0gASgLMikubXV4cG9ydC5w'
    'cm90b2NvbC52MS5MaXN0Um90YXRpb25Qb29sc0NtZEgAUhFsaXN0Um90YXRpb25Qb29scxJeCh'
    'R1cHNlcnRfcm90YXRpb25fcG9vbBgOIAEoCzIqLm11eHBvcnQucHJvdG9jb2wudjEuVXBzZXJ0'
    'Um90YXRpb25Qb29sQ21kSABSEnVwc2VydFJvdGF0aW9uUG9vbEIHCgVpbm5lcg==');

@$core.Deprecated('Use startSessionCmdDescriptor instead')
const StartSessionCmd$json = {
  '1': 'StartSessionCmd',
  '2': [
    {'1': 'runtime_id', '3': 1, '4': 1, '5': 9, '10': 'runtimeId'},
    {'1': 'project_path', '3': 2, '4': 1, '5': 9, '10': 'projectPath'},
    {'1': 'prompt', '3': 3, '4': 1, '5': 9, '10': 'prompt'},
    {
      '1': 'credential_profile_id',
      '3': 4,
      '4': 1,
      '5': 9,
      '10': 'credentialProfileId'
    },
  ],
};

/// Descriptor for `StartSessionCmd`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List startSessionCmdDescriptor = $convert.base64Decode(
    'Cg9TdGFydFNlc3Npb25DbWQSHQoKcnVudGltZV9pZBgBIAEoCVIJcnVudGltZUlkEiEKDHByb2'
    'plY3RfcGF0aBgCIAEoCVILcHJvamVjdFBhdGgSFgoGcHJvbXB0GAMgASgJUgZwcm9tcHQSMgoV'
    'Y3JlZGVudGlhbF9wcm9maWxlX2lkGAQgASgJUhNjcmVkZW50aWFsUHJvZmlsZUlk');

@$core.Deprecated('Use sendInputCmdDescriptor instead')
const SendInputCmd$json = {
  '1': 'SendInputCmd',
  '2': [
    {'1': 'session_id', '3': 1, '4': 1, '5': 9, '10': 'sessionId'},
    {'1': 'text', '3': 2, '4': 1, '5': 9, '10': 'text'},
    {'1': 'runtime_id', '3': 3, '4': 1, '5': 9, '10': 'runtimeId'},
  ],
};

/// Descriptor for `SendInputCmd`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List sendInputCmdDescriptor = $convert.base64Decode(
    'CgxTZW5kSW5wdXRDbWQSHQoKc2Vzc2lvbl9pZBgBIAEoCVIJc2Vzc2lvbklkEhIKBHRleHQYAi'
    'ABKAlSBHRleHQSHQoKcnVudGltZV9pZBgDIAEoCVIJcnVudGltZUlk');

@$core.Deprecated('Use steerCmdDescriptor instead')
const SteerCmd$json = {
  '1': 'SteerCmd',
  '2': [
    {'1': 'session_id', '3': 1, '4': 1, '5': 9, '10': 'sessionId'},
    {'1': 'instruction', '3': 2, '4': 1, '5': 9, '10': 'instruction'},
    {'1': 'runtime_id', '3': 3, '4': 1, '5': 9, '10': 'runtimeId'},
  ],
};

/// Descriptor for `SteerCmd`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List steerCmdDescriptor = $convert.base64Decode(
    'CghTdGVlckNtZBIdCgpzZXNzaW9uX2lkGAEgASgJUglzZXNzaW9uSWQSIAoLaW5zdHJ1Y3Rpb2'
    '4YAiABKAlSC2luc3RydWN0aW9uEh0KCnJ1bnRpbWVfaWQYAyABKAlSCXJ1bnRpbWVJZA==');

@$core.Deprecated('Use interruptCmdDescriptor instead')
const InterruptCmd$json = {
  '1': 'InterruptCmd',
  '2': [
    {'1': 'session_id', '3': 1, '4': 1, '5': 9, '10': 'sessionId'},
    {'1': 'reason', '3': 2, '4': 1, '5': 9, '10': 'reason'},
    {'1': 'runtime_id', '3': 3, '4': 1, '5': 9, '10': 'runtimeId'},
  ],
};

/// Descriptor for `InterruptCmd`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List interruptCmdDescriptor = $convert.base64Decode(
    'CgxJbnRlcnJ1cHRDbWQSHQoKc2Vzc2lvbl9pZBgBIAEoCVIJc2Vzc2lvbklkEhYKBnJlYXNvbh'
    'gCIAEoCVIGcmVhc29uEh0KCnJ1bnRpbWVfaWQYAyABKAlSCXJ1bnRpbWVJZA==');

@$core.Deprecated('Use approveActionCmdDescriptor instead')
const ApproveActionCmd$json = {
  '1': 'ApproveActionCmd',
  '2': [
    {'1': 'approval_id', '3': 1, '4': 1, '5': 9, '10': 'approvalId'},
    {'1': 'approved', '3': 2, '4': 1, '5': 8, '10': 'approved'},
    {'1': 'decision_reason', '3': 3, '4': 1, '5': 9, '10': 'decisionReason'},
    {'1': 'session_id', '3': 4, '4': 1, '5': 9, '10': 'sessionId'},
    {'1': 'runtime_id', '3': 5, '4': 1, '5': 9, '10': 'runtimeId'},
  ],
};

/// Descriptor for `ApproveActionCmd`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List approveActionCmdDescriptor = $convert.base64Decode(
    'ChBBcHByb3ZlQWN0aW9uQ21kEh8KC2FwcHJvdmFsX2lkGAEgASgJUgphcHByb3ZhbElkEhoKCG'
    'FwcHJvdmVkGAIgASgIUghhcHByb3ZlZBInCg9kZWNpc2lvbl9yZWFzb24YAyABKAlSDmRlY2lz'
    'aW9uUmVhc29uEh0KCnNlc3Npb25faWQYBCABKAlSCXNlc3Npb25JZBIdCgpydW50aW1lX2lkGA'
    'UgASgJUglydW50aW1lSWQ=');

@$core.Deprecated('Use changeAssignmentCmdDescriptor instead')
const ChangeAssignmentCmd$json = {
  '1': 'ChangeAssignmentCmd',
  '2': [
    {'1': 'target_type', '3': 1, '4': 1, '5': 9, '10': 'targetType'},
    {'1': 'target_id', '3': 2, '4': 1, '5': 9, '10': 'targetId'},
    {
      '1': 'new_credential_profile_id',
      '3': 3,
      '4': 1,
      '5': 9,
      '10': 'newCredentialProfileId'
    },
  ],
};

/// Descriptor for `ChangeAssignmentCmd`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List changeAssignmentCmdDescriptor = $convert.base64Decode(
    'ChNDaGFuZ2VBc3NpZ25tZW50Q21kEh8KC3RhcmdldF90eXBlGAEgASgJUgp0YXJnZXRUeXBlEh'
    'sKCXRhcmdldF9pZBgCIAEoCVIIdGFyZ2V0SWQSOQoZbmV3X2NyZWRlbnRpYWxfcHJvZmlsZV9p'
    'ZBgDIAEoCVIWbmV3Q3JlZGVudGlhbFByb2ZpbGVJZA==');

@$core.Deprecated('Use rotateCredentialCmdDescriptor instead')
const RotateCredentialCmd$json = {
  '1': 'RotateCredentialCmd',
  '2': [
    {'1': 'pool_id', '3': 1, '4': 1, '5': 9, '10': 'poolId'},
    {'1': 'target_runtime_id', '3': 2, '4': 1, '5': 9, '10': 'targetRuntimeId'},
    {'1': 'force', '3': 3, '4': 1, '5': 8, '10': 'force'},
  ],
};

/// Descriptor for `RotateCredentialCmd`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List rotateCredentialCmdDescriptor = $convert.base64Decode(
    'ChNSb3RhdGVDcmVkZW50aWFsQ21kEhcKB3Bvb2xfaWQYASABKAlSBnBvb2xJZBIqChF0YXJnZX'
    'RfcnVudGltZV9pZBgCIAEoCVIPdGFyZ2V0UnVudGltZUlkEhQKBWZvcmNlGAMgASgIUgVmb3Jj'
    'ZQ==');

@$core.Deprecated('Use probeHostCmdDescriptor instead')
const ProbeHostCmd$json = {
  '1': 'ProbeHostCmd',
};

/// Descriptor for `ProbeHostCmd`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List probeHostCmdDescriptor =
    $convert.base64Decode('CgxQcm9iZUhvc3RDbWQ=');

@$core.Deprecated('Use queryOperationCmdDescriptor instead')
const QueryOperationCmd$json = {
  '1': 'QueryOperationCmd',
  '2': [
    {'1': 'idempotency_key', '3': 1, '4': 1, '5': 9, '10': 'idempotencyKey'},
  ],
};

/// Descriptor for `QueryOperationCmd`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List queryOperationCmdDescriptor = $convert.base64Decode(
    'ChFRdWVyeU9wZXJhdGlvbkNtZBInCg9pZGVtcG90ZW5jeV9rZXkYASABKAlSDmlkZW1wb3Rlbm'
    'N5S2V5');

@$core.Deprecated('Use listRotationPoolsCmdDescriptor instead')
const ListRotationPoolsCmd$json = {
  '1': 'ListRotationPoolsCmd',
};

/// Descriptor for `ListRotationPoolsCmd`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List listRotationPoolsCmdDescriptor =
    $convert.base64Decode('ChRMaXN0Um90YXRpb25Qb29sc0NtZA==');

@$core.Deprecated('Use upsertRotationPoolCmdDescriptor instead')
const UpsertRotationPoolCmd$json = {
  '1': 'UpsertRotationPoolCmd',
  '2': [
    {'1': 'pool_id', '3': 1, '4': 1, '5': 9, '10': 'poolId'},
    {'1': 'provider_id', '3': 2, '4': 1, '5': 9, '10': 'providerId'},
    {
      '1': 'ordered_profile_ids',
      '3': 3,
      '4': 3,
      '5': 9,
      '10': 'orderedProfileIds'
    },
    {'1': 'mode', '3': 4, '4': 1, '5': 9, '10': 'mode'},
    {'1': 'cooldown_ms', '3': 5, '4': 1, '5': 3, '10': 'cooldownMs'},
    {
      '1': 'max_switches_per_hour',
      '3': 6,
      '4': 1,
      '5': 13,
      '10': 'maxSwitchesPerHour'
    },
    {'1': 'allowed_host_ids', '3': 7, '4': 3, '5': 9, '10': 'allowedHostIds'},
    {
      '1': 'quota_failover_enabled',
      '3': 8,
      '4': 1,
      '5': 8,
      '10': 'quotaFailoverEnabled'
    },
  ],
};

/// Descriptor for `UpsertRotationPoolCmd`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List upsertRotationPoolCmdDescriptor = $convert.base64Decode(
    'ChVVcHNlcnRSb3RhdGlvblBvb2xDbWQSFwoHcG9vbF9pZBgBIAEoCVIGcG9vbElkEh8KC3Byb3'
    'ZpZGVyX2lkGAIgASgJUgpwcm92aWRlcklkEi4KE29yZGVyZWRfcHJvZmlsZV9pZHMYAyADKAlS'
    'EW9yZGVyZWRQcm9maWxlSWRzEhIKBG1vZGUYBCABKAlSBG1vZGUSHwoLY29vbGRvd25fbXMYBS'
    'ABKANSCmNvb2xkb3duTXMSMQoVbWF4X3N3aXRjaGVzX3Blcl9ob3VyGAYgASgNUhJtYXhTd2l0'
    'Y2hlc1BlckhvdXISKAoQYWxsb3dlZF9ob3N0X2lkcxgHIAMoCVIOYWxsb3dlZEhvc3RJZHMSNA'
    'oWcXVvdGFfZmFpbG92ZXJfZW5hYmxlZBgIIAEoCFIUcXVvdGFGYWlsb3ZlckVuYWJsZWQ=');

@$core.Deprecated('Use provisionCredentialCmdDescriptor instead')
const ProvisionCredentialCmd$json = {
  '1': 'ProvisionCredentialCmd',
  '2': [
    {'1': 'profile_id', '3': 1, '4': 1, '5': 9, '10': 'profileId'},
    {'1': 'display_name', '3': 2, '4': 1, '5': 9, '10': 'displayName'},
    {'1': 'provider', '3': 3, '4': 1, '5': 9, '10': 'provider'},
    {'1': 'credential_type', '3': 4, '4': 1, '5': 9, '10': 'credentialType'},
    {
      '1': 'account_fingerprint',
      '3': 5,
      '4': 1,
      '5': 9,
      '10': 'accountFingerprint'
    },
    {'1': 'secret_nonce', '3': 6, '4': 1, '5': 12, '10': 'secretNonce'},
    {
      '1': 'secret_ciphertext',
      '3': 7,
      '4': 1,
      '5': 12,
      '10': 'secretCiphertext'
    },
  ],
};

/// Descriptor for `ProvisionCredentialCmd`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List provisionCredentialCmdDescriptor = $convert.base64Decode(
    'ChZQcm92aXNpb25DcmVkZW50aWFsQ21kEh0KCnByb2ZpbGVfaWQYASABKAlSCXByb2ZpbGVJZB'
    'IhCgxkaXNwbGF5X25hbWUYAiABKAlSC2Rpc3BsYXlOYW1lEhoKCHByb3ZpZGVyGAMgASgJUghw'
    'cm92aWRlchInCg9jcmVkZW50aWFsX3R5cGUYBCABKAlSDmNyZWRlbnRpYWxUeXBlEi8KE2FjY2'
    '91bnRfZmluZ2VycHJpbnQYBSABKAlSEmFjY291bnRGaW5nZXJwcmludBIhCgxzZWNyZXRfbm9u'
    'Y2UYBiABKAxSC3NlY3JldE5vbmNlEisKEXNlY3JldF9jaXBoZXJ0ZXh0GAcgASgMUhBzZWNyZX'
    'RDaXBoZXJ0ZXh0');

@$core.Deprecated('Use commandResultDescriptor instead')
const CommandResult$json = {
  '1': 'CommandResult',
  '2': [
    {'1': 'command_id', '3': 1, '4': 1, '5': 9, '10': 'commandId'},
    {
      '1': 'state',
      '3': 2,
      '4': 1,
      '5': 14,
      '6': '.muxport.protocol.v1.RemoteOpState',
      '10': 'state'
    },
    {'1': 'success', '3': 3, '4': 1, '5': 8, '10': 'success'},
    {'1': 'error_message', '3': 4, '4': 1, '5': 9, '10': 'errorMessage'},
    {'1': 'completed_at_ms', '3': 5, '4': 1, '5': 3, '10': 'completedAtMs'},
    {'1': 'result_json', '3': 6, '4': 1, '5': 9, '10': 'resultJson'},
  ],
};

/// Descriptor for `CommandResult`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List commandResultDescriptor = $convert.base64Decode(
    'Cg1Db21tYW5kUmVzdWx0Eh0KCmNvbW1hbmRfaWQYASABKAlSCWNvbW1hbmRJZBI4CgVzdGF0ZR'
    'gCIAEoDjIiLm11eHBvcnQucHJvdG9jb2wudjEuUmVtb3RlT3BTdGF0ZVIFc3RhdGUSGAoHc3Vj'
    'Y2VzcxgDIAEoCFIHc3VjY2VzcxIjCg1lcnJvcl9tZXNzYWdlGAQgASgJUgxlcnJvck1lc3NhZ2'
    'USJgoPY29tcGxldGVkX2F0X21zGAUgASgDUg1jb21wbGV0ZWRBdE1zEh8KC3Jlc3VsdF9qc29u'
    'GAYgASgJUgpyZXN1bHRKc29u');

@$core.Deprecated('Use eventDescriptor instead')
const Event$json = {
  '1': 'Event',
  '2': [
    {'1': 'event_id', '3': 1, '4': 1, '5': 9, '10': 'eventId'},
    {'1': 'timestamp_ms', '3': 2, '4': 1, '5': 3, '10': 'timestampMs'},
    {
      '1': 'host_status',
      '3': 3,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.HostStatusEvent',
      '9': 0,
      '10': 'hostStatus'
    },
    {
      '1': 'runtime_state',
      '3': 4,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.RuntimeStateEvent',
      '9': 0,
      '10': 'runtimeState'
    },
    {
      '1': 'session_updated',
      '3': 5,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.SessionUpdatedEvent',
      '9': 0,
      '10': 'sessionUpdated'
    },
    {
      '1': 'stream_delta',
      '3': 6,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.StreamDeltaEvent',
      '9': 0,
      '10': 'streamDelta'
    },
    {
      '1': 'approval_requested',
      '3': 7,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.ApprovalRequestedEvent',
      '9': 0,
      '10': 'approvalRequested'
    },
    {
      '1': 'approval_resolved',
      '3': 8,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.ApprovalResolvedEvent',
      '9': 0,
      '10': 'approvalResolved'
    },
    {
      '1': 'credential_rotated',
      '3': 9,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.CredentialRotatedEvent',
      '9': 0,
      '10': 'credentialRotated'
    },
    {
      '1': 'audit_logged',
      '3': 10,
      '4': 1,
      '5': 11,
      '6': '.muxport.protocol.v1.AuditLoggedEvent',
      '9': 0,
      '10': 'auditLogged'
    },
  ],
  '8': [
    {'1': 'inner'},
  ],
};

/// Descriptor for `Event`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List eventDescriptor = $convert.base64Decode(
    'CgVFdmVudBIZCghldmVudF9pZBgBIAEoCVIHZXZlbnRJZBIhCgx0aW1lc3RhbXBfbXMYAiABKA'
    'NSC3RpbWVzdGFtcE1zEkcKC2hvc3Rfc3RhdHVzGAMgASgLMiQubXV4cG9ydC5wcm90b2NvbC52'
    'MS5Ib3N0U3RhdHVzRXZlbnRIAFIKaG9zdFN0YXR1cxJNCg1ydW50aW1lX3N0YXRlGAQgASgLMi'
    'YubXV4cG9ydC5wcm90b2NvbC52MS5SdW50aW1lU3RhdGVFdmVudEgAUgxydW50aW1lU3RhdGUS'
    'UwoPc2Vzc2lvbl91cGRhdGVkGAUgASgLMigubXV4cG9ydC5wcm90b2NvbC52MS5TZXNzaW9uVX'
    'BkYXRlZEV2ZW50SABSDnNlc3Npb25VcGRhdGVkEkoKDHN0cmVhbV9kZWx0YRgGIAEoCzIlLm11'
    'eHBvcnQucHJvdG9jb2wudjEuU3RyZWFtRGVsdGFFdmVudEgAUgtzdHJlYW1EZWx0YRJcChJhcH'
    'Byb3ZhbF9yZXF1ZXN0ZWQYByABKAsyKy5tdXhwb3J0LnByb3RvY29sLnYxLkFwcHJvdmFsUmVx'
    'dWVzdGVkRXZlbnRIAFIRYXBwcm92YWxSZXF1ZXN0ZWQSWQoRYXBwcm92YWxfcmVzb2x2ZWQYCC'
    'ABKAsyKi5tdXhwb3J0LnByb3RvY29sLnYxLkFwcHJvdmFsUmVzb2x2ZWRFdmVudEgAUhBhcHBy'
    'b3ZhbFJlc29sdmVkElwKEmNyZWRlbnRpYWxfcm90YXRlZBgJIAEoCzIrLm11eHBvcnQucHJvdG'
    '9jb2wudjEuQ3JlZGVudGlhbFJvdGF0ZWRFdmVudEgAUhFjcmVkZW50aWFsUm90YXRlZBJKCgxh'
    'dWRpdF9sb2dnZWQYCiABKAsyJS5tdXhwb3J0LnByb3RvY29sLnYxLkF1ZGl0TG9nZ2VkRXZlbn'
    'RIAFILYXVkaXRMb2dnZWRCBwoFaW5uZXI=');

@$core.Deprecated('Use hostStatusEventDescriptor instead')
const HostStatusEvent$json = {
  '1': 'HostStatusEvent',
  '2': [
    {'1': 'host_id', '3': 1, '4': 1, '5': 9, '10': 'hostId'},
    {
      '1': 'state',
      '3': 2,
      '4': 1,
      '5': 14,
      '6': '.muxport.protocol.v1.ConnectorState',
      '10': 'state'
    },
    {'1': 'version', '3': 3, '4': 1, '5': 9, '10': 'version'},
  ],
};

/// Descriptor for `HostStatusEvent`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List hostStatusEventDescriptor = $convert.base64Decode(
    'Cg9Ib3N0U3RhdHVzRXZlbnQSFwoHaG9zdF9pZBgBIAEoCVIGaG9zdElkEjkKBXN0YXRlGAIgAS'
    'gOMiMubXV4cG9ydC5wcm90b2NvbC52MS5Db25uZWN0b3JTdGF0ZVIFc3RhdGUSGAoHdmVyc2lv'
    'bhgDIAEoCVIHdmVyc2lvbg==');

@$core.Deprecated('Use runtimeStateEventDescriptor instead')
const RuntimeStateEvent$json = {
  '1': 'RuntimeStateEvent',
  '2': [
    {'1': 'runtime_id', '3': 1, '4': 1, '5': 9, '10': 'runtimeId'},
    {
      '1': 'agent_type',
      '3': 2,
      '4': 1,
      '5': 14,
      '6': '.muxport.protocol.v1.AgentType',
      '10': 'agentType'
    },
    {
      '1': 'state',
      '3': 3,
      '4': 1,
      '5': 14,
      '6': '.muxport.protocol.v1.RuntimeState',
      '10': 'state'
    },
    {'1': 'active_profile_id', '3': 4, '4': 1, '5': 9, '10': 'activeProfileId'},
    {'1': 'details', '3': 5, '4': 1, '5': 9, '10': 'details'},
  ],
};

/// Descriptor for `RuntimeStateEvent`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List runtimeStateEventDescriptor = $convert.base64Decode(
    'ChFSdW50aW1lU3RhdGVFdmVudBIdCgpydW50aW1lX2lkGAEgASgJUglydW50aW1lSWQSPQoKYW'
    'dlbnRfdHlwZRgCIAEoDjIeLm11eHBvcnQucHJvdG9jb2wudjEuQWdlbnRUeXBlUglhZ2VudFR5'
    'cGUSNwoFc3RhdGUYAyABKA4yIS5tdXhwb3J0LnByb3RvY29sLnYxLlJ1bnRpbWVTdGF0ZVIFc3'
    'RhdGUSKgoRYWN0aXZlX3Byb2ZpbGVfaWQYBCABKAlSD2FjdGl2ZVByb2ZpbGVJZBIYCgdkZXRh'
    'aWxzGAUgASgJUgdkZXRhaWxz');

@$core.Deprecated('Use sessionUpdatedEventDescriptor instead')
const SessionUpdatedEvent$json = {
  '1': 'SessionUpdatedEvent',
  '2': [
    {'1': 'session_id', '3': 1, '4': 1, '5': 9, '10': 'sessionId'},
    {'1': 'runtime_id', '3': 2, '4': 1, '5': 9, '10': 'runtimeId'},
    {'1': 'title', '3': 3, '4': 1, '5': 9, '10': 'title'},
    {'1': 'status', '3': 4, '4': 1, '5': 9, '10': 'status'},
    {
      '1': 'credential_profile_id',
      '3': 5,
      '4': 1,
      '5': 9,
      '10': 'credentialProfileId'
    },
    {'1': 'updated_at_ms', '3': 6, '4': 1, '5': 3, '10': 'updatedAtMs'},
    {'1': 'project_path', '3': 7, '4': 1, '5': 9, '10': 'projectPath'},
  ],
};

/// Descriptor for `SessionUpdatedEvent`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List sessionUpdatedEventDescriptor = $convert.base64Decode(
    'ChNTZXNzaW9uVXBkYXRlZEV2ZW50Eh0KCnNlc3Npb25faWQYASABKAlSCXNlc3Npb25JZBIdCg'
    'pydW50aW1lX2lkGAIgASgJUglydW50aW1lSWQSFAoFdGl0bGUYAyABKAlSBXRpdGxlEhYKBnN0'
    'YXR1cxgEIAEoCVIGc3RhdHVzEjIKFWNyZWRlbnRpYWxfcHJvZmlsZV9pZBgFIAEoCVITY3JlZG'
    'VudGlhbFByb2ZpbGVJZBIiCg11cGRhdGVkX2F0X21zGAYgASgDUgt1cGRhdGVkQXRNcxIhCgxw'
    'cm9qZWN0X3BhdGgYByABKAlSC3Byb2plY3RQYXRo');

@$core.Deprecated('Use streamDeltaEventDescriptor instead')
const StreamDeltaEvent$json = {
  '1': 'StreamDeltaEvent',
  '2': [
    {'1': 'session_id', '3': 1, '4': 1, '5': 9, '10': 'sessionId'},
    {'1': 'turn_id', '3': 2, '4': 1, '5': 9, '10': 'turnId'},
    {'1': 'delta_text', '3': 3, '4': 1, '5': 9, '10': 'deltaText'},
    {'1': 'is_final', '3': 4, '4': 1, '5': 8, '10': 'isFinal'},
  ],
};

/// Descriptor for `StreamDeltaEvent`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List streamDeltaEventDescriptor = $convert.base64Decode(
    'ChBTdHJlYW1EZWx0YUV2ZW50Eh0KCnNlc3Npb25faWQYASABKAlSCXNlc3Npb25JZBIXCgd0dX'
    'JuX2lkGAIgASgJUgZ0dXJuSWQSHQoKZGVsdGFfdGV4dBgDIAEoCVIJZGVsdGFUZXh0EhkKCGlz'
    'X2ZpbmFsGAQgASgIUgdpc0ZpbmFs');

@$core.Deprecated('Use approvalRequestedEventDescriptor instead')
const ApprovalRequestedEvent$json = {
  '1': 'ApprovalRequestedEvent',
  '2': [
    {'1': 'approval_id', '3': 1, '4': 1, '5': 9, '10': 'approvalId'},
    {'1': 'session_id', '3': 2, '4': 1, '5': 9, '10': 'sessionId'},
    {'1': 'action_type', '3': 3, '4': 1, '5': 9, '10': 'actionType'},
    {'1': 'description', '3': 4, '4': 1, '5': 9, '10': 'description'},
    {'1': 'payload_json', '3': 5, '4': 1, '5': 9, '10': 'payloadJson'},
  ],
};

/// Descriptor for `ApprovalRequestedEvent`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List approvalRequestedEventDescriptor = $convert.base64Decode(
    'ChZBcHByb3ZhbFJlcXVlc3RlZEV2ZW50Eh8KC2FwcHJvdmFsX2lkGAEgASgJUgphcHByb3ZhbE'
    'lkEh0KCnNlc3Npb25faWQYAiABKAlSCXNlc3Npb25JZBIfCgthY3Rpb25fdHlwZRgDIAEoCVIK'
    'YWN0aW9uVHlwZRIgCgtkZXNjcmlwdGlvbhgEIAEoCVILZGVzY3JpcHRpb24SIQoMcGF5bG9hZF'
    '9qc29uGAUgASgJUgtwYXlsb2FkSnNvbg==');

@$core.Deprecated('Use approvalResolvedEventDescriptor instead')
const ApprovalResolvedEvent$json = {
  '1': 'ApprovalResolvedEvent',
  '2': [
    {'1': 'approval_id', '3': 1, '4': 1, '5': 9, '10': 'approvalId'},
    {'1': 'approved', '3': 2, '4': 1, '5': 8, '10': 'approved'},
    {'1': 'resolved_by', '3': 3, '4': 1, '5': 9, '10': 'resolvedBy'},
    {'1': 'session_id', '3': 4, '4': 1, '5': 9, '10': 'sessionId'},
  ],
};

/// Descriptor for `ApprovalResolvedEvent`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List approvalResolvedEventDescriptor = $convert.base64Decode(
    'ChVBcHByb3ZhbFJlc29sdmVkRXZlbnQSHwoLYXBwcm92YWxfaWQYASABKAlSCmFwcHJvdmFsSW'
    'QSGgoIYXBwcm92ZWQYAiABKAhSCGFwcHJvdmVkEh8KC3Jlc29sdmVkX2J5GAMgASgJUgpyZXNv'
    'bHZlZEJ5Eh0KCnNlc3Npb25faWQYBCABKAlSCXNlc3Npb25JZA==');

@$core.Deprecated('Use credentialRotatedEventDescriptor instead')
const CredentialRotatedEvent$json = {
  '1': 'CredentialRotatedEvent',
  '2': [
    {'1': 'profile_id', '3': 1, '4': 1, '5': 9, '10': 'profileId'},
    {'1': 'pool_id', '3': 2, '4': 1, '5': 9, '10': 'poolId'},
    {'1': 'runtime_id', '3': 3, '4': 1, '5': 9, '10': 'runtimeId'},
    {'1': 'reason', '3': 4, '4': 1, '5': 9, '10': 'reason'},
  ],
};

/// Descriptor for `CredentialRotatedEvent`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List credentialRotatedEventDescriptor = $convert.base64Decode(
    'ChZDcmVkZW50aWFsUm90YXRlZEV2ZW50Eh0KCnByb2ZpbGVfaWQYASABKAlSCXByb2ZpbGVJZB'
    'IXCgdwb29sX2lkGAIgASgJUgZwb29sSWQSHQoKcnVudGltZV9pZBgDIAEoCVIJcnVudGltZUlk'
    'EhYKBnJlYXNvbhgEIAEoCVIGcmVhc29u');

@$core.Deprecated('Use auditLoggedEventDescriptor instead')
const AuditLoggedEvent$json = {
  '1': 'AuditLoggedEvent',
  '2': [
    {'1': 'audit_id', '3': 1, '4': 1, '5': 9, '10': 'auditId'},
    {'1': 'action', '3': 2, '4': 1, '5': 9, '10': 'action'},
    {'1': 'actor', '3': 3, '4': 1, '5': 9, '10': 'actor'},
    {'1': 'target', '3': 4, '4': 1, '5': 9, '10': 'target'},
    {'1': 'outcome', '3': 5, '4': 1, '5': 9, '10': 'outcome'},
  ],
};

/// Descriptor for `AuditLoggedEvent`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List auditLoggedEventDescriptor = $convert.base64Decode(
    'ChBBdWRpdExvZ2dlZEV2ZW50EhkKCGF1ZGl0X2lkGAEgASgJUgdhdWRpdElkEhYKBmFjdGlvbh'
    'gCIAEoCVIGYWN0aW9uEhQKBWFjdG9yGAMgASgJUgVhY3RvchIWCgZ0YXJnZXQYBCABKAlSBnRh'
    'cmdldBIYCgdvdXRjb21lGAUgASgJUgdvdXRjb21l');

@$core.Deprecated('Use hostSnapshotDescriptor instead')
const HostSnapshot$json = {
  '1': 'HostSnapshot',
  '2': [
    {'1': 'host_id', '3': 1, '4': 1, '5': 9, '10': 'hostId'},
    {'1': 'hostname', '3': 2, '4': 1, '5': 9, '10': 'hostname'},
    {
      '1': 'connector_state',
      '3': 3,
      '4': 1,
      '5': 14,
      '6': '.muxport.protocol.v1.ConnectorState',
      '10': 'connectorState'
    },
    {
      '1': 'runtimes',
      '3': 4,
      '4': 3,
      '5': 11,
      '6': '.muxport.protocol.v1.RuntimeInfo',
      '10': 'runtimes'
    },
    {
      '1': 'credential_profiles',
      '3': 5,
      '4': 3,
      '5': 11,
      '6': '.muxport.protocol.v1.CredentialProfileInfo',
      '10': 'credentialProfiles'
    },
    {
      '1': 'active_sessions',
      '3': 6,
      '4': 3,
      '5': 11,
      '6': '.muxport.protocol.v1.SessionInfo',
      '10': 'activeSessions'
    },
    {
      '1': 'snapshot_sequence',
      '3': 7,
      '4': 1,
      '5': 4,
      '10': 'snapshotSequence'
    },
  ],
};

/// Descriptor for `HostSnapshot`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List hostSnapshotDescriptor = $convert.base64Decode(
    'CgxIb3N0U25hcHNob3QSFwoHaG9zdF9pZBgBIAEoCVIGaG9zdElkEhoKCGhvc3RuYW1lGAIgAS'
    'gJUghob3N0bmFtZRJMCg9jb25uZWN0b3Jfc3RhdGUYAyABKA4yIy5tdXhwb3J0LnByb3RvY29s'
    'LnYxLkNvbm5lY3RvclN0YXRlUg5jb25uZWN0b3JTdGF0ZRI8CghydW50aW1lcxgEIAMoCzIgLm'
    '11eHBvcnQucHJvdG9jb2wudjEuUnVudGltZUluZm9SCHJ1bnRpbWVzElsKE2NyZWRlbnRpYWxf'
    'cHJvZmlsZXMYBSADKAsyKi5tdXhwb3J0LnByb3RvY29sLnYxLkNyZWRlbnRpYWxQcm9maWxlSW'
    '5mb1ISY3JlZGVudGlhbFByb2ZpbGVzEkkKD2FjdGl2ZV9zZXNzaW9ucxgGIAMoCzIgLm11eHBv'
    'cnQucHJvdG9jb2wudjEuU2Vzc2lvbkluZm9SDmFjdGl2ZVNlc3Npb25zEisKEXNuYXBzaG90X3'
    'NlcXVlbmNlGAcgASgEUhBzbmFwc2hvdFNlcXVlbmNl');

@$core.Deprecated('Use runtimeInfoDescriptor instead')
const RuntimeInfo$json = {
  '1': 'RuntimeInfo',
  '2': [
    {'1': 'runtime_id', '3': 1, '4': 1, '5': 9, '10': 'runtimeId'},
    {
      '1': 'agent_type',
      '3': 2,
      '4': 1,
      '5': 14,
      '6': '.muxport.protocol.v1.AgentType',
      '10': 'agentType'
    },
    {'1': 'name', '3': 3, '4': 1, '5': 9, '10': 'name'},
    {
      '1': 'state',
      '3': 4,
      '4': 1,
      '5': 14,
      '6': '.muxport.protocol.v1.RuntimeState',
      '10': 'state'
    },
    {
      '1': 'active_credential_profile_id',
      '3': 5,
      '4': 1,
      '5': 9,
      '10': 'activeCredentialProfileId'
    },
    {'1': 'project_paths', '3': 6, '4': 3, '5': 9, '10': 'projectPaths'},
    {
      '1': 'connector_managed',
      '3': 7,
      '4': 1,
      '5': 8,
      '10': 'connectorManaged'
    },
  ],
};

/// Descriptor for `RuntimeInfo`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List runtimeInfoDescriptor = $convert.base64Decode(
    'CgtSdW50aW1lSW5mbxIdCgpydW50aW1lX2lkGAEgASgJUglydW50aW1lSWQSPQoKYWdlbnRfdH'
    'lwZRgCIAEoDjIeLm11eHBvcnQucHJvdG9jb2wudjEuQWdlbnRUeXBlUglhZ2VudFR5cGUSEgoE'
    'bmFtZRgDIAEoCVIEbmFtZRI3CgVzdGF0ZRgEIAEoDjIhLm11eHBvcnQucHJvdG9jb2wudjEuUn'
    'VudGltZVN0YXRlUgVzdGF0ZRI/ChxhY3RpdmVfY3JlZGVudGlhbF9wcm9maWxlX2lkGAUgASgJ'
    'UhlhY3RpdmVDcmVkZW50aWFsUHJvZmlsZUlkEiMKDXByb2plY3RfcGF0aHMYBiADKAlSDHByb2'
    'plY3RQYXRocxIrChFjb25uZWN0b3JfbWFuYWdlZBgHIAEoCFIQY29ubmVjdG9yTWFuYWdlZA==');

@$core.Deprecated('Use credentialProfileInfoDescriptor instead')
const CredentialProfileInfo$json = {
  '1': 'CredentialProfileInfo',
  '2': [
    {'1': 'profile_id', '3': 1, '4': 1, '5': 9, '10': 'profileId'},
    {'1': 'display_name', '3': 2, '4': 1, '5': 9, '10': 'displayName'},
    {'1': 'provider', '3': 3, '4': 1, '5': 9, '10': 'provider'},
    {
      '1': 'account_fingerprint',
      '3': 4,
      '4': 1,
      '5': 9,
      '10': 'accountFingerprint'
    },
    {
      '1': 'status',
      '3': 5,
      '4': 1,
      '5': 14,
      '6': '.muxport.protocol.v1.CredentialStatus',
      '10': 'status'
    },
    {
      '1': 'last_validated_at_ms',
      '3': 6,
      '4': 1,
      '5': 3,
      '10': 'lastValidatedAtMs'
    },
  ],
};

/// Descriptor for `CredentialProfileInfo`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List credentialProfileInfoDescriptor = $convert.base64Decode(
    'ChVDcmVkZW50aWFsUHJvZmlsZUluZm8SHQoKcHJvZmlsZV9pZBgBIAEoCVIJcHJvZmlsZUlkEi'
    'EKDGRpc3BsYXlfbmFtZRgCIAEoCVILZGlzcGxheU5hbWUSGgoIcHJvdmlkZXIYAyABKAlSCHBy'
    'b3ZpZGVyEi8KE2FjY291bnRfZmluZ2VycHJpbnQYBCABKAlSEmFjY291bnRGaW5nZXJwcmludB'
    'I9CgZzdGF0dXMYBSABKA4yJS5tdXhwb3J0LnByb3RvY29sLnYxLkNyZWRlbnRpYWxTdGF0dXNS'
    'BnN0YXR1cxIvChRsYXN0X3ZhbGlkYXRlZF9hdF9tcxgGIAEoA1IRbGFzdFZhbGlkYXRlZEF0TX'
    'M=');

@$core.Deprecated('Use sessionInfoDescriptor instead')
const SessionInfo$json = {
  '1': 'SessionInfo',
  '2': [
    {'1': 'session_id', '3': 1, '4': 1, '5': 9, '10': 'sessionId'},
    {'1': 'runtime_id', '3': 2, '4': 1, '5': 9, '10': 'runtimeId'},
    {'1': 'project_path', '3': 3, '4': 1, '5': 9, '10': 'projectPath'},
    {'1': 'title', '3': 4, '4': 1, '5': 9, '10': 'title'},
    {
      '1': 'credential_profile_id',
      '3': 5,
      '4': 1,
      '5': 9,
      '10': 'credentialProfileId'
    },
    {'1': 'status', '3': 6, '4': 1, '5': 9, '10': 'status'},
  ],
};

/// Descriptor for `SessionInfo`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List sessionInfoDescriptor = $convert.base64Decode(
    'CgtTZXNzaW9uSW5mbxIdCgpzZXNzaW9uX2lkGAEgASgJUglzZXNzaW9uSWQSHQoKcnVudGltZV'
    '9pZBgCIAEoCVIJcnVudGltZUlkEiEKDHByb2plY3RfcGF0aBgDIAEoCVILcHJvamVjdFBhdGgS'
    'FAoFdGl0bGUYBCABKAlSBXRpdGxlEjIKFWNyZWRlbnRpYWxfcHJvZmlsZV9pZBgFIAEoCVITY3'
    'JlZGVudGlhbFByb2ZpbGVJZBIWCgZzdGF0dXMYBiABKAlSBnN0YXR1cw==');

@$core.Deprecated('Use ackDescriptor instead')
const Ack$json = {
  '1': 'Ack',
  '2': [
    {
      '1': 'sequence_acknowledged',
      '3': 1,
      '4': 1,
      '5': 4,
      '10': 'sequenceAcknowledged'
    },
    {'1': 'boot_epoch', '3': 2, '4': 1, '5': 4, '10': 'bootEpoch'},
  ],
};

/// Descriptor for `Ack`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List ackDescriptor = $convert.base64Decode(
    'CgNBY2sSMwoVc2VxdWVuY2VfYWNrbm93bGVkZ2VkGAEgASgEUhRzZXF1ZW5jZUFja25vd2xlZG'
    'dlZBIdCgpib290X2Vwb2NoGAIgASgEUglib290RXBvY2g=');

@$core.Deprecated('Use errorPayloadDescriptor instead')
const ErrorPayload$json = {
  '1': 'ErrorPayload',
  '2': [
    {'1': 'code', '3': 1, '4': 1, '5': 13, '10': 'code'},
    {'1': 'message', '3': 2, '4': 1, '5': 9, '10': 'message'},
    {
      '1': 'outcome_is_unknown',
      '3': 3,
      '4': 1,
      '5': 8,
      '10': 'outcomeIsUnknown'
    },
  ],
};

/// Descriptor for `ErrorPayload`. Decode as a `google.protobuf.DescriptorProto`.
final $typed_data.Uint8List errorPayloadDescriptor = $convert.base64Decode(
    'CgxFcnJvclBheWxvYWQSEgoEY29kZRgBIAEoDVIEY29kZRIYCgdtZXNzYWdlGAIgASgJUgdtZX'
    'NzYWdlEiwKEm91dGNvbWVfaXNfdW5rbm93bhgDIAEoCFIQb3V0Y29tZUlzVW5rbm93bg==');
