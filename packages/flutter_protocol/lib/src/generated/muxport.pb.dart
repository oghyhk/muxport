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

import 'package:fixnum/fixnum.dart' as $fixnum;
import 'package:protobuf/protobuf.dart' as $pb;

import 'muxport.pbenum.dart';

export 'package:protobuf/protobuf.dart' show GeneratedMessageGenericExtensions;

export 'muxport.pbenum.dart';

/// Wire Frame Header
class EnvelopeHeader extends $pb.GeneratedMessage {
  factory EnvelopeHeader({
    $core.int? protocolVersion,
    $core.String? senderId,
    $core.String? recipientId,
    $fixnum.Int64? bootEpoch,
    $fixnum.Int64? sequence,
    $fixnum.Int64? timestampMs,
    $core.String? idempotencyKey,
    $core.int? capabilityVersion,
    $core.String? requestId,
    $fixnum.Int64? expiresAtMs,
  }) {
    final result = create();
    if (protocolVersion != null) result.protocolVersion = protocolVersion;
    if (senderId != null) result.senderId = senderId;
    if (recipientId != null) result.recipientId = recipientId;
    if (bootEpoch != null) result.bootEpoch = bootEpoch;
    if (sequence != null) result.sequence = sequence;
    if (timestampMs != null) result.timestampMs = timestampMs;
    if (idempotencyKey != null) result.idempotencyKey = idempotencyKey;
    if (capabilityVersion != null) result.capabilityVersion = capabilityVersion;
    if (requestId != null) result.requestId = requestId;
    if (expiresAtMs != null) result.expiresAtMs = expiresAtMs;
    return result;
  }

  EnvelopeHeader._();

  factory EnvelopeHeader.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory EnvelopeHeader.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'EnvelopeHeader',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aI(1, _omitFieldNames ? '' : 'protocolVersion',
        fieldType: $pb.PbFieldType.OU3)
    ..aOS(2, _omitFieldNames ? '' : 'senderId')
    ..aOS(3, _omitFieldNames ? '' : 'recipientId')
    ..a<$fixnum.Int64>(
        4, _omitFieldNames ? '' : 'bootEpoch', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..a<$fixnum.Int64>(
        5, _omitFieldNames ? '' : 'sequence', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..aInt64(6, _omitFieldNames ? '' : 'timestampMs')
    ..aOS(7, _omitFieldNames ? '' : 'idempotencyKey')
    ..aI(8, _omitFieldNames ? '' : 'capabilityVersion',
        fieldType: $pb.PbFieldType.OU3)
    ..aOS(9, _omitFieldNames ? '' : 'requestId')
    ..aInt64(10, _omitFieldNames ? '' : 'expiresAtMs')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  EnvelopeHeader clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  EnvelopeHeader copyWith(void Function(EnvelopeHeader) updates) =>
      super.copyWith((message) => updates(message as EnvelopeHeader))
          as EnvelopeHeader;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static EnvelopeHeader create() => EnvelopeHeader._();
  @$core.override
  EnvelopeHeader createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static EnvelopeHeader getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<EnvelopeHeader>(create);
  static EnvelopeHeader? _defaultInstance;

  @$pb.TagNumber(1)
  $core.int get protocolVersion => $_getIZ(0);
  @$pb.TagNumber(1)
  set protocolVersion($core.int value) => $_setUnsignedInt32(0, value);
  @$pb.TagNumber(1)
  $core.bool hasProtocolVersion() => $_has(0);
  @$pb.TagNumber(1)
  void clearProtocolVersion() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get senderId => $_getSZ(1);
  @$pb.TagNumber(2)
  set senderId($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasSenderId() => $_has(1);
  @$pb.TagNumber(2)
  void clearSenderId() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get recipientId => $_getSZ(2);
  @$pb.TagNumber(3)
  set recipientId($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasRecipientId() => $_has(2);
  @$pb.TagNumber(3)
  void clearRecipientId() => $_clearField(3);

  @$pb.TagNumber(4)
  $fixnum.Int64 get bootEpoch => $_getI64(3);
  @$pb.TagNumber(4)
  set bootEpoch($fixnum.Int64 value) => $_setInt64(3, value);
  @$pb.TagNumber(4)
  $core.bool hasBootEpoch() => $_has(3);
  @$pb.TagNumber(4)
  void clearBootEpoch() => $_clearField(4);

  @$pb.TagNumber(5)
  $fixnum.Int64 get sequence => $_getI64(4);
  @$pb.TagNumber(5)
  set sequence($fixnum.Int64 value) => $_setInt64(4, value);
  @$pb.TagNumber(5)
  $core.bool hasSequence() => $_has(4);
  @$pb.TagNumber(5)
  void clearSequence() => $_clearField(5);

  @$pb.TagNumber(6)
  $fixnum.Int64 get timestampMs => $_getI64(5);
  @$pb.TagNumber(6)
  set timestampMs($fixnum.Int64 value) => $_setInt64(5, value);
  @$pb.TagNumber(6)
  $core.bool hasTimestampMs() => $_has(5);
  @$pb.TagNumber(6)
  void clearTimestampMs() => $_clearField(6);

  @$pb.TagNumber(7)
  $core.String get idempotencyKey => $_getSZ(6);
  @$pb.TagNumber(7)
  set idempotencyKey($core.String value) => $_setString(6, value);
  @$pb.TagNumber(7)
  $core.bool hasIdempotencyKey() => $_has(6);
  @$pb.TagNumber(7)
  void clearIdempotencyKey() => $_clearField(7);

  @$pb.TagNumber(8)
  $core.int get capabilityVersion => $_getIZ(7);
  @$pb.TagNumber(8)
  set capabilityVersion($core.int value) => $_setUnsignedInt32(7, value);
  @$pb.TagNumber(8)
  $core.bool hasCapabilityVersion() => $_has(7);
  @$pb.TagNumber(8)
  void clearCapabilityVersion() => $_clearField(8);

  @$pb.TagNumber(9)
  $core.String get requestId => $_getSZ(8);
  @$pb.TagNumber(9)
  set requestId($core.String value) => $_setString(8, value);
  @$pb.TagNumber(9)
  $core.bool hasRequestId() => $_has(8);
  @$pb.TagNumber(9)
  void clearRequestId() => $_clearField(9);

  @$pb.TagNumber(10)
  $fixnum.Int64 get expiresAtMs => $_getI64(9);
  @$pb.TagNumber(10)
  set expiresAtMs($fixnum.Int64 value) => $_setInt64(9, value);
  @$pb.TagNumber(10)
  $core.bool hasExpiresAtMs() => $_has(9);
  @$pb.TagNumber(10)
  void clearExpiresAtMs() => $_clearField(10);
}

enum MuxportEnvelope_Payload {
  command,
  commandResult,
  event,
  snapshot,
  ack,
  error,
  notSet
}

/// Main Frame Envelope
class MuxportEnvelope extends $pb.GeneratedMessage {
  factory MuxportEnvelope({
    EnvelopeHeader? header,
    Command? command,
    CommandResult? commandResult,
    Event? event,
    HostSnapshot? snapshot,
    Ack? ack,
    ErrorPayload? error,
  }) {
    final result = create();
    if (header != null) result.header = header;
    if (command != null) result.command = command;
    if (commandResult != null) result.commandResult = commandResult;
    if (event != null) result.event = event;
    if (snapshot != null) result.snapshot = snapshot;
    if (ack != null) result.ack = ack;
    if (error != null) result.error = error;
    return result;
  }

  MuxportEnvelope._();

  factory MuxportEnvelope.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory MuxportEnvelope.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static const $core.Map<$core.int, MuxportEnvelope_Payload>
      _MuxportEnvelope_PayloadByTag = {
    2: MuxportEnvelope_Payload.command,
    3: MuxportEnvelope_Payload.commandResult,
    4: MuxportEnvelope_Payload.event,
    5: MuxportEnvelope_Payload.snapshot,
    6: MuxportEnvelope_Payload.ack,
    7: MuxportEnvelope_Payload.error,
    0: MuxportEnvelope_Payload.notSet
  };
  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'MuxportEnvelope',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..oo(0, [2, 3, 4, 5, 6, 7])
    ..aOM<EnvelopeHeader>(1, _omitFieldNames ? '' : 'header',
        subBuilder: EnvelopeHeader.create)
    ..aOM<Command>(2, _omitFieldNames ? '' : 'command',
        subBuilder: Command.create)
    ..aOM<CommandResult>(3, _omitFieldNames ? '' : 'commandResult',
        subBuilder: CommandResult.create)
    ..aOM<Event>(4, _omitFieldNames ? '' : 'event', subBuilder: Event.create)
    ..aOM<HostSnapshot>(5, _omitFieldNames ? '' : 'snapshot',
        subBuilder: HostSnapshot.create)
    ..aOM<Ack>(6, _omitFieldNames ? '' : 'ack', subBuilder: Ack.create)
    ..aOM<ErrorPayload>(7, _omitFieldNames ? '' : 'error',
        subBuilder: ErrorPayload.create)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  MuxportEnvelope clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  MuxportEnvelope copyWith(void Function(MuxportEnvelope) updates) =>
      super.copyWith((message) => updates(message as MuxportEnvelope))
          as MuxportEnvelope;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static MuxportEnvelope create() => MuxportEnvelope._();
  @$core.override
  MuxportEnvelope createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static MuxportEnvelope getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<MuxportEnvelope>(create);
  static MuxportEnvelope? _defaultInstance;

  @$pb.TagNumber(2)
  @$pb.TagNumber(3)
  @$pb.TagNumber(4)
  @$pb.TagNumber(5)
  @$pb.TagNumber(6)
  @$pb.TagNumber(7)
  MuxportEnvelope_Payload whichPayload() =>
      _MuxportEnvelope_PayloadByTag[$_whichOneof(0)]!;
  @$pb.TagNumber(2)
  @$pb.TagNumber(3)
  @$pb.TagNumber(4)
  @$pb.TagNumber(5)
  @$pb.TagNumber(6)
  @$pb.TagNumber(7)
  void clearPayload() => $_clearField($_whichOneof(0));

  @$pb.TagNumber(1)
  EnvelopeHeader get header => $_getN(0);
  @$pb.TagNumber(1)
  set header(EnvelopeHeader value) => $_setField(1, value);
  @$pb.TagNumber(1)
  $core.bool hasHeader() => $_has(0);
  @$pb.TagNumber(1)
  void clearHeader() => $_clearField(1);
  @$pb.TagNumber(1)
  EnvelopeHeader ensureHeader() => $_ensure(0);

  @$pb.TagNumber(2)
  Command get command => $_getN(1);
  @$pb.TagNumber(2)
  set command(Command value) => $_setField(2, value);
  @$pb.TagNumber(2)
  $core.bool hasCommand() => $_has(1);
  @$pb.TagNumber(2)
  void clearCommand() => $_clearField(2);
  @$pb.TagNumber(2)
  Command ensureCommand() => $_ensure(1);

  @$pb.TagNumber(3)
  CommandResult get commandResult => $_getN(2);
  @$pb.TagNumber(3)
  set commandResult(CommandResult value) => $_setField(3, value);
  @$pb.TagNumber(3)
  $core.bool hasCommandResult() => $_has(2);
  @$pb.TagNumber(3)
  void clearCommandResult() => $_clearField(3);
  @$pb.TagNumber(3)
  CommandResult ensureCommandResult() => $_ensure(2);

  @$pb.TagNumber(4)
  Event get event => $_getN(3);
  @$pb.TagNumber(4)
  set event(Event value) => $_setField(4, value);
  @$pb.TagNumber(4)
  $core.bool hasEvent() => $_has(3);
  @$pb.TagNumber(4)
  void clearEvent() => $_clearField(4);
  @$pb.TagNumber(4)
  Event ensureEvent() => $_ensure(3);

  @$pb.TagNumber(5)
  HostSnapshot get snapshot => $_getN(4);
  @$pb.TagNumber(5)
  set snapshot(HostSnapshot value) => $_setField(5, value);
  @$pb.TagNumber(5)
  $core.bool hasSnapshot() => $_has(4);
  @$pb.TagNumber(5)
  void clearSnapshot() => $_clearField(5);
  @$pb.TagNumber(5)
  HostSnapshot ensureSnapshot() => $_ensure(4);

  @$pb.TagNumber(6)
  Ack get ack => $_getN(5);
  @$pb.TagNumber(6)
  set ack(Ack value) => $_setField(6, value);
  @$pb.TagNumber(6)
  $core.bool hasAck() => $_has(5);
  @$pb.TagNumber(6)
  void clearAck() => $_clearField(6);
  @$pb.TagNumber(6)
  Ack ensureAck() => $_ensure(5);

  @$pb.TagNumber(7)
  ErrorPayload get error => $_getN(6);
  @$pb.TagNumber(7)
  set error(ErrorPayload value) => $_setField(7, value);
  @$pb.TagNumber(7)
  $core.bool hasError() => $_has(6);
  @$pb.TagNumber(7)
  void clearError() => $_clearField(7);
  @$pb.TagNumber(7)
  ErrorPayload ensureError() => $_ensure(6);
}

enum Command_Inner {
  startSession,
  sendInput,
  steer,
  interrupt,
  approveAction,
  changeAssignment,
  rotateCredential,
  probeHost,
  queryOperation,
  provisionCredential,
  listRotationPools,
  upsertRotationPool,
  listCommandAudit,
  notSet
}

/// Commands
class Command extends $pb.GeneratedMessage {
  factory Command({
    $core.String? commandId,
    $fixnum.Int64? deadlineMs,
    StartSessionCmd? startSession,
    SendInputCmd? sendInput,
    SteerCmd? steer,
    InterruptCmd? interrupt,
    ApproveActionCmd? approveAction,
    ChangeAssignmentCmd? changeAssignment,
    RotateCredentialCmd? rotateCredential,
    ProbeHostCmd? probeHost,
    QueryOperationCmd? queryOperation,
    ProvisionCredentialCmd? provisionCredential,
    ListRotationPoolsCmd? listRotationPools,
    UpsertRotationPoolCmd? upsertRotationPool,
    ListCommandAuditCmd? listCommandAudit,
  }) {
    final result = create();
    if (commandId != null) result.commandId = commandId;
    if (deadlineMs != null) result.deadlineMs = deadlineMs;
    if (startSession != null) result.startSession = startSession;
    if (sendInput != null) result.sendInput = sendInput;
    if (steer != null) result.steer = steer;
    if (interrupt != null) result.interrupt = interrupt;
    if (approveAction != null) result.approveAction = approveAction;
    if (changeAssignment != null) result.changeAssignment = changeAssignment;
    if (rotateCredential != null) result.rotateCredential = rotateCredential;
    if (probeHost != null) result.probeHost = probeHost;
    if (queryOperation != null) result.queryOperation = queryOperation;
    if (provisionCredential != null)
      result.provisionCredential = provisionCredential;
    if (listRotationPools != null) result.listRotationPools = listRotationPools;
    if (upsertRotationPool != null)
      result.upsertRotationPool = upsertRotationPool;
    if (listCommandAudit != null) result.listCommandAudit = listCommandAudit;
    return result;
  }

  Command._();

  factory Command.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory Command.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static const $core.Map<$core.int, Command_Inner> _Command_InnerByTag = {
    3: Command_Inner.startSession,
    4: Command_Inner.sendInput,
    5: Command_Inner.steer,
    6: Command_Inner.interrupt,
    7: Command_Inner.approveAction,
    8: Command_Inner.changeAssignment,
    9: Command_Inner.rotateCredential,
    10: Command_Inner.probeHost,
    11: Command_Inner.queryOperation,
    12: Command_Inner.provisionCredential,
    13: Command_Inner.listRotationPools,
    14: Command_Inner.upsertRotationPool,
    15: Command_Inner.listCommandAudit,
    0: Command_Inner.notSet
  };
  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'Command',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..oo(0, [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])
    ..aOS(1, _omitFieldNames ? '' : 'commandId')
    ..aInt64(2, _omitFieldNames ? '' : 'deadlineMs')
    ..aOM<StartSessionCmd>(3, _omitFieldNames ? '' : 'startSession',
        subBuilder: StartSessionCmd.create)
    ..aOM<SendInputCmd>(4, _omitFieldNames ? '' : 'sendInput',
        subBuilder: SendInputCmd.create)
    ..aOM<SteerCmd>(5, _omitFieldNames ? '' : 'steer',
        subBuilder: SteerCmd.create)
    ..aOM<InterruptCmd>(6, _omitFieldNames ? '' : 'interrupt',
        subBuilder: InterruptCmd.create)
    ..aOM<ApproveActionCmd>(7, _omitFieldNames ? '' : 'approveAction',
        subBuilder: ApproveActionCmd.create)
    ..aOM<ChangeAssignmentCmd>(8, _omitFieldNames ? '' : 'changeAssignment',
        subBuilder: ChangeAssignmentCmd.create)
    ..aOM<RotateCredentialCmd>(9, _omitFieldNames ? '' : 'rotateCredential',
        subBuilder: RotateCredentialCmd.create)
    ..aOM<ProbeHostCmd>(10, _omitFieldNames ? '' : 'probeHost',
        subBuilder: ProbeHostCmd.create)
    ..aOM<QueryOperationCmd>(11, _omitFieldNames ? '' : 'queryOperation',
        subBuilder: QueryOperationCmd.create)
    ..aOM<ProvisionCredentialCmd>(
        12, _omitFieldNames ? '' : 'provisionCredential',
        subBuilder: ProvisionCredentialCmd.create)
    ..aOM<ListRotationPoolsCmd>(13, _omitFieldNames ? '' : 'listRotationPools',
        subBuilder: ListRotationPoolsCmd.create)
    ..aOM<UpsertRotationPoolCmd>(
        14, _omitFieldNames ? '' : 'upsertRotationPool',
        subBuilder: UpsertRotationPoolCmd.create)
    ..aOM<ListCommandAuditCmd>(15, _omitFieldNames ? '' : 'listCommandAudit',
        subBuilder: ListCommandAuditCmd.create)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Command clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Command copyWith(void Function(Command) updates) =>
      super.copyWith((message) => updates(message as Command)) as Command;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static Command create() => Command._();
  @$core.override
  Command createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static Command getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<Command>(create);
  static Command? _defaultInstance;

  @$pb.TagNumber(3)
  @$pb.TagNumber(4)
  @$pb.TagNumber(5)
  @$pb.TagNumber(6)
  @$pb.TagNumber(7)
  @$pb.TagNumber(8)
  @$pb.TagNumber(9)
  @$pb.TagNumber(10)
  @$pb.TagNumber(11)
  @$pb.TagNumber(12)
  @$pb.TagNumber(13)
  @$pb.TagNumber(14)
  @$pb.TagNumber(15)
  Command_Inner whichInner() => _Command_InnerByTag[$_whichOneof(0)]!;
  @$pb.TagNumber(3)
  @$pb.TagNumber(4)
  @$pb.TagNumber(5)
  @$pb.TagNumber(6)
  @$pb.TagNumber(7)
  @$pb.TagNumber(8)
  @$pb.TagNumber(9)
  @$pb.TagNumber(10)
  @$pb.TagNumber(11)
  @$pb.TagNumber(12)
  @$pb.TagNumber(13)
  @$pb.TagNumber(14)
  @$pb.TagNumber(15)
  void clearInner() => $_clearField($_whichOneof(0));

  @$pb.TagNumber(1)
  $core.String get commandId => $_getSZ(0);
  @$pb.TagNumber(1)
  set commandId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasCommandId() => $_has(0);
  @$pb.TagNumber(1)
  void clearCommandId() => $_clearField(1);

  @$pb.TagNumber(2)
  $fixnum.Int64 get deadlineMs => $_getI64(1);
  @$pb.TagNumber(2)
  set deadlineMs($fixnum.Int64 value) => $_setInt64(1, value);
  @$pb.TagNumber(2)
  $core.bool hasDeadlineMs() => $_has(1);
  @$pb.TagNumber(2)
  void clearDeadlineMs() => $_clearField(2);

  @$pb.TagNumber(3)
  StartSessionCmd get startSession => $_getN(2);
  @$pb.TagNumber(3)
  set startSession(StartSessionCmd value) => $_setField(3, value);
  @$pb.TagNumber(3)
  $core.bool hasStartSession() => $_has(2);
  @$pb.TagNumber(3)
  void clearStartSession() => $_clearField(3);
  @$pb.TagNumber(3)
  StartSessionCmd ensureStartSession() => $_ensure(2);

  @$pb.TagNumber(4)
  SendInputCmd get sendInput => $_getN(3);
  @$pb.TagNumber(4)
  set sendInput(SendInputCmd value) => $_setField(4, value);
  @$pb.TagNumber(4)
  $core.bool hasSendInput() => $_has(3);
  @$pb.TagNumber(4)
  void clearSendInput() => $_clearField(4);
  @$pb.TagNumber(4)
  SendInputCmd ensureSendInput() => $_ensure(3);

  @$pb.TagNumber(5)
  SteerCmd get steer => $_getN(4);
  @$pb.TagNumber(5)
  set steer(SteerCmd value) => $_setField(5, value);
  @$pb.TagNumber(5)
  $core.bool hasSteer() => $_has(4);
  @$pb.TagNumber(5)
  void clearSteer() => $_clearField(5);
  @$pb.TagNumber(5)
  SteerCmd ensureSteer() => $_ensure(4);

  @$pb.TagNumber(6)
  InterruptCmd get interrupt => $_getN(5);
  @$pb.TagNumber(6)
  set interrupt(InterruptCmd value) => $_setField(6, value);
  @$pb.TagNumber(6)
  $core.bool hasInterrupt() => $_has(5);
  @$pb.TagNumber(6)
  void clearInterrupt() => $_clearField(6);
  @$pb.TagNumber(6)
  InterruptCmd ensureInterrupt() => $_ensure(5);

  @$pb.TagNumber(7)
  ApproveActionCmd get approveAction => $_getN(6);
  @$pb.TagNumber(7)
  set approveAction(ApproveActionCmd value) => $_setField(7, value);
  @$pb.TagNumber(7)
  $core.bool hasApproveAction() => $_has(6);
  @$pb.TagNumber(7)
  void clearApproveAction() => $_clearField(7);
  @$pb.TagNumber(7)
  ApproveActionCmd ensureApproveAction() => $_ensure(6);

  @$pb.TagNumber(8)
  ChangeAssignmentCmd get changeAssignment => $_getN(7);
  @$pb.TagNumber(8)
  set changeAssignment(ChangeAssignmentCmd value) => $_setField(8, value);
  @$pb.TagNumber(8)
  $core.bool hasChangeAssignment() => $_has(7);
  @$pb.TagNumber(8)
  void clearChangeAssignment() => $_clearField(8);
  @$pb.TagNumber(8)
  ChangeAssignmentCmd ensureChangeAssignment() => $_ensure(7);

  @$pb.TagNumber(9)
  RotateCredentialCmd get rotateCredential => $_getN(8);
  @$pb.TagNumber(9)
  set rotateCredential(RotateCredentialCmd value) => $_setField(9, value);
  @$pb.TagNumber(9)
  $core.bool hasRotateCredential() => $_has(8);
  @$pb.TagNumber(9)
  void clearRotateCredential() => $_clearField(9);
  @$pb.TagNumber(9)
  RotateCredentialCmd ensureRotateCredential() => $_ensure(8);

  @$pb.TagNumber(10)
  ProbeHostCmd get probeHost => $_getN(9);
  @$pb.TagNumber(10)
  set probeHost(ProbeHostCmd value) => $_setField(10, value);
  @$pb.TagNumber(10)
  $core.bool hasProbeHost() => $_has(9);
  @$pb.TagNumber(10)
  void clearProbeHost() => $_clearField(10);
  @$pb.TagNumber(10)
  ProbeHostCmd ensureProbeHost() => $_ensure(9);

  @$pb.TagNumber(11)
  QueryOperationCmd get queryOperation => $_getN(10);
  @$pb.TagNumber(11)
  set queryOperation(QueryOperationCmd value) => $_setField(11, value);
  @$pb.TagNumber(11)
  $core.bool hasQueryOperation() => $_has(10);
  @$pb.TagNumber(11)
  void clearQueryOperation() => $_clearField(11);
  @$pb.TagNumber(11)
  QueryOperationCmd ensureQueryOperation() => $_ensure(10);

  @$pb.TagNumber(12)
  ProvisionCredentialCmd get provisionCredential => $_getN(11);
  @$pb.TagNumber(12)
  set provisionCredential(ProvisionCredentialCmd value) =>
      $_setField(12, value);
  @$pb.TagNumber(12)
  $core.bool hasProvisionCredential() => $_has(11);
  @$pb.TagNumber(12)
  void clearProvisionCredential() => $_clearField(12);
  @$pb.TagNumber(12)
  ProvisionCredentialCmd ensureProvisionCredential() => $_ensure(11);

  @$pb.TagNumber(13)
  ListRotationPoolsCmd get listRotationPools => $_getN(12);
  @$pb.TagNumber(13)
  set listRotationPools(ListRotationPoolsCmd value) => $_setField(13, value);
  @$pb.TagNumber(13)
  $core.bool hasListRotationPools() => $_has(12);
  @$pb.TagNumber(13)
  void clearListRotationPools() => $_clearField(13);
  @$pb.TagNumber(13)
  ListRotationPoolsCmd ensureListRotationPools() => $_ensure(12);

  @$pb.TagNumber(14)
  UpsertRotationPoolCmd get upsertRotationPool => $_getN(13);
  @$pb.TagNumber(14)
  set upsertRotationPool(UpsertRotationPoolCmd value) => $_setField(14, value);
  @$pb.TagNumber(14)
  $core.bool hasUpsertRotationPool() => $_has(13);
  @$pb.TagNumber(14)
  void clearUpsertRotationPool() => $_clearField(14);
  @$pb.TagNumber(14)
  UpsertRotationPoolCmd ensureUpsertRotationPool() => $_ensure(13);

  @$pb.TagNumber(15)
  ListCommandAuditCmd get listCommandAudit => $_getN(14);
  @$pb.TagNumber(15)
  set listCommandAudit(ListCommandAuditCmd value) => $_setField(15, value);
  @$pb.TagNumber(15)
  $core.bool hasListCommandAudit() => $_has(14);
  @$pb.TagNumber(15)
  void clearListCommandAudit() => $_clearField(15);
  @$pb.TagNumber(15)
  ListCommandAuditCmd ensureListCommandAudit() => $_ensure(14);
}

class StartSessionCmd extends $pb.GeneratedMessage {
  factory StartSessionCmd({
    $core.String? runtimeId,
    $core.String? projectPath,
    $core.String? prompt,
    $core.String? credentialProfileId,
  }) {
    final result = create();
    if (runtimeId != null) result.runtimeId = runtimeId;
    if (projectPath != null) result.projectPath = projectPath;
    if (prompt != null) result.prompt = prompt;
    if (credentialProfileId != null)
      result.credentialProfileId = credentialProfileId;
    return result;
  }

  StartSessionCmd._();

  factory StartSessionCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory StartSessionCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'StartSessionCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'runtimeId')
    ..aOS(2, _omitFieldNames ? '' : 'projectPath')
    ..aOS(3, _omitFieldNames ? '' : 'prompt')
    ..aOS(4, _omitFieldNames ? '' : 'credentialProfileId')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  StartSessionCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  StartSessionCmd copyWith(void Function(StartSessionCmd) updates) =>
      super.copyWith((message) => updates(message as StartSessionCmd))
          as StartSessionCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static StartSessionCmd create() => StartSessionCmd._();
  @$core.override
  StartSessionCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static StartSessionCmd getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<StartSessionCmd>(create);
  static StartSessionCmd? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get runtimeId => $_getSZ(0);
  @$pb.TagNumber(1)
  set runtimeId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasRuntimeId() => $_has(0);
  @$pb.TagNumber(1)
  void clearRuntimeId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get projectPath => $_getSZ(1);
  @$pb.TagNumber(2)
  set projectPath($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasProjectPath() => $_has(1);
  @$pb.TagNumber(2)
  void clearProjectPath() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get prompt => $_getSZ(2);
  @$pb.TagNumber(3)
  set prompt($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasPrompt() => $_has(2);
  @$pb.TagNumber(3)
  void clearPrompt() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get credentialProfileId => $_getSZ(3);
  @$pb.TagNumber(4)
  set credentialProfileId($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasCredentialProfileId() => $_has(3);
  @$pb.TagNumber(4)
  void clearCredentialProfileId() => $_clearField(4);
}

class SendInputCmd extends $pb.GeneratedMessage {
  factory SendInputCmd({
    $core.String? sessionId,
    $core.String? text,
    $core.String? runtimeId,
  }) {
    final result = create();
    if (sessionId != null) result.sessionId = sessionId;
    if (text != null) result.text = text;
    if (runtimeId != null) result.runtimeId = runtimeId;
    return result;
  }

  SendInputCmd._();

  factory SendInputCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory SendInputCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'SendInputCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'sessionId')
    ..aOS(2, _omitFieldNames ? '' : 'text')
    ..aOS(3, _omitFieldNames ? '' : 'runtimeId')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SendInputCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SendInputCmd copyWith(void Function(SendInputCmd) updates) =>
      super.copyWith((message) => updates(message as SendInputCmd))
          as SendInputCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static SendInputCmd create() => SendInputCmd._();
  @$core.override
  SendInputCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static SendInputCmd getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<SendInputCmd>(create);
  static SendInputCmd? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get sessionId => $_getSZ(0);
  @$pb.TagNumber(1)
  set sessionId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasSessionId() => $_has(0);
  @$pb.TagNumber(1)
  void clearSessionId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get text => $_getSZ(1);
  @$pb.TagNumber(2)
  set text($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasText() => $_has(1);
  @$pb.TagNumber(2)
  void clearText() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get runtimeId => $_getSZ(2);
  @$pb.TagNumber(3)
  set runtimeId($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasRuntimeId() => $_has(2);
  @$pb.TagNumber(3)
  void clearRuntimeId() => $_clearField(3);
}

class SteerCmd extends $pb.GeneratedMessage {
  factory SteerCmd({
    $core.String? sessionId,
    $core.String? instruction,
    $core.String? runtimeId,
  }) {
    final result = create();
    if (sessionId != null) result.sessionId = sessionId;
    if (instruction != null) result.instruction = instruction;
    if (runtimeId != null) result.runtimeId = runtimeId;
    return result;
  }

  SteerCmd._();

  factory SteerCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory SteerCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'SteerCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'sessionId')
    ..aOS(2, _omitFieldNames ? '' : 'instruction')
    ..aOS(3, _omitFieldNames ? '' : 'runtimeId')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SteerCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SteerCmd copyWith(void Function(SteerCmd) updates) =>
      super.copyWith((message) => updates(message as SteerCmd)) as SteerCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static SteerCmd create() => SteerCmd._();
  @$core.override
  SteerCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static SteerCmd getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<SteerCmd>(create);
  static SteerCmd? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get sessionId => $_getSZ(0);
  @$pb.TagNumber(1)
  set sessionId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasSessionId() => $_has(0);
  @$pb.TagNumber(1)
  void clearSessionId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get instruction => $_getSZ(1);
  @$pb.TagNumber(2)
  set instruction($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasInstruction() => $_has(1);
  @$pb.TagNumber(2)
  void clearInstruction() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get runtimeId => $_getSZ(2);
  @$pb.TagNumber(3)
  set runtimeId($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasRuntimeId() => $_has(2);
  @$pb.TagNumber(3)
  void clearRuntimeId() => $_clearField(3);
}

class InterruptCmd extends $pb.GeneratedMessage {
  factory InterruptCmd({
    $core.String? sessionId,
    $core.String? reason,
    $core.String? runtimeId,
  }) {
    final result = create();
    if (sessionId != null) result.sessionId = sessionId;
    if (reason != null) result.reason = reason;
    if (runtimeId != null) result.runtimeId = runtimeId;
    return result;
  }

  InterruptCmd._();

  factory InterruptCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory InterruptCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'InterruptCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'sessionId')
    ..aOS(2, _omitFieldNames ? '' : 'reason')
    ..aOS(3, _omitFieldNames ? '' : 'runtimeId')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  InterruptCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  InterruptCmd copyWith(void Function(InterruptCmd) updates) =>
      super.copyWith((message) => updates(message as InterruptCmd))
          as InterruptCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static InterruptCmd create() => InterruptCmd._();
  @$core.override
  InterruptCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static InterruptCmd getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<InterruptCmd>(create);
  static InterruptCmd? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get sessionId => $_getSZ(0);
  @$pb.TagNumber(1)
  set sessionId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasSessionId() => $_has(0);
  @$pb.TagNumber(1)
  void clearSessionId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get reason => $_getSZ(1);
  @$pb.TagNumber(2)
  set reason($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasReason() => $_has(1);
  @$pb.TagNumber(2)
  void clearReason() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get runtimeId => $_getSZ(2);
  @$pb.TagNumber(3)
  set runtimeId($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasRuntimeId() => $_has(2);
  @$pb.TagNumber(3)
  void clearRuntimeId() => $_clearField(3);
}

class ApproveActionCmd extends $pb.GeneratedMessage {
  factory ApproveActionCmd({
    $core.String? approvalId,
    $core.bool? approved,
    $core.String? decisionReason,
    $core.String? sessionId,
    $core.String? runtimeId,
  }) {
    final result = create();
    if (approvalId != null) result.approvalId = approvalId;
    if (approved != null) result.approved = approved;
    if (decisionReason != null) result.decisionReason = decisionReason;
    if (sessionId != null) result.sessionId = sessionId;
    if (runtimeId != null) result.runtimeId = runtimeId;
    return result;
  }

  ApproveActionCmd._();

  factory ApproveActionCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory ApproveActionCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ApproveActionCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'approvalId')
    ..aOB(2, _omitFieldNames ? '' : 'approved')
    ..aOS(3, _omitFieldNames ? '' : 'decisionReason')
    ..aOS(4, _omitFieldNames ? '' : 'sessionId')
    ..aOS(5, _omitFieldNames ? '' : 'runtimeId')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ApproveActionCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ApproveActionCmd copyWith(void Function(ApproveActionCmd) updates) =>
      super.copyWith((message) => updates(message as ApproveActionCmd))
          as ApproveActionCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static ApproveActionCmd create() => ApproveActionCmd._();
  @$core.override
  ApproveActionCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static ApproveActionCmd getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ApproveActionCmd>(create);
  static ApproveActionCmd? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get approvalId => $_getSZ(0);
  @$pb.TagNumber(1)
  set approvalId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasApprovalId() => $_has(0);
  @$pb.TagNumber(1)
  void clearApprovalId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.bool get approved => $_getBF(1);
  @$pb.TagNumber(2)
  set approved($core.bool value) => $_setBool(1, value);
  @$pb.TagNumber(2)
  $core.bool hasApproved() => $_has(1);
  @$pb.TagNumber(2)
  void clearApproved() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get decisionReason => $_getSZ(2);
  @$pb.TagNumber(3)
  set decisionReason($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasDecisionReason() => $_has(2);
  @$pb.TagNumber(3)
  void clearDecisionReason() => $_clearField(3);

  /// Required so approval replies remain routable after connector restart.
  @$pb.TagNumber(4)
  $core.String get sessionId => $_getSZ(3);
  @$pb.TagNumber(4)
  set sessionId($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasSessionId() => $_has(3);
  @$pb.TagNumber(4)
  void clearSessionId() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get runtimeId => $_getSZ(4);
  @$pb.TagNumber(5)
  set runtimeId($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasRuntimeId() => $_has(4);
  @$pb.TagNumber(5)
  void clearRuntimeId() => $_clearField(5);
}

class ChangeAssignmentCmd extends $pb.GeneratedMessage {
  factory ChangeAssignmentCmd({
    $core.String? targetType,
    $core.String? targetId,
    $core.String? newCredentialProfileId,
  }) {
    final result = create();
    if (targetType != null) result.targetType = targetType;
    if (targetId != null) result.targetId = targetId;
    if (newCredentialProfileId != null)
      result.newCredentialProfileId = newCredentialProfileId;
    return result;
  }

  ChangeAssignmentCmd._();

  factory ChangeAssignmentCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory ChangeAssignmentCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ChangeAssignmentCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'targetType')
    ..aOS(2, _omitFieldNames ? '' : 'targetId')
    ..aOS(3, _omitFieldNames ? '' : 'newCredentialProfileId')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ChangeAssignmentCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ChangeAssignmentCmd copyWith(void Function(ChangeAssignmentCmd) updates) =>
      super.copyWith((message) => updates(message as ChangeAssignmentCmd))
          as ChangeAssignmentCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static ChangeAssignmentCmd create() => ChangeAssignmentCmd._();
  @$core.override
  ChangeAssignmentCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static ChangeAssignmentCmd getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ChangeAssignmentCmd>(create);
  static ChangeAssignmentCmd? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get targetType => $_getSZ(0);
  @$pb.TagNumber(1)
  set targetType($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasTargetType() => $_has(0);
  @$pb.TagNumber(1)
  void clearTargetType() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get targetId => $_getSZ(1);
  @$pb.TagNumber(2)
  set targetId($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasTargetId() => $_has(1);
  @$pb.TagNumber(2)
  void clearTargetId() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get newCredentialProfileId => $_getSZ(2);
  @$pb.TagNumber(3)
  set newCredentialProfileId($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasNewCredentialProfileId() => $_has(2);
  @$pb.TagNumber(3)
  void clearNewCredentialProfileId() => $_clearField(3);
}

class RotateCredentialCmd extends $pb.GeneratedMessage {
  factory RotateCredentialCmd({
    $core.String? poolId,
    $core.String? targetRuntimeId,
    $core.bool? force,
  }) {
    final result = create();
    if (poolId != null) result.poolId = poolId;
    if (targetRuntimeId != null) result.targetRuntimeId = targetRuntimeId;
    if (force != null) result.force = force;
    return result;
  }

  RotateCredentialCmd._();

  factory RotateCredentialCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory RotateCredentialCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'RotateCredentialCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'poolId')
    ..aOS(2, _omitFieldNames ? '' : 'targetRuntimeId')
    ..aOB(3, _omitFieldNames ? '' : 'force')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  RotateCredentialCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  RotateCredentialCmd copyWith(void Function(RotateCredentialCmd) updates) =>
      super.copyWith((message) => updates(message as RotateCredentialCmd))
          as RotateCredentialCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static RotateCredentialCmd create() => RotateCredentialCmd._();
  @$core.override
  RotateCredentialCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static RotateCredentialCmd getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<RotateCredentialCmd>(create);
  static RotateCredentialCmd? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get poolId => $_getSZ(0);
  @$pb.TagNumber(1)
  set poolId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasPoolId() => $_has(0);
  @$pb.TagNumber(1)
  void clearPoolId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get targetRuntimeId => $_getSZ(1);
  @$pb.TagNumber(2)
  set targetRuntimeId($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasTargetRuntimeId() => $_has(1);
  @$pb.TagNumber(2)
  void clearTargetRuntimeId() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.bool get force => $_getBF(2);
  @$pb.TagNumber(3)
  set force($core.bool value) => $_setBool(2, value);
  @$pb.TagNumber(3)
  $core.bool hasForce() => $_has(2);
  @$pb.TagNumber(3)
  void clearForce() => $_clearField(3);
}

class ProbeHostCmd extends $pb.GeneratedMessage {
  factory ProbeHostCmd() => create();

  ProbeHostCmd._();

  factory ProbeHostCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory ProbeHostCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ProbeHostCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ProbeHostCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ProbeHostCmd copyWith(void Function(ProbeHostCmd) updates) =>
      super.copyWith((message) => updates(message as ProbeHostCmd))
          as ProbeHostCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static ProbeHostCmd create() => ProbeHostCmd._();
  @$core.override
  ProbeHostCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static ProbeHostCmd getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ProbeHostCmd>(create);
  static ProbeHostCmd? _defaultInstance;
}

class QueryOperationCmd extends $pb.GeneratedMessage {
  factory QueryOperationCmd({
    $core.String? idempotencyKey,
  }) {
    final result = create();
    if (idempotencyKey != null) result.idempotencyKey = idempotencyKey;
    return result;
  }

  QueryOperationCmd._();

  factory QueryOperationCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory QueryOperationCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'QueryOperationCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'idempotencyKey')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  QueryOperationCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  QueryOperationCmd copyWith(void Function(QueryOperationCmd) updates) =>
      super.copyWith((message) => updates(message as QueryOperationCmd))
          as QueryOperationCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static QueryOperationCmd create() => QueryOperationCmd._();
  @$core.override
  QueryOperationCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static QueryOperationCmd getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<QueryOperationCmd>(create);
  static QueryOperationCmd? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get idempotencyKey => $_getSZ(0);
  @$pb.TagNumber(1)
  set idempotencyKey($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasIdempotencyKey() => $_has(0);
  @$pb.TagNumber(1)
  void clearIdempotencyKey() => $_clearField(1);
}

/// Rotation policy contains references and bounded non-secret metadata only.
/// Provider credential values are never represented in a pool command.
class ListRotationPoolsCmd extends $pb.GeneratedMessage {
  factory ListRotationPoolsCmd() => create();

  ListRotationPoolsCmd._();

  factory ListRotationPoolsCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory ListRotationPoolsCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ListRotationPoolsCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ListRotationPoolsCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ListRotationPoolsCmd copyWith(void Function(ListRotationPoolsCmd) updates) =>
      super.copyWith((message) => updates(message as ListRotationPoolsCmd))
          as ListRotationPoolsCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static ListRotationPoolsCmd create() => ListRotationPoolsCmd._();
  @$core.override
  ListRotationPoolsCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static ListRotationPoolsCmd getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ListRotationPoolsCmd>(create);
  static ListRotationPoolsCmd? _defaultInstance;
}

class UpsertRotationPoolCmd extends $pb.GeneratedMessage {
  factory UpsertRotationPoolCmd({
    $core.String? poolId,
    $core.String? providerId,
    $core.Iterable<$core.String>? orderedProfileIds,
    $core.String? mode,
    $fixnum.Int64? cooldownMs,
    $core.int? maxSwitchesPerHour,
    $core.Iterable<$core.String>? allowedHostIds,
    $core.bool? quotaFailoverEnabled,
  }) {
    final result = create();
    if (poolId != null) result.poolId = poolId;
    if (providerId != null) result.providerId = providerId;
    if (orderedProfileIds != null)
      result.orderedProfileIds.addAll(orderedProfileIds);
    if (mode != null) result.mode = mode;
    if (cooldownMs != null) result.cooldownMs = cooldownMs;
    if (maxSwitchesPerHour != null)
      result.maxSwitchesPerHour = maxSwitchesPerHour;
    if (allowedHostIds != null) result.allowedHostIds.addAll(allowedHostIds);
    if (quotaFailoverEnabled != null)
      result.quotaFailoverEnabled = quotaFailoverEnabled;
    return result;
  }

  UpsertRotationPoolCmd._();

  factory UpsertRotationPoolCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory UpsertRotationPoolCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'UpsertRotationPoolCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'poolId')
    ..aOS(2, _omitFieldNames ? '' : 'providerId')
    ..pPS(3, _omitFieldNames ? '' : 'orderedProfileIds')
    ..aOS(4, _omitFieldNames ? '' : 'mode')
    ..aInt64(5, _omitFieldNames ? '' : 'cooldownMs')
    ..aI(6, _omitFieldNames ? '' : 'maxSwitchesPerHour',
        fieldType: $pb.PbFieldType.OU3)
    ..pPS(7, _omitFieldNames ? '' : 'allowedHostIds')
    ..aOB(8, _omitFieldNames ? '' : 'quotaFailoverEnabled')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UpsertRotationPoolCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  UpsertRotationPoolCmd copyWith(
          void Function(UpsertRotationPoolCmd) updates) =>
      super.copyWith((message) => updates(message as UpsertRotationPoolCmd))
          as UpsertRotationPoolCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static UpsertRotationPoolCmd create() => UpsertRotationPoolCmd._();
  @$core.override
  UpsertRotationPoolCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static UpsertRotationPoolCmd getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<UpsertRotationPoolCmd>(create);
  static UpsertRotationPoolCmd? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get poolId => $_getSZ(0);
  @$pb.TagNumber(1)
  set poolId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasPoolId() => $_has(0);
  @$pb.TagNumber(1)
  void clearPoolId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get providerId => $_getSZ(1);
  @$pb.TagNumber(2)
  set providerId($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasProviderId() => $_has(1);
  @$pb.TagNumber(2)
  void clearProviderId() => $_clearField(2);

  @$pb.TagNumber(3)
  $pb.PbList<$core.String> get orderedProfileIds => $_getList(2);

  @$pb.TagNumber(4)
  $core.String get mode => $_getSZ(3);
  @$pb.TagNumber(4)
  set mode($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasMode() => $_has(3);
  @$pb.TagNumber(4)
  void clearMode() => $_clearField(4);

  @$pb.TagNumber(5)
  $fixnum.Int64 get cooldownMs => $_getI64(4);
  @$pb.TagNumber(5)
  set cooldownMs($fixnum.Int64 value) => $_setInt64(4, value);
  @$pb.TagNumber(5)
  $core.bool hasCooldownMs() => $_has(4);
  @$pb.TagNumber(5)
  void clearCooldownMs() => $_clearField(5);

  @$pb.TagNumber(6)
  $core.int get maxSwitchesPerHour => $_getIZ(5);
  @$pb.TagNumber(6)
  set maxSwitchesPerHour($core.int value) => $_setUnsignedInt32(5, value);
  @$pb.TagNumber(6)
  $core.bool hasMaxSwitchesPerHour() => $_has(5);
  @$pb.TagNumber(6)
  void clearMaxSwitchesPerHour() => $_clearField(6);

  @$pb.TagNumber(7)
  $pb.PbList<$core.String> get allowedHostIds => $_getList(6);

  @$pb.TagNumber(8)
  $core.bool get quotaFailoverEnabled => $_getBF(7);
  @$pb.TagNumber(8)
  set quotaFailoverEnabled($core.bool value) => $_setBool(7, value);
  @$pb.TagNumber(8)
  $core.bool hasQuotaFailoverEnabled() => $_has(7);
  @$pb.TagNumber(8)
  void clearQuotaFailoverEnabled() => $_clearField(8);
}

/// Reads the connector-local, redacted control-plane audit history. Records
/// never contain command payloads, provider credential values, or raw errors.
class ListCommandAuditCmd extends $pb.GeneratedMessage {
  factory ListCommandAuditCmd({
    $core.int? limit,
  }) {
    final result = create();
    if (limit != null) result.limit = limit;
    return result;
  }

  ListCommandAuditCmd._();

  factory ListCommandAuditCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory ListCommandAuditCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ListCommandAuditCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aI(1, _omitFieldNames ? '' : 'limit', fieldType: $pb.PbFieldType.OU3)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ListCommandAuditCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ListCommandAuditCmd copyWith(void Function(ListCommandAuditCmd) updates) =>
      super.copyWith((message) => updates(message as ListCommandAuditCmd))
          as ListCommandAuditCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static ListCommandAuditCmd create() => ListCommandAuditCmd._();
  @$core.override
  ListCommandAuditCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static ListCommandAuditCmd getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ListCommandAuditCmd>(create);
  static ListCommandAuditCmd? _defaultInstance;

  @$pb.TagNumber(1)
  $core.int get limit => $_getIZ(0);
  @$pb.TagNumber(1)
  set limit($core.int value) => $_setUnsignedInt32(0, value);
  @$pb.TagNumber(1)
  $core.bool hasLimit() => $_has(0);
  @$pb.TagNumber(1)
  void clearLimit() => $_clearField(1);
}

/// The provider secret is never a plaintext command field. `secret_ciphertext`
/// is separately encrypted with a purpose-separated provisioning key derived
/// from the authenticated device/host handshake and bound to every metadata
/// field below.
class ProvisionCredentialCmd extends $pb.GeneratedMessage {
  factory ProvisionCredentialCmd({
    $core.String? profileId,
    $core.String? displayName,
    $core.String? provider,
    $core.String? credentialType,
    $core.String? accountFingerprint,
    $core.List<$core.int>? secretNonce,
    $core.List<$core.int>? secretCiphertext,
  }) {
    final result = create();
    if (profileId != null) result.profileId = profileId;
    if (displayName != null) result.displayName = displayName;
    if (provider != null) result.provider = provider;
    if (credentialType != null) result.credentialType = credentialType;
    if (accountFingerprint != null)
      result.accountFingerprint = accountFingerprint;
    if (secretNonce != null) result.secretNonce = secretNonce;
    if (secretCiphertext != null) result.secretCiphertext = secretCiphertext;
    return result;
  }

  ProvisionCredentialCmd._();

  factory ProvisionCredentialCmd.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory ProvisionCredentialCmd.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ProvisionCredentialCmd',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'profileId')
    ..aOS(2, _omitFieldNames ? '' : 'displayName')
    ..aOS(3, _omitFieldNames ? '' : 'provider')
    ..aOS(4, _omitFieldNames ? '' : 'credentialType')
    ..aOS(5, _omitFieldNames ? '' : 'accountFingerprint')
    ..a<$core.List<$core.int>>(
        6, _omitFieldNames ? '' : 'secretNonce', $pb.PbFieldType.OY)
    ..a<$core.List<$core.int>>(
        7, _omitFieldNames ? '' : 'secretCiphertext', $pb.PbFieldType.OY)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ProvisionCredentialCmd clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ProvisionCredentialCmd copyWith(
          void Function(ProvisionCredentialCmd) updates) =>
      super.copyWith((message) => updates(message as ProvisionCredentialCmd))
          as ProvisionCredentialCmd;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static ProvisionCredentialCmd create() => ProvisionCredentialCmd._();
  @$core.override
  ProvisionCredentialCmd createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static ProvisionCredentialCmd getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ProvisionCredentialCmd>(create);
  static ProvisionCredentialCmd? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get profileId => $_getSZ(0);
  @$pb.TagNumber(1)
  set profileId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasProfileId() => $_has(0);
  @$pb.TagNumber(1)
  void clearProfileId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get displayName => $_getSZ(1);
  @$pb.TagNumber(2)
  set displayName($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasDisplayName() => $_has(1);
  @$pb.TagNumber(2)
  void clearDisplayName() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get provider => $_getSZ(2);
  @$pb.TagNumber(3)
  set provider($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasProvider() => $_has(2);
  @$pb.TagNumber(3)
  void clearProvider() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get credentialType => $_getSZ(3);
  @$pb.TagNumber(4)
  set credentialType($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasCredentialType() => $_has(3);
  @$pb.TagNumber(4)
  void clearCredentialType() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get accountFingerprint => $_getSZ(4);
  @$pb.TagNumber(5)
  set accountFingerprint($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasAccountFingerprint() => $_has(4);
  @$pb.TagNumber(5)
  void clearAccountFingerprint() => $_clearField(5);

  @$pb.TagNumber(6)
  $core.List<$core.int> get secretNonce => $_getN(5);
  @$pb.TagNumber(6)
  set secretNonce($core.List<$core.int> value) => $_setBytes(5, value);
  @$pb.TagNumber(6)
  $core.bool hasSecretNonce() => $_has(5);
  @$pb.TagNumber(6)
  void clearSecretNonce() => $_clearField(6);

  @$pb.TagNumber(7)
  $core.List<$core.int> get secretCiphertext => $_getN(6);
  @$pb.TagNumber(7)
  set secretCiphertext($core.List<$core.int> value) => $_setBytes(6, value);
  @$pb.TagNumber(7)
  $core.bool hasSecretCiphertext() => $_has(6);
  @$pb.TagNumber(7)
  void clearSecretCiphertext() => $_clearField(7);
}

/// Command Result
class CommandResult extends $pb.GeneratedMessage {
  factory CommandResult({
    $core.String? commandId,
    RemoteOpState? state,
    $core.bool? success,
    $core.String? errorMessage,
    $fixnum.Int64? completedAtMs,
    $core.String? resultJson,
  }) {
    final result = create();
    if (commandId != null) result.commandId = commandId;
    if (state != null) result.state = state;
    if (success != null) result.success = success;
    if (errorMessage != null) result.errorMessage = errorMessage;
    if (completedAtMs != null) result.completedAtMs = completedAtMs;
    if (resultJson != null) result.resultJson = resultJson;
    return result;
  }

  CommandResult._();

  factory CommandResult.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory CommandResult.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'CommandResult',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'commandId')
    ..aE<RemoteOpState>(2, _omitFieldNames ? '' : 'state',
        enumValues: RemoteOpState.values)
    ..aOB(3, _omitFieldNames ? '' : 'success')
    ..aOS(4, _omitFieldNames ? '' : 'errorMessage')
    ..aInt64(5, _omitFieldNames ? '' : 'completedAtMs')
    ..aOS(6, _omitFieldNames ? '' : 'resultJson')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CommandResult clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CommandResult copyWith(void Function(CommandResult) updates) =>
      super.copyWith((message) => updates(message as CommandResult))
          as CommandResult;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static CommandResult create() => CommandResult._();
  @$core.override
  CommandResult createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static CommandResult getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<CommandResult>(create);
  static CommandResult? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get commandId => $_getSZ(0);
  @$pb.TagNumber(1)
  set commandId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasCommandId() => $_has(0);
  @$pb.TagNumber(1)
  void clearCommandId() => $_clearField(1);

  @$pb.TagNumber(2)
  RemoteOpState get state => $_getN(1);
  @$pb.TagNumber(2)
  set state(RemoteOpState value) => $_setField(2, value);
  @$pb.TagNumber(2)
  $core.bool hasState() => $_has(1);
  @$pb.TagNumber(2)
  void clearState() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.bool get success => $_getBF(2);
  @$pb.TagNumber(3)
  set success($core.bool value) => $_setBool(2, value);
  @$pb.TagNumber(3)
  $core.bool hasSuccess() => $_has(2);
  @$pb.TagNumber(3)
  void clearSuccess() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get errorMessage => $_getSZ(3);
  @$pb.TagNumber(4)
  set errorMessage($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasErrorMessage() => $_has(3);
  @$pb.TagNumber(4)
  void clearErrorMessage() => $_clearField(4);

  @$pb.TagNumber(5)
  $fixnum.Int64 get completedAtMs => $_getI64(4);
  @$pb.TagNumber(5)
  set completedAtMs($fixnum.Int64 value) => $_setInt64(4, value);
  @$pb.TagNumber(5)
  $core.bool hasCompletedAtMs() => $_has(4);
  @$pb.TagNumber(5)
  void clearCompletedAtMs() => $_clearField(5);

  @$pb.TagNumber(6)
  $core.String get resultJson => $_getSZ(5);
  @$pb.TagNumber(6)
  set resultJson($core.String value) => $_setString(5, value);
  @$pb.TagNumber(6)
  $core.bool hasResultJson() => $_has(5);
  @$pb.TagNumber(6)
  void clearResultJson() => $_clearField(6);
}

enum Event_Inner {
  hostStatus,
  runtimeState,
  sessionUpdated,
  streamDelta,
  approvalRequested,
  approvalResolved,
  credentialRotated,
  auditLogged,
  notSet
}

/// Events
class Event extends $pb.GeneratedMessage {
  factory Event({
    $core.String? eventId,
    $fixnum.Int64? timestampMs,
    HostStatusEvent? hostStatus,
    RuntimeStateEvent? runtimeState,
    SessionUpdatedEvent? sessionUpdated,
    StreamDeltaEvent? streamDelta,
    ApprovalRequestedEvent? approvalRequested,
    ApprovalResolvedEvent? approvalResolved,
    CredentialRotatedEvent? credentialRotated,
    AuditLoggedEvent? auditLogged,
  }) {
    final result = create();
    if (eventId != null) result.eventId = eventId;
    if (timestampMs != null) result.timestampMs = timestampMs;
    if (hostStatus != null) result.hostStatus = hostStatus;
    if (runtimeState != null) result.runtimeState = runtimeState;
    if (sessionUpdated != null) result.sessionUpdated = sessionUpdated;
    if (streamDelta != null) result.streamDelta = streamDelta;
    if (approvalRequested != null) result.approvalRequested = approvalRequested;
    if (approvalResolved != null) result.approvalResolved = approvalResolved;
    if (credentialRotated != null) result.credentialRotated = credentialRotated;
    if (auditLogged != null) result.auditLogged = auditLogged;
    return result;
  }

  Event._();

  factory Event.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory Event.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static const $core.Map<$core.int, Event_Inner> _Event_InnerByTag = {
    3: Event_Inner.hostStatus,
    4: Event_Inner.runtimeState,
    5: Event_Inner.sessionUpdated,
    6: Event_Inner.streamDelta,
    7: Event_Inner.approvalRequested,
    8: Event_Inner.approvalResolved,
    9: Event_Inner.credentialRotated,
    10: Event_Inner.auditLogged,
    0: Event_Inner.notSet
  };
  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'Event',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..oo(0, [3, 4, 5, 6, 7, 8, 9, 10])
    ..aOS(1, _omitFieldNames ? '' : 'eventId')
    ..aInt64(2, _omitFieldNames ? '' : 'timestampMs')
    ..aOM<HostStatusEvent>(3, _omitFieldNames ? '' : 'hostStatus',
        subBuilder: HostStatusEvent.create)
    ..aOM<RuntimeStateEvent>(4, _omitFieldNames ? '' : 'runtimeState',
        subBuilder: RuntimeStateEvent.create)
    ..aOM<SessionUpdatedEvent>(5, _omitFieldNames ? '' : 'sessionUpdated',
        subBuilder: SessionUpdatedEvent.create)
    ..aOM<StreamDeltaEvent>(6, _omitFieldNames ? '' : 'streamDelta',
        subBuilder: StreamDeltaEvent.create)
    ..aOM<ApprovalRequestedEvent>(7, _omitFieldNames ? '' : 'approvalRequested',
        subBuilder: ApprovalRequestedEvent.create)
    ..aOM<ApprovalResolvedEvent>(8, _omitFieldNames ? '' : 'approvalResolved',
        subBuilder: ApprovalResolvedEvent.create)
    ..aOM<CredentialRotatedEvent>(9, _omitFieldNames ? '' : 'credentialRotated',
        subBuilder: CredentialRotatedEvent.create)
    ..aOM<AuditLoggedEvent>(10, _omitFieldNames ? '' : 'auditLogged',
        subBuilder: AuditLoggedEvent.create)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Event clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Event copyWith(void Function(Event) updates) =>
      super.copyWith((message) => updates(message as Event)) as Event;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static Event create() => Event._();
  @$core.override
  Event createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static Event getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<Event>(create);
  static Event? _defaultInstance;

  @$pb.TagNumber(3)
  @$pb.TagNumber(4)
  @$pb.TagNumber(5)
  @$pb.TagNumber(6)
  @$pb.TagNumber(7)
  @$pb.TagNumber(8)
  @$pb.TagNumber(9)
  @$pb.TagNumber(10)
  Event_Inner whichInner() => _Event_InnerByTag[$_whichOneof(0)]!;
  @$pb.TagNumber(3)
  @$pb.TagNumber(4)
  @$pb.TagNumber(5)
  @$pb.TagNumber(6)
  @$pb.TagNumber(7)
  @$pb.TagNumber(8)
  @$pb.TagNumber(9)
  @$pb.TagNumber(10)
  void clearInner() => $_clearField($_whichOneof(0));

  @$pb.TagNumber(1)
  $core.String get eventId => $_getSZ(0);
  @$pb.TagNumber(1)
  set eventId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasEventId() => $_has(0);
  @$pb.TagNumber(1)
  void clearEventId() => $_clearField(1);

  @$pb.TagNumber(2)
  $fixnum.Int64 get timestampMs => $_getI64(1);
  @$pb.TagNumber(2)
  set timestampMs($fixnum.Int64 value) => $_setInt64(1, value);
  @$pb.TagNumber(2)
  $core.bool hasTimestampMs() => $_has(1);
  @$pb.TagNumber(2)
  void clearTimestampMs() => $_clearField(2);

  @$pb.TagNumber(3)
  HostStatusEvent get hostStatus => $_getN(2);
  @$pb.TagNumber(3)
  set hostStatus(HostStatusEvent value) => $_setField(3, value);
  @$pb.TagNumber(3)
  $core.bool hasHostStatus() => $_has(2);
  @$pb.TagNumber(3)
  void clearHostStatus() => $_clearField(3);
  @$pb.TagNumber(3)
  HostStatusEvent ensureHostStatus() => $_ensure(2);

  @$pb.TagNumber(4)
  RuntimeStateEvent get runtimeState => $_getN(3);
  @$pb.TagNumber(4)
  set runtimeState(RuntimeStateEvent value) => $_setField(4, value);
  @$pb.TagNumber(4)
  $core.bool hasRuntimeState() => $_has(3);
  @$pb.TagNumber(4)
  void clearRuntimeState() => $_clearField(4);
  @$pb.TagNumber(4)
  RuntimeStateEvent ensureRuntimeState() => $_ensure(3);

  @$pb.TagNumber(5)
  SessionUpdatedEvent get sessionUpdated => $_getN(4);
  @$pb.TagNumber(5)
  set sessionUpdated(SessionUpdatedEvent value) => $_setField(5, value);
  @$pb.TagNumber(5)
  $core.bool hasSessionUpdated() => $_has(4);
  @$pb.TagNumber(5)
  void clearSessionUpdated() => $_clearField(5);
  @$pb.TagNumber(5)
  SessionUpdatedEvent ensureSessionUpdated() => $_ensure(4);

  @$pb.TagNumber(6)
  StreamDeltaEvent get streamDelta => $_getN(5);
  @$pb.TagNumber(6)
  set streamDelta(StreamDeltaEvent value) => $_setField(6, value);
  @$pb.TagNumber(6)
  $core.bool hasStreamDelta() => $_has(5);
  @$pb.TagNumber(6)
  void clearStreamDelta() => $_clearField(6);
  @$pb.TagNumber(6)
  StreamDeltaEvent ensureStreamDelta() => $_ensure(5);

  @$pb.TagNumber(7)
  ApprovalRequestedEvent get approvalRequested => $_getN(6);
  @$pb.TagNumber(7)
  set approvalRequested(ApprovalRequestedEvent value) => $_setField(7, value);
  @$pb.TagNumber(7)
  $core.bool hasApprovalRequested() => $_has(6);
  @$pb.TagNumber(7)
  void clearApprovalRequested() => $_clearField(7);
  @$pb.TagNumber(7)
  ApprovalRequestedEvent ensureApprovalRequested() => $_ensure(6);

  @$pb.TagNumber(8)
  ApprovalResolvedEvent get approvalResolved => $_getN(7);
  @$pb.TagNumber(8)
  set approvalResolved(ApprovalResolvedEvent value) => $_setField(8, value);
  @$pb.TagNumber(8)
  $core.bool hasApprovalResolved() => $_has(7);
  @$pb.TagNumber(8)
  void clearApprovalResolved() => $_clearField(8);
  @$pb.TagNumber(8)
  ApprovalResolvedEvent ensureApprovalResolved() => $_ensure(7);

  @$pb.TagNumber(9)
  CredentialRotatedEvent get credentialRotated => $_getN(8);
  @$pb.TagNumber(9)
  set credentialRotated(CredentialRotatedEvent value) => $_setField(9, value);
  @$pb.TagNumber(9)
  $core.bool hasCredentialRotated() => $_has(8);
  @$pb.TagNumber(9)
  void clearCredentialRotated() => $_clearField(9);
  @$pb.TagNumber(9)
  CredentialRotatedEvent ensureCredentialRotated() => $_ensure(8);

  @$pb.TagNumber(10)
  AuditLoggedEvent get auditLogged => $_getN(9);
  @$pb.TagNumber(10)
  set auditLogged(AuditLoggedEvent value) => $_setField(10, value);
  @$pb.TagNumber(10)
  $core.bool hasAuditLogged() => $_has(9);
  @$pb.TagNumber(10)
  void clearAuditLogged() => $_clearField(10);
  @$pb.TagNumber(10)
  AuditLoggedEvent ensureAuditLogged() => $_ensure(9);
}

class HostStatusEvent extends $pb.GeneratedMessage {
  factory HostStatusEvent({
    $core.String? hostId,
    ConnectorState? state,
    $core.String? version,
  }) {
    final result = create();
    if (hostId != null) result.hostId = hostId;
    if (state != null) result.state = state;
    if (version != null) result.version = version;
    return result;
  }

  HostStatusEvent._();

  factory HostStatusEvent.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory HostStatusEvent.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'HostStatusEvent',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'hostId')
    ..aE<ConnectorState>(2, _omitFieldNames ? '' : 'state',
        enumValues: ConnectorState.values)
    ..aOS(3, _omitFieldNames ? '' : 'version')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  HostStatusEvent clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  HostStatusEvent copyWith(void Function(HostStatusEvent) updates) =>
      super.copyWith((message) => updates(message as HostStatusEvent))
          as HostStatusEvent;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static HostStatusEvent create() => HostStatusEvent._();
  @$core.override
  HostStatusEvent createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static HostStatusEvent getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<HostStatusEvent>(create);
  static HostStatusEvent? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get hostId => $_getSZ(0);
  @$pb.TagNumber(1)
  set hostId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasHostId() => $_has(0);
  @$pb.TagNumber(1)
  void clearHostId() => $_clearField(1);

  @$pb.TagNumber(2)
  ConnectorState get state => $_getN(1);
  @$pb.TagNumber(2)
  set state(ConnectorState value) => $_setField(2, value);
  @$pb.TagNumber(2)
  $core.bool hasState() => $_has(1);
  @$pb.TagNumber(2)
  void clearState() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get version => $_getSZ(2);
  @$pb.TagNumber(3)
  set version($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasVersion() => $_has(2);
  @$pb.TagNumber(3)
  void clearVersion() => $_clearField(3);
}

class RuntimeStateEvent extends $pb.GeneratedMessage {
  factory RuntimeStateEvent({
    $core.String? runtimeId,
    AgentType? agentType,
    RuntimeState? state,
    $core.String? activeProfileId,
    $core.String? details,
  }) {
    final result = create();
    if (runtimeId != null) result.runtimeId = runtimeId;
    if (agentType != null) result.agentType = agentType;
    if (state != null) result.state = state;
    if (activeProfileId != null) result.activeProfileId = activeProfileId;
    if (details != null) result.details = details;
    return result;
  }

  RuntimeStateEvent._();

  factory RuntimeStateEvent.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory RuntimeStateEvent.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'RuntimeStateEvent',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'runtimeId')
    ..aE<AgentType>(2, _omitFieldNames ? '' : 'agentType',
        enumValues: AgentType.values)
    ..aE<RuntimeState>(3, _omitFieldNames ? '' : 'state',
        enumValues: RuntimeState.values)
    ..aOS(4, _omitFieldNames ? '' : 'activeProfileId')
    ..aOS(5, _omitFieldNames ? '' : 'details')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  RuntimeStateEvent clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  RuntimeStateEvent copyWith(void Function(RuntimeStateEvent) updates) =>
      super.copyWith((message) => updates(message as RuntimeStateEvent))
          as RuntimeStateEvent;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static RuntimeStateEvent create() => RuntimeStateEvent._();
  @$core.override
  RuntimeStateEvent createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static RuntimeStateEvent getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<RuntimeStateEvent>(create);
  static RuntimeStateEvent? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get runtimeId => $_getSZ(0);
  @$pb.TagNumber(1)
  set runtimeId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasRuntimeId() => $_has(0);
  @$pb.TagNumber(1)
  void clearRuntimeId() => $_clearField(1);

  @$pb.TagNumber(2)
  AgentType get agentType => $_getN(1);
  @$pb.TagNumber(2)
  set agentType(AgentType value) => $_setField(2, value);
  @$pb.TagNumber(2)
  $core.bool hasAgentType() => $_has(1);
  @$pb.TagNumber(2)
  void clearAgentType() => $_clearField(2);

  @$pb.TagNumber(3)
  RuntimeState get state => $_getN(2);
  @$pb.TagNumber(3)
  set state(RuntimeState value) => $_setField(3, value);
  @$pb.TagNumber(3)
  $core.bool hasState() => $_has(2);
  @$pb.TagNumber(3)
  void clearState() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get activeProfileId => $_getSZ(3);
  @$pb.TagNumber(4)
  set activeProfileId($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasActiveProfileId() => $_has(3);
  @$pb.TagNumber(4)
  void clearActiveProfileId() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get details => $_getSZ(4);
  @$pb.TagNumber(5)
  set details($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasDetails() => $_has(4);
  @$pb.TagNumber(5)
  void clearDetails() => $_clearField(5);
}

class SessionUpdatedEvent extends $pb.GeneratedMessage {
  factory SessionUpdatedEvent({
    $core.String? sessionId,
    $core.String? runtimeId,
    $core.String? title,
    $core.String? status,
    $core.String? credentialProfileId,
    $fixnum.Int64? updatedAtMs,
    $core.String? projectPath,
  }) {
    final result = create();
    if (sessionId != null) result.sessionId = sessionId;
    if (runtimeId != null) result.runtimeId = runtimeId;
    if (title != null) result.title = title;
    if (status != null) result.status = status;
    if (credentialProfileId != null)
      result.credentialProfileId = credentialProfileId;
    if (updatedAtMs != null) result.updatedAtMs = updatedAtMs;
    if (projectPath != null) result.projectPath = projectPath;
    return result;
  }

  SessionUpdatedEvent._();

  factory SessionUpdatedEvent.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory SessionUpdatedEvent.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'SessionUpdatedEvent',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'sessionId')
    ..aOS(2, _omitFieldNames ? '' : 'runtimeId')
    ..aOS(3, _omitFieldNames ? '' : 'title')
    ..aOS(4, _omitFieldNames ? '' : 'status')
    ..aOS(5, _omitFieldNames ? '' : 'credentialProfileId')
    ..aInt64(6, _omitFieldNames ? '' : 'updatedAtMs')
    ..aOS(7, _omitFieldNames ? '' : 'projectPath')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SessionUpdatedEvent clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SessionUpdatedEvent copyWith(void Function(SessionUpdatedEvent) updates) =>
      super.copyWith((message) => updates(message as SessionUpdatedEvent))
          as SessionUpdatedEvent;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static SessionUpdatedEvent create() => SessionUpdatedEvent._();
  @$core.override
  SessionUpdatedEvent createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static SessionUpdatedEvent getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<SessionUpdatedEvent>(create);
  static SessionUpdatedEvent? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get sessionId => $_getSZ(0);
  @$pb.TagNumber(1)
  set sessionId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasSessionId() => $_has(0);
  @$pb.TagNumber(1)
  void clearSessionId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get runtimeId => $_getSZ(1);
  @$pb.TagNumber(2)
  set runtimeId($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasRuntimeId() => $_has(1);
  @$pb.TagNumber(2)
  void clearRuntimeId() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get title => $_getSZ(2);
  @$pb.TagNumber(3)
  set title($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasTitle() => $_has(2);
  @$pb.TagNumber(3)
  void clearTitle() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get status => $_getSZ(3);
  @$pb.TagNumber(4)
  set status($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasStatus() => $_has(3);
  @$pb.TagNumber(4)
  void clearStatus() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get credentialProfileId => $_getSZ(4);
  @$pb.TagNumber(5)
  set credentialProfileId($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasCredentialProfileId() => $_has(4);
  @$pb.TagNumber(5)
  void clearCredentialProfileId() => $_clearField(5);

  @$pb.TagNumber(6)
  $fixnum.Int64 get updatedAtMs => $_getI64(5);
  @$pb.TagNumber(6)
  set updatedAtMs($fixnum.Int64 value) => $_setInt64(5, value);
  @$pb.TagNumber(6)
  $core.bool hasUpdatedAtMs() => $_has(5);
  @$pb.TagNumber(6)
  void clearUpdatedAtMs() => $_clearField(6);

  @$pb.TagNumber(7)
  $core.String get projectPath => $_getSZ(6);
  @$pb.TagNumber(7)
  set projectPath($core.String value) => $_setString(6, value);
  @$pb.TagNumber(7)
  $core.bool hasProjectPath() => $_has(6);
  @$pb.TagNumber(7)
  void clearProjectPath() => $_clearField(7);
}

class StreamDeltaEvent extends $pb.GeneratedMessage {
  factory StreamDeltaEvent({
    $core.String? sessionId,
    $core.String? turnId,
    $core.String? deltaText,
    $core.bool? isFinal,
  }) {
    final result = create();
    if (sessionId != null) result.sessionId = sessionId;
    if (turnId != null) result.turnId = turnId;
    if (deltaText != null) result.deltaText = deltaText;
    if (isFinal != null) result.isFinal = isFinal;
    return result;
  }

  StreamDeltaEvent._();

  factory StreamDeltaEvent.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory StreamDeltaEvent.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'StreamDeltaEvent',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'sessionId')
    ..aOS(2, _omitFieldNames ? '' : 'turnId')
    ..aOS(3, _omitFieldNames ? '' : 'deltaText')
    ..aOB(4, _omitFieldNames ? '' : 'isFinal')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  StreamDeltaEvent clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  StreamDeltaEvent copyWith(void Function(StreamDeltaEvent) updates) =>
      super.copyWith((message) => updates(message as StreamDeltaEvent))
          as StreamDeltaEvent;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static StreamDeltaEvent create() => StreamDeltaEvent._();
  @$core.override
  StreamDeltaEvent createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static StreamDeltaEvent getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<StreamDeltaEvent>(create);
  static StreamDeltaEvent? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get sessionId => $_getSZ(0);
  @$pb.TagNumber(1)
  set sessionId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasSessionId() => $_has(0);
  @$pb.TagNumber(1)
  void clearSessionId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get turnId => $_getSZ(1);
  @$pb.TagNumber(2)
  set turnId($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasTurnId() => $_has(1);
  @$pb.TagNumber(2)
  void clearTurnId() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get deltaText => $_getSZ(2);
  @$pb.TagNumber(3)
  set deltaText($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasDeltaText() => $_has(2);
  @$pb.TagNumber(3)
  void clearDeltaText() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.bool get isFinal => $_getBF(3);
  @$pb.TagNumber(4)
  set isFinal($core.bool value) => $_setBool(3, value);
  @$pb.TagNumber(4)
  $core.bool hasIsFinal() => $_has(3);
  @$pb.TagNumber(4)
  void clearIsFinal() => $_clearField(4);
}

class ApprovalRequestedEvent extends $pb.GeneratedMessage {
  factory ApprovalRequestedEvent({
    $core.String? approvalId,
    $core.String? sessionId,
    $core.String? actionType,
    $core.String? description,
    $core.String? payloadJson,
  }) {
    final result = create();
    if (approvalId != null) result.approvalId = approvalId;
    if (sessionId != null) result.sessionId = sessionId;
    if (actionType != null) result.actionType = actionType;
    if (description != null) result.description = description;
    if (payloadJson != null) result.payloadJson = payloadJson;
    return result;
  }

  ApprovalRequestedEvent._();

  factory ApprovalRequestedEvent.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory ApprovalRequestedEvent.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ApprovalRequestedEvent',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'approvalId')
    ..aOS(2, _omitFieldNames ? '' : 'sessionId')
    ..aOS(3, _omitFieldNames ? '' : 'actionType')
    ..aOS(4, _omitFieldNames ? '' : 'description')
    ..aOS(5, _omitFieldNames ? '' : 'payloadJson')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ApprovalRequestedEvent clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ApprovalRequestedEvent copyWith(
          void Function(ApprovalRequestedEvent) updates) =>
      super.copyWith((message) => updates(message as ApprovalRequestedEvent))
          as ApprovalRequestedEvent;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static ApprovalRequestedEvent create() => ApprovalRequestedEvent._();
  @$core.override
  ApprovalRequestedEvent createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static ApprovalRequestedEvent getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ApprovalRequestedEvent>(create);
  static ApprovalRequestedEvent? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get approvalId => $_getSZ(0);
  @$pb.TagNumber(1)
  set approvalId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasApprovalId() => $_has(0);
  @$pb.TagNumber(1)
  void clearApprovalId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get sessionId => $_getSZ(1);
  @$pb.TagNumber(2)
  set sessionId($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasSessionId() => $_has(1);
  @$pb.TagNumber(2)
  void clearSessionId() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get actionType => $_getSZ(2);
  @$pb.TagNumber(3)
  set actionType($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasActionType() => $_has(2);
  @$pb.TagNumber(3)
  void clearActionType() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get description => $_getSZ(3);
  @$pb.TagNumber(4)
  set description($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasDescription() => $_has(3);
  @$pb.TagNumber(4)
  void clearDescription() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get payloadJson => $_getSZ(4);
  @$pb.TagNumber(5)
  set payloadJson($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasPayloadJson() => $_has(4);
  @$pb.TagNumber(5)
  void clearPayloadJson() => $_clearField(5);
}

class ApprovalResolvedEvent extends $pb.GeneratedMessage {
  factory ApprovalResolvedEvent({
    $core.String? approvalId,
    $core.bool? approved,
    $core.String? resolvedBy,
    $core.String? sessionId,
  }) {
    final result = create();
    if (approvalId != null) result.approvalId = approvalId;
    if (approved != null) result.approved = approved;
    if (resolvedBy != null) result.resolvedBy = resolvedBy;
    if (sessionId != null) result.sessionId = sessionId;
    return result;
  }

  ApprovalResolvedEvent._();

  factory ApprovalResolvedEvent.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory ApprovalResolvedEvent.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ApprovalResolvedEvent',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'approvalId')
    ..aOB(2, _omitFieldNames ? '' : 'approved')
    ..aOS(3, _omitFieldNames ? '' : 'resolvedBy')
    ..aOS(4, _omitFieldNames ? '' : 'sessionId')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ApprovalResolvedEvent clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ApprovalResolvedEvent copyWith(
          void Function(ApprovalResolvedEvent) updates) =>
      super.copyWith((message) => updates(message as ApprovalResolvedEvent))
          as ApprovalResolvedEvent;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static ApprovalResolvedEvent create() => ApprovalResolvedEvent._();
  @$core.override
  ApprovalResolvedEvent createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static ApprovalResolvedEvent getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ApprovalResolvedEvent>(create);
  static ApprovalResolvedEvent? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get approvalId => $_getSZ(0);
  @$pb.TagNumber(1)
  set approvalId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasApprovalId() => $_has(0);
  @$pb.TagNumber(1)
  void clearApprovalId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.bool get approved => $_getBF(1);
  @$pb.TagNumber(2)
  set approved($core.bool value) => $_setBool(1, value);
  @$pb.TagNumber(2)
  $core.bool hasApproved() => $_has(1);
  @$pb.TagNumber(2)
  void clearApproved() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get resolvedBy => $_getSZ(2);
  @$pb.TagNumber(3)
  set resolvedBy($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasResolvedBy() => $_has(2);
  @$pb.TagNumber(3)
  void clearResolvedBy() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get sessionId => $_getSZ(3);
  @$pb.TagNumber(4)
  set sessionId($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasSessionId() => $_has(3);
  @$pb.TagNumber(4)
  void clearSessionId() => $_clearField(4);
}

class CredentialRotatedEvent extends $pb.GeneratedMessage {
  factory CredentialRotatedEvent({
    $core.String? profileId,
    $core.String? poolId,
    $core.String? runtimeId,
    $core.String? reason,
  }) {
    final result = create();
    if (profileId != null) result.profileId = profileId;
    if (poolId != null) result.poolId = poolId;
    if (runtimeId != null) result.runtimeId = runtimeId;
    if (reason != null) result.reason = reason;
    return result;
  }

  CredentialRotatedEvent._();

  factory CredentialRotatedEvent.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory CredentialRotatedEvent.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'CredentialRotatedEvent',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'profileId')
    ..aOS(2, _omitFieldNames ? '' : 'poolId')
    ..aOS(3, _omitFieldNames ? '' : 'runtimeId')
    ..aOS(4, _omitFieldNames ? '' : 'reason')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CredentialRotatedEvent clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CredentialRotatedEvent copyWith(
          void Function(CredentialRotatedEvent) updates) =>
      super.copyWith((message) => updates(message as CredentialRotatedEvent))
          as CredentialRotatedEvent;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static CredentialRotatedEvent create() => CredentialRotatedEvent._();
  @$core.override
  CredentialRotatedEvent createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static CredentialRotatedEvent getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<CredentialRotatedEvent>(create);
  static CredentialRotatedEvent? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get profileId => $_getSZ(0);
  @$pb.TagNumber(1)
  set profileId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasProfileId() => $_has(0);
  @$pb.TagNumber(1)
  void clearProfileId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get poolId => $_getSZ(1);
  @$pb.TagNumber(2)
  set poolId($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasPoolId() => $_has(1);
  @$pb.TagNumber(2)
  void clearPoolId() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get runtimeId => $_getSZ(2);
  @$pb.TagNumber(3)
  set runtimeId($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasRuntimeId() => $_has(2);
  @$pb.TagNumber(3)
  void clearRuntimeId() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get reason => $_getSZ(3);
  @$pb.TagNumber(4)
  set reason($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasReason() => $_has(3);
  @$pb.TagNumber(4)
  void clearReason() => $_clearField(4);
}

class AuditLoggedEvent extends $pb.GeneratedMessage {
  factory AuditLoggedEvent({
    $core.String? auditId,
    $core.String? action,
    $core.String? actor,
    $core.String? target,
    $core.String? outcome,
  }) {
    final result = create();
    if (auditId != null) result.auditId = auditId;
    if (action != null) result.action = action;
    if (actor != null) result.actor = actor;
    if (target != null) result.target = target;
    if (outcome != null) result.outcome = outcome;
    return result;
  }

  AuditLoggedEvent._();

  factory AuditLoggedEvent.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory AuditLoggedEvent.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'AuditLoggedEvent',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'auditId')
    ..aOS(2, _omitFieldNames ? '' : 'action')
    ..aOS(3, _omitFieldNames ? '' : 'actor')
    ..aOS(4, _omitFieldNames ? '' : 'target')
    ..aOS(5, _omitFieldNames ? '' : 'outcome')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  AuditLoggedEvent clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  AuditLoggedEvent copyWith(void Function(AuditLoggedEvent) updates) =>
      super.copyWith((message) => updates(message as AuditLoggedEvent))
          as AuditLoggedEvent;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static AuditLoggedEvent create() => AuditLoggedEvent._();
  @$core.override
  AuditLoggedEvent createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static AuditLoggedEvent getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<AuditLoggedEvent>(create);
  static AuditLoggedEvent? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get auditId => $_getSZ(0);
  @$pb.TagNumber(1)
  set auditId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasAuditId() => $_has(0);
  @$pb.TagNumber(1)
  void clearAuditId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get action => $_getSZ(1);
  @$pb.TagNumber(2)
  set action($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasAction() => $_has(1);
  @$pb.TagNumber(2)
  void clearAction() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get actor => $_getSZ(2);
  @$pb.TagNumber(3)
  set actor($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasActor() => $_has(2);
  @$pb.TagNumber(3)
  void clearActor() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get target => $_getSZ(3);
  @$pb.TagNumber(4)
  set target($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasTarget() => $_has(3);
  @$pb.TagNumber(4)
  void clearTarget() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get outcome => $_getSZ(4);
  @$pb.TagNumber(5)
  set outcome($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasOutcome() => $_has(4);
  @$pb.TagNumber(5)
  void clearOutcome() => $_clearField(5);
}

/// Snapshot
class HostSnapshot extends $pb.GeneratedMessage {
  factory HostSnapshot({
    $core.String? hostId,
    $core.String? hostname,
    ConnectorState? connectorState,
    $core.Iterable<RuntimeInfo>? runtimes,
    $core.Iterable<CredentialProfileInfo>? credentialProfiles,
    $core.Iterable<SessionInfo>? activeSessions,
    $fixnum.Int64? snapshotSequence,
  }) {
    final result = create();
    if (hostId != null) result.hostId = hostId;
    if (hostname != null) result.hostname = hostname;
    if (connectorState != null) result.connectorState = connectorState;
    if (runtimes != null) result.runtimes.addAll(runtimes);
    if (credentialProfiles != null)
      result.credentialProfiles.addAll(credentialProfiles);
    if (activeSessions != null) result.activeSessions.addAll(activeSessions);
    if (snapshotSequence != null) result.snapshotSequence = snapshotSequence;
    return result;
  }

  HostSnapshot._();

  factory HostSnapshot.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory HostSnapshot.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'HostSnapshot',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'hostId')
    ..aOS(2, _omitFieldNames ? '' : 'hostname')
    ..aE<ConnectorState>(3, _omitFieldNames ? '' : 'connectorState',
        enumValues: ConnectorState.values)
    ..pPM<RuntimeInfo>(4, _omitFieldNames ? '' : 'runtimes',
        subBuilder: RuntimeInfo.create)
    ..pPM<CredentialProfileInfo>(5, _omitFieldNames ? '' : 'credentialProfiles',
        subBuilder: CredentialProfileInfo.create)
    ..pPM<SessionInfo>(6, _omitFieldNames ? '' : 'activeSessions',
        subBuilder: SessionInfo.create)
    ..a<$fixnum.Int64>(
        7, _omitFieldNames ? '' : 'snapshotSequence', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  HostSnapshot clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  HostSnapshot copyWith(void Function(HostSnapshot) updates) =>
      super.copyWith((message) => updates(message as HostSnapshot))
          as HostSnapshot;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static HostSnapshot create() => HostSnapshot._();
  @$core.override
  HostSnapshot createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static HostSnapshot getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<HostSnapshot>(create);
  static HostSnapshot? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get hostId => $_getSZ(0);
  @$pb.TagNumber(1)
  set hostId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasHostId() => $_has(0);
  @$pb.TagNumber(1)
  void clearHostId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get hostname => $_getSZ(1);
  @$pb.TagNumber(2)
  set hostname($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasHostname() => $_has(1);
  @$pb.TagNumber(2)
  void clearHostname() => $_clearField(2);

  @$pb.TagNumber(3)
  ConnectorState get connectorState => $_getN(2);
  @$pb.TagNumber(3)
  set connectorState(ConnectorState value) => $_setField(3, value);
  @$pb.TagNumber(3)
  $core.bool hasConnectorState() => $_has(2);
  @$pb.TagNumber(3)
  void clearConnectorState() => $_clearField(3);

  @$pb.TagNumber(4)
  $pb.PbList<RuntimeInfo> get runtimes => $_getList(3);

  @$pb.TagNumber(5)
  $pb.PbList<CredentialProfileInfo> get credentialProfiles => $_getList(4);

  @$pb.TagNumber(6)
  $pb.PbList<SessionInfo> get activeSessions => $_getList(5);

  @$pb.TagNumber(7)
  $fixnum.Int64 get snapshotSequence => $_getI64(6);
  @$pb.TagNumber(7)
  set snapshotSequence($fixnum.Int64 value) => $_setInt64(6, value);
  @$pb.TagNumber(7)
  $core.bool hasSnapshotSequence() => $_has(6);
  @$pb.TagNumber(7)
  void clearSnapshotSequence() => $_clearField(7);
}

class RuntimeInfo extends $pb.GeneratedMessage {
  factory RuntimeInfo({
    $core.String? runtimeId,
    AgentType? agentType,
    $core.String? name,
    RuntimeState? state,
    $core.String? activeCredentialProfileId,
    $core.Iterable<$core.String>? projectPaths,
    $core.bool? connectorManaged,
  }) {
    final result = create();
    if (runtimeId != null) result.runtimeId = runtimeId;
    if (agentType != null) result.agentType = agentType;
    if (name != null) result.name = name;
    if (state != null) result.state = state;
    if (activeCredentialProfileId != null)
      result.activeCredentialProfileId = activeCredentialProfileId;
    if (projectPaths != null) result.projectPaths.addAll(projectPaths);
    if (connectorManaged != null) result.connectorManaged = connectorManaged;
    return result;
  }

  RuntimeInfo._();

  factory RuntimeInfo.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory RuntimeInfo.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'RuntimeInfo',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'runtimeId')
    ..aE<AgentType>(2, _omitFieldNames ? '' : 'agentType',
        enumValues: AgentType.values)
    ..aOS(3, _omitFieldNames ? '' : 'name')
    ..aE<RuntimeState>(4, _omitFieldNames ? '' : 'state',
        enumValues: RuntimeState.values)
    ..aOS(5, _omitFieldNames ? '' : 'activeCredentialProfileId')
    ..pPS(6, _omitFieldNames ? '' : 'projectPaths')
    ..aOB(7, _omitFieldNames ? '' : 'connectorManaged')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  RuntimeInfo clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  RuntimeInfo copyWith(void Function(RuntimeInfo) updates) =>
      super.copyWith((message) => updates(message as RuntimeInfo))
          as RuntimeInfo;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static RuntimeInfo create() => RuntimeInfo._();
  @$core.override
  RuntimeInfo createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static RuntimeInfo getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<RuntimeInfo>(create);
  static RuntimeInfo? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get runtimeId => $_getSZ(0);
  @$pb.TagNumber(1)
  set runtimeId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasRuntimeId() => $_has(0);
  @$pb.TagNumber(1)
  void clearRuntimeId() => $_clearField(1);

  @$pb.TagNumber(2)
  AgentType get agentType => $_getN(1);
  @$pb.TagNumber(2)
  set agentType(AgentType value) => $_setField(2, value);
  @$pb.TagNumber(2)
  $core.bool hasAgentType() => $_has(1);
  @$pb.TagNumber(2)
  void clearAgentType() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get name => $_getSZ(2);
  @$pb.TagNumber(3)
  set name($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasName() => $_has(2);
  @$pb.TagNumber(3)
  void clearName() => $_clearField(3);

  @$pb.TagNumber(4)
  RuntimeState get state => $_getN(3);
  @$pb.TagNumber(4)
  set state(RuntimeState value) => $_setField(4, value);
  @$pb.TagNumber(4)
  $core.bool hasState() => $_has(3);
  @$pb.TagNumber(4)
  void clearState() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get activeCredentialProfileId => $_getSZ(4);
  @$pb.TagNumber(5)
  set activeCredentialProfileId($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasActiveCredentialProfileId() => $_has(4);
  @$pb.TagNumber(5)
  void clearActiveCredentialProfileId() => $_clearField(5);

  @$pb.TagNumber(6)
  $pb.PbList<$core.String> get projectPaths => $_getList(5);

  /// Only the connector decides this from its runtime manifest/profile. Mobile
  /// clients must never infer management from a process name or PID.
  @$pb.TagNumber(7)
  $core.bool get connectorManaged => $_getBF(6);
  @$pb.TagNumber(7)
  set connectorManaged($core.bool value) => $_setBool(6, value);
  @$pb.TagNumber(7)
  $core.bool hasConnectorManaged() => $_has(6);
  @$pb.TagNumber(7)
  void clearConnectorManaged() => $_clearField(7);
}

class CredentialProfileInfo extends $pb.GeneratedMessage {
  factory CredentialProfileInfo({
    $core.String? profileId,
    $core.String? displayName,
    $core.String? provider,
    $core.String? accountFingerprint,
    CredentialStatus? status,
    $fixnum.Int64? lastValidatedAtMs,
  }) {
    final result = create();
    if (profileId != null) result.profileId = profileId;
    if (displayName != null) result.displayName = displayName;
    if (provider != null) result.provider = provider;
    if (accountFingerprint != null)
      result.accountFingerprint = accountFingerprint;
    if (status != null) result.status = status;
    if (lastValidatedAtMs != null) result.lastValidatedAtMs = lastValidatedAtMs;
    return result;
  }

  CredentialProfileInfo._();

  factory CredentialProfileInfo.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory CredentialProfileInfo.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'CredentialProfileInfo',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'profileId')
    ..aOS(2, _omitFieldNames ? '' : 'displayName')
    ..aOS(3, _omitFieldNames ? '' : 'provider')
    ..aOS(4, _omitFieldNames ? '' : 'accountFingerprint')
    ..aE<CredentialStatus>(5, _omitFieldNames ? '' : 'status',
        enumValues: CredentialStatus.values)
    ..aInt64(6, _omitFieldNames ? '' : 'lastValidatedAtMs')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CredentialProfileInfo clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  CredentialProfileInfo copyWith(
          void Function(CredentialProfileInfo) updates) =>
      super.copyWith((message) => updates(message as CredentialProfileInfo))
          as CredentialProfileInfo;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static CredentialProfileInfo create() => CredentialProfileInfo._();
  @$core.override
  CredentialProfileInfo createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static CredentialProfileInfo getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<CredentialProfileInfo>(create);
  static CredentialProfileInfo? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get profileId => $_getSZ(0);
  @$pb.TagNumber(1)
  set profileId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasProfileId() => $_has(0);
  @$pb.TagNumber(1)
  void clearProfileId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get displayName => $_getSZ(1);
  @$pb.TagNumber(2)
  set displayName($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasDisplayName() => $_has(1);
  @$pb.TagNumber(2)
  void clearDisplayName() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get provider => $_getSZ(2);
  @$pb.TagNumber(3)
  set provider($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasProvider() => $_has(2);
  @$pb.TagNumber(3)
  void clearProvider() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get accountFingerprint => $_getSZ(3);
  @$pb.TagNumber(4)
  set accountFingerprint($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasAccountFingerprint() => $_has(3);
  @$pb.TagNumber(4)
  void clearAccountFingerprint() => $_clearField(4);

  @$pb.TagNumber(5)
  CredentialStatus get status => $_getN(4);
  @$pb.TagNumber(5)
  set status(CredentialStatus value) => $_setField(5, value);
  @$pb.TagNumber(5)
  $core.bool hasStatus() => $_has(4);
  @$pb.TagNumber(5)
  void clearStatus() => $_clearField(5);

  @$pb.TagNumber(6)
  $fixnum.Int64 get lastValidatedAtMs => $_getI64(5);
  @$pb.TagNumber(6)
  set lastValidatedAtMs($fixnum.Int64 value) => $_setInt64(5, value);
  @$pb.TagNumber(6)
  $core.bool hasLastValidatedAtMs() => $_has(5);
  @$pb.TagNumber(6)
  void clearLastValidatedAtMs() => $_clearField(6);
}

class SessionInfo extends $pb.GeneratedMessage {
  factory SessionInfo({
    $core.String? sessionId,
    $core.String? runtimeId,
    $core.String? projectPath,
    $core.String? title,
    $core.String? credentialProfileId,
    $core.String? status,
  }) {
    final result = create();
    if (sessionId != null) result.sessionId = sessionId;
    if (runtimeId != null) result.runtimeId = runtimeId;
    if (projectPath != null) result.projectPath = projectPath;
    if (title != null) result.title = title;
    if (credentialProfileId != null)
      result.credentialProfileId = credentialProfileId;
    if (status != null) result.status = status;
    return result;
  }

  SessionInfo._();

  factory SessionInfo.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory SessionInfo.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'SessionInfo',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aOS(1, _omitFieldNames ? '' : 'sessionId')
    ..aOS(2, _omitFieldNames ? '' : 'runtimeId')
    ..aOS(3, _omitFieldNames ? '' : 'projectPath')
    ..aOS(4, _omitFieldNames ? '' : 'title')
    ..aOS(5, _omitFieldNames ? '' : 'credentialProfileId')
    ..aOS(6, _omitFieldNames ? '' : 'status')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SessionInfo clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  SessionInfo copyWith(void Function(SessionInfo) updates) =>
      super.copyWith((message) => updates(message as SessionInfo))
          as SessionInfo;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static SessionInfo create() => SessionInfo._();
  @$core.override
  SessionInfo createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static SessionInfo getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<SessionInfo>(create);
  static SessionInfo? _defaultInstance;

  @$pb.TagNumber(1)
  $core.String get sessionId => $_getSZ(0);
  @$pb.TagNumber(1)
  set sessionId($core.String value) => $_setString(0, value);
  @$pb.TagNumber(1)
  $core.bool hasSessionId() => $_has(0);
  @$pb.TagNumber(1)
  void clearSessionId() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get runtimeId => $_getSZ(1);
  @$pb.TagNumber(2)
  set runtimeId($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasRuntimeId() => $_has(1);
  @$pb.TagNumber(2)
  void clearRuntimeId() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.String get projectPath => $_getSZ(2);
  @$pb.TagNumber(3)
  set projectPath($core.String value) => $_setString(2, value);
  @$pb.TagNumber(3)
  $core.bool hasProjectPath() => $_has(2);
  @$pb.TagNumber(3)
  void clearProjectPath() => $_clearField(3);

  @$pb.TagNumber(4)
  $core.String get title => $_getSZ(3);
  @$pb.TagNumber(4)
  set title($core.String value) => $_setString(3, value);
  @$pb.TagNumber(4)
  $core.bool hasTitle() => $_has(3);
  @$pb.TagNumber(4)
  void clearTitle() => $_clearField(4);

  @$pb.TagNumber(5)
  $core.String get credentialProfileId => $_getSZ(4);
  @$pb.TagNumber(5)
  set credentialProfileId($core.String value) => $_setString(4, value);
  @$pb.TagNumber(5)
  $core.bool hasCredentialProfileId() => $_has(4);
  @$pb.TagNumber(5)
  void clearCredentialProfileId() => $_clearField(5);

  @$pb.TagNumber(6)
  $core.String get status => $_getSZ(5);
  @$pb.TagNumber(6)
  set status($core.String value) => $_setString(5, value);
  @$pb.TagNumber(6)
  $core.bool hasStatus() => $_has(5);
  @$pb.TagNumber(6)
  void clearStatus() => $_clearField(6);
}

/// Ack
class Ack extends $pb.GeneratedMessage {
  factory Ack({
    $fixnum.Int64? sequenceAcknowledged,
    $fixnum.Int64? bootEpoch,
  }) {
    final result = create();
    if (sequenceAcknowledged != null)
      result.sequenceAcknowledged = sequenceAcknowledged;
    if (bootEpoch != null) result.bootEpoch = bootEpoch;
    return result;
  }

  Ack._();

  factory Ack.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory Ack.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'Ack',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..a<$fixnum.Int64>(
        1, _omitFieldNames ? '' : 'sequenceAcknowledged', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..a<$fixnum.Int64>(
        2, _omitFieldNames ? '' : 'bootEpoch', $pb.PbFieldType.OU6,
        defaultOrMaker: $fixnum.Int64.ZERO)
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Ack clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  Ack copyWith(void Function(Ack) updates) =>
      super.copyWith((message) => updates(message as Ack)) as Ack;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static Ack create() => Ack._();
  @$core.override
  Ack createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static Ack getDefault() =>
      _defaultInstance ??= $pb.GeneratedMessage.$_defaultFor<Ack>(create);
  static Ack? _defaultInstance;

  @$pb.TagNumber(1)
  $fixnum.Int64 get sequenceAcknowledged => $_getI64(0);
  @$pb.TagNumber(1)
  set sequenceAcknowledged($fixnum.Int64 value) => $_setInt64(0, value);
  @$pb.TagNumber(1)
  $core.bool hasSequenceAcknowledged() => $_has(0);
  @$pb.TagNumber(1)
  void clearSequenceAcknowledged() => $_clearField(1);

  @$pb.TagNumber(2)
  $fixnum.Int64 get bootEpoch => $_getI64(1);
  @$pb.TagNumber(2)
  set bootEpoch($fixnum.Int64 value) => $_setInt64(1, value);
  @$pb.TagNumber(2)
  $core.bool hasBootEpoch() => $_has(1);
  @$pb.TagNumber(2)
  void clearBootEpoch() => $_clearField(2);
}

/// Error Payload
class ErrorPayload extends $pb.GeneratedMessage {
  factory ErrorPayload({
    $core.int? code,
    $core.String? message,
    $core.bool? outcomeIsUnknown,
  }) {
    final result = create();
    if (code != null) result.code = code;
    if (message != null) result.message = message;
    if (outcomeIsUnknown != null) result.outcomeIsUnknown = outcomeIsUnknown;
    return result;
  }

  ErrorPayload._();

  factory ErrorPayload.fromBuffer($core.List<$core.int> data,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromBuffer(data, registry);
  factory ErrorPayload.fromJson($core.String json,
          [$pb.ExtensionRegistry registry = $pb.ExtensionRegistry.EMPTY]) =>
      create()..mergeFromJson(json, registry);

  static final $pb.BuilderInfo _i = $pb.BuilderInfo(
      _omitMessageNames ? '' : 'ErrorPayload',
      package:
          const $pb.PackageName(_omitMessageNames ? '' : 'muxport.protocol.v1'),
      createEmptyInstance: create)
    ..aI(1, _omitFieldNames ? '' : 'code', fieldType: $pb.PbFieldType.OU3)
    ..aOS(2, _omitFieldNames ? '' : 'message')
    ..aOB(3, _omitFieldNames ? '' : 'outcomeIsUnknown')
    ..hasRequiredFields = false;

  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ErrorPayload clone() => deepCopy();
  @$core.Deprecated('See https://github.com/google/protobuf.dart/issues/998.')
  ErrorPayload copyWith(void Function(ErrorPayload) updates) =>
      super.copyWith((message) => updates(message as ErrorPayload))
          as ErrorPayload;

  @$core.override
  $pb.BuilderInfo get info_ => _i;

  @$core.pragma('dart2js:noInline')
  static ErrorPayload create() => ErrorPayload._();
  @$core.override
  ErrorPayload createEmptyInstance() => create();
  @$core.pragma('dart2js:noInline')
  static ErrorPayload getDefault() => _defaultInstance ??=
      $pb.GeneratedMessage.$_defaultFor<ErrorPayload>(create);
  static ErrorPayload? _defaultInstance;

  @$pb.TagNumber(1)
  $core.int get code => $_getIZ(0);
  @$pb.TagNumber(1)
  set code($core.int value) => $_setUnsignedInt32(0, value);
  @$pb.TagNumber(1)
  $core.bool hasCode() => $_has(0);
  @$pb.TagNumber(1)
  void clearCode() => $_clearField(1);

  @$pb.TagNumber(2)
  $core.String get message => $_getSZ(1);
  @$pb.TagNumber(2)
  set message($core.String value) => $_setString(1, value);
  @$pb.TagNumber(2)
  $core.bool hasMessage() => $_has(1);
  @$pb.TagNumber(2)
  void clearMessage() => $_clearField(2);

  @$pb.TagNumber(3)
  $core.bool get outcomeIsUnknown => $_getBF(2);
  @$pb.TagNumber(3)
  set outcomeIsUnknown($core.bool value) => $_setBool(2, value);
  @$pb.TagNumber(3)
  $core.bool hasOutcomeIsUnknown() => $_has(2);
  @$pb.TagNumber(3)
  void clearOutcomeIsUnknown() => $_clearField(3);
}

const $core.bool _omitFieldNames =
    $core.bool.fromEnvironment('protobuf.omit_field_names');
const $core.bool _omitMessageNames =
    $core.bool.fromEnvironment('protobuf.omit_message_names');
