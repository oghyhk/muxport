use adapter_api::AdapterError;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex};

const MAX_JSONL_BYTES: usize = 4 * 1024 * 1024;
const INCOMING_CAPACITY: usize = 256;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

type BoxWriter = Box<dyn AsyncWrite + Unpin + Send>;
type PendingMap = HashMap<u64, oneshot::Sender<Result<Value, ReplyError>>>;

#[derive(Clone, Debug)]
pub(crate) struct ProcessConfig {
    cmd_path: PathBuf,
    current_dir: Option<PathBuf>,
    clear_env: bool,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
}

impl ProcessConfig {
    pub(crate) fn inherited(cmd_path: impl Into<PathBuf>) -> Self {
        Self {
            cmd_path: cmd_path.into(),
            current_dir: None,
            clear_env: false,
            args: Vec::new(),
            env: Vec::new(),
        }
    }

    pub(crate) fn isolated(
        cmd_path: impl Into<PathBuf>,
        current_dir: impl Into<PathBuf>,
        args: Vec<OsString>,
        env: Vec<(OsString, OsString)>,
    ) -> Self {
        Self {
            cmd_path: cmd_path.into(),
            current_dir: Some(current_dir.into()),
            clear_env: true,
            args,
            env,
        }
    }

    pub(crate) fn cmd_path(&self) -> &Path {
        &self.cmd_path
    }

    pub(crate) fn configure(&self, command: &mut Command) {
        if self.clear_env {
            command.env_clear();
        }
        if let Some(current_dir) = self.current_dir.as_ref() {
            command.current_dir(current_dir);
        }
        command.args(&self.args);
        for (name, value) in &self.env {
            command.env(name, value);
        }
    }

    #[cfg(test)]
    pub(crate) fn environment_value(&self, name: &str) -> Option<&std::ffi::OsStr> {
        self.env
            .iter()
            .find_map(|(key, value)| (key == name).then_some(value.as_os_str()))
    }

    #[cfg(test)]
    pub(crate) fn clears_environment(&self) -> bool {
        self.clear_env
    }
}

#[derive(Debug)]
pub(crate) enum Incoming {
    Message(Value),
    Failure(String),
    Closed,
}

#[derive(Debug)]
enum ReplyError {
    Remote(String),
    Transport(String),
}

/// A supervised Codex App Server JSONL peer.
///
/// Codex uses JSON-RPC 2.0 semantics over newline-delimited stdio, but omits
/// the `jsonrpc` member. The peer owns the child process and fails every
/// outstanding request when framing, transport, or process state is lost.
pub(crate) struct JsonRpcPeer {
    instance_id: String,
    writer: Mutex<Option<BoxWriter>>,
    child: Mutex<Option<Child>>,
    pending: Mutex<PendingMap>,
    next_id: AtomicU64,
    closed: AtomicBool,
    incoming: mpsc::Sender<Incoming>,
    max_jsonl_bytes: usize,
}

impl JsonRpcPeer {
    pub(crate) async fn connect_process(
        process: &ProcessConfig,
    ) -> Result<(Arc<Self>, mpsc::Receiver<Incoming>), AdapterError> {
        let mut command = Command::new(process.cmd_path());
        process.configure(&mut command);
        command
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|error| AdapterError::InitFailed(error.to_string()))?;
        let stdin = child.stdin.take().ok_or_else(|| {
            AdapterError::InitFailed("Codex App Server stdin was not piped".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AdapterError::InitFailed("Codex App Server stdout was not piped".into())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            AdapterError::InitFailed("Codex App Server stderr was not piped".into())
        })?;

        // Drain stderr to prevent the child from blocking. Its contents are
        // intentionally not logged because runtime diagnostics may contain
        // user prompts, paths, or provider details.
        tokio::spawn(async move {
            let mut stderr = stderr;
            let mut buffer = [0_u8; 8192];
            loop {
                match stderr.read(&mut buffer).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
        });

        let (peer, incoming) =
            Self::from_io_with_limit(stdout, stdin, Some(child), MAX_JSONL_BYTES);
        if let Err(error) = peer.initialize().await {
            let _ = peer.shutdown().await;
            return Err(error);
        }
        Ok((peer, incoming))
    }

    fn from_io_with_limit<R, W>(
        reader: R,
        writer: W,
        child: Option<Child>,
        max_jsonl_bytes: usize,
    ) -> (Arc<Self>, mpsc::Receiver<Incoming>)
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (incoming_tx, incoming_rx) = mpsc::channel(INCOMING_CAPACITY);
        let peer = Arc::new(Self {
            instance_id: uuid::Uuid::new_v4().to_string(),
            writer: Mutex::new(Some(Box::new(writer))),
            child: Mutex::new(child),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            incoming: incoming_tx,
            max_jsonl_bytes,
        });
        let reader_peer = Arc::downgrade(&peer);
        tokio::spawn(async move {
            Self::read_loop(reader_peer, reader, max_jsonl_bytes).await;
        });
        (peer, incoming_rx)
    }

    #[cfg(test)]
    pub(crate) fn from_io<R, W>(
        reader: R,
        writer: W,
    ) -> (Arc<Self>, mpsc::Receiver<Incoming>)
    where
        R: AsyncRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        Self::from_io_with_limit(reader, writer, None, MAX_JSONL_BYTES)
    }

    pub(crate) fn instance_id(&self) -> &str {
        &self.instance_id
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub(crate) async fn initialize(&self) -> Result<(), AdapterError> {
        self.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "muxport",
                    "title": "Muxport",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": false
                }
            }),
            false,
        )
        .await?;
        self.send_message(json!({"method": "initialized"}), false)
            .await
    }

    pub(crate) async fn request(
        &self,
        method: &str,
        params: Value,
        mutation: bool,
    ) -> Result<Value, AdapterError> {
        if self.is_closed() {
            return Err(transport_error(mutation, "Codex App Server is closed"));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if id > i64::MAX as u64 {
            return Err(AdapterError::Internal(
                "Codex JSON-RPC request id space was exhausted".into(),
            ));
        }
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(id, sender);

        if let Err(error) = self
            .send_message(json!({"id": id, "method": method, "params": params}), mutation)
            .await
        {
            self.pending.lock().await.remove(&id);
            return Err(error);
        }

        let reply = match tokio::time::timeout(REQUEST_TIMEOUT, receiver).await {
            Ok(Ok(reply)) => reply,
            Ok(Err(_)) => Err(ReplyError::Transport(
                "Codex App Server response channel closed".into(),
            )),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                return Err(transport_error(
                    mutation,
                    &format!("Codex App Server {method} timed out"),
                ));
            }
        };
        match reply {
            Ok(result) => Ok(result),
            Err(ReplyError::Remote(detail)) => Err(AdapterError::Protocol(detail)),
            Err(ReplyError::Transport(detail)) => Err(transport_error(mutation, &detail)),
        }
    }

    pub(crate) async fn respond_result(
        &self,
        id: Value,
        result: Value,
    ) -> Result<(), AdapterError> {
        self.send_message(json!({"id": id, "result": result}), true)
            .await
    }

    pub(crate) async fn respond_error(
        &self,
        id: Value,
        code: i64,
        message: &str,
    ) -> Result<(), AdapterError> {
        self.send_message(
            json!({"id": id, "error": {"code": code, "message": message}}),
            true,
        )
        .await
    }

    async fn send_message(&self, value: Value, mutation: bool) -> Result<(), AdapterError> {
        if self.is_closed() {
            return Err(transport_error(mutation, "Codex App Server is closed"));
        }
        let mut bytes = serde_json::to_vec(&value)
            .map_err(|error| AdapterError::Internal(error.to_string()))?;
        if bytes.len() > self.max_jsonl_bytes {
            return Err(AdapterError::Protocol(format!(
                "Codex JSONL message exceeded {} bytes",
                self.max_jsonl_bytes
            )));
        }
        bytes.push(b'\n');

        let write_result = {
            let mut writer = self.writer.lock().await;
            let writer = writer.as_mut().ok_or_else(|| {
                transport_error(mutation, "Codex App Server stdin is unavailable")
            })?;
            async {
                writer.write_all(&bytes).await?;
                writer.flush().await
            }
            .await
        };
        if let Err(error) = write_result {
            let detail = format!("Codex App Server write failed: {error}");
            self.invalidate(detail.clone()).await;
            return Err(transport_error(mutation, &detail));
        }
        Ok(())
    }

    async fn read_loop<R>(peer: Weak<Self>, mut reader: R, max_jsonl_bytes: usize)
    where
        R: AsyncRead + Unpin,
    {
        let mut chunk = [0_u8; 8192];
        let mut line = Vec::new();
        loop {
            match reader.read(&mut chunk).await {
                Ok(0) => {
                    if !line.is_empty() {
                        let Some(peer) = peer.upgrade() else {
                            return;
                        };
                        if let Err(error) = peer.route_line(&line).await {
                            peer.invalidate(error).await;
                            return;
                        }
                    }
                    if let Some(peer) = peer.upgrade() {
                        peer.invalidate("Codex App Server stdout closed".into()).await;
                    }
                    return;
                }
                Ok(count) => {
                    for byte in &chunk[..count] {
                        if *byte == b'\n' {
                            if line.last() == Some(&b'\r') {
                                line.pop();
                            }
                            if line.is_empty() {
                                if let Some(peer) = peer.upgrade() {
                                    peer.invalidate(
                                        "Codex App Server emitted an empty JSONL frame".into(),
                                    )
                                    .await;
                                }
                                return;
                            }
                            let Some(active_peer) = peer.upgrade() else {
                                return;
                            };
                            if let Err(error) = active_peer.route_line(&line).await {
                                active_peer.invalidate(error).await;
                                return;
                            }
                            line.clear();
                        } else {
                            line.push(*byte);
                            if line.len() > max_jsonl_bytes {
                                if let Some(peer) = peer.upgrade() {
                                    peer.invalidate(format!(
                                        "Codex JSONL frame exceeded {max_jsonl_bytes} bytes"
                                    ))
                                    .await;
                                }
                                return;
                            }
                        }
                    }
                }
                Err(error) => {
                    if let Some(peer) = peer.upgrade() {
                        peer.invalidate(format!("Codex App Server read failed: {error}"))
                            .await;
                    }
                    return;
                }
            }
        }
    }

    async fn route_line(&self, line: &[u8]) -> Result<(), String> {
        let value: Value = serde_json::from_slice(line)
            .map_err(|error| format!("Codex App Server emitted invalid JSON: {error}"))?;
        let object = value
            .as_object()
            .ok_or_else(|| "Codex App Server JSONL frame was not an object".to_owned())?;
        if object.contains_key("method") {
            self.incoming
                .send(Incoming::Message(value))
                .await
                .map_err(|_| "Codex incoming message consumer stopped".to_owned())?;
            return Ok(());
        }

        let id = object
            .get("id")
            .and_then(Value::as_u64)
            .ok_or_else(|| "Codex response had no positive integer id".to_owned())?;
        let reply = if let Some(result) = object.get("result") {
            Ok(result.clone())
        } else if let Some(error) = object.get("error") {
            Err(ReplyError::Remote(format_remote_error(error)))
        } else {
            return Err(format!("Codex response {id} had neither result nor error"));
        };
        if let Some(sender) = self.pending.lock().await.remove(&id) {
            let _ = sender.send(reply);
        }
        Ok(())
    }

    pub(crate) async fn invalidate(&self, detail: String) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        let detail = self.redacted_exit_detail(detail).await;
        self.writer.lock().await.take();
        let pending = std::mem::take(&mut *self.pending.lock().await);
        for (_, sender) in pending {
            let _ = sender.send(Err(ReplyError::Transport(detail.clone())));
        }
        let _ = self.incoming.send(Incoming::Failure(detail)).await;
    }

    /// A closed stdio stream normally means the child has already exited. Keep
    /// only its coarse exit code for recovery diagnostics; stderr can contain
    /// prompts, paths, account information, or provider errors and is never
    /// retained or logged.
    async fn redacted_exit_detail(&self, detail: String) -> String {
        let status = {
            let mut child = self.child.lock().await;
            child
                .as_mut()
                .and_then(|child| child.try_wait().ok().flatten())
        };
        match status {
            Some(status) => match status.code() {
                Some(code) => format!("{detail}; app_server_exit_code={code}"),
                None => format!("{detail}; app_server_exit_signal_or_unknown"),
            },
            None => detail,
        }
    }

    pub(crate) async fn shutdown(&self) -> Result<(), AdapterError> {
        let was_open = !self.closed.swap(true, Ordering::AcqRel);
        self.writer.lock().await.take();
        if was_open {
            let pending = std::mem::take(&mut *self.pending.lock().await);
            for (_, sender) in pending {
                let _ = sender.send(Err(ReplyError::Transport(
                    "Codex App Server is shutting down".into(),
                )));
            }
            let _ = self.incoming.send(Incoming::Closed).await;
        }

        let mut child = self.child.lock().await.take();
        if let Some(child) = child.as_mut() {
            if tokio::time::timeout(SHUTDOWN_TIMEOUT, child.wait())
                .await
                .is_err()
            {
                child
                    .kill()
                    .await
                    .map_err(|error| AdapterError::Internal(error.to_string()))?;
                child
                    .wait()
                    .await
                    .map_err(|error| AdapterError::Internal(error.to_string()))?;
            }
        }
        Ok(())
    }
}

fn format_remote_error(error: &Value) -> String {
    let code = error.get("code").and_then(Value::as_i64);
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unknown Codex App Server error");
    match code {
        Some(code) => format!("Codex App Server error {code}: {message}"),
        None => format!("Codex App Server error: {message}"),
    }
}

fn transport_error(mutation: bool, detail: &str) -> AdapterError {
    if mutation {
        AdapterError::OutcomeUnknown(format!(
            "{detail}; reconcile Codex state before retrying"
        ))
    } else {
        AdapterError::ConnectionLostWithDetail(detail.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{duplex, split, AsyncBufReadExt, AsyncWriteExt, BufReader};

    #[cfg(unix)]
    #[tokio::test]
    async fn process_exit_diagnostic_keeps_only_the_exit_code() {
        let mut command = Command::new("sh");
        command.args(["-c", "exit 7"]);
        let child = command.spawn().unwrap();
        let (client, server) = duplex(64);
        let (reader, writer) = split(client);
        let (_server_reader, _server_writer) = split(server);
        let (peer, mut incoming) =
            JsonRpcPeer::from_io_with_limit(reader, writer, Some(child), 1024);
        for _ in 0..20 {
            if peer
                .child
                .lock()
                .await
                .as_mut()
                .is_some_and(|child| child.try_wait().ok().flatten().is_some())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        peer.invalidate("Codex App Server stdout closed".into()).await;
        let Some(Incoming::Failure(detail)) = incoming.recv().await else {
            panic!("expected a redacted app-server failure diagnostic");
        };
        assert!(detail.contains("app_server_exit_code=7"));
        assert!(!detail.contains("stderr"));
    }

    #[tokio::test]
    async fn performs_required_initialize_handshake() {
        let (client_io, server_io) = duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let (peer, _incoming) = JsonRpcPeer::from_io(client_reader, client_writer);
        let server = tokio::spawn(async move {
            let mut lines = BufReader::new(server_reader).lines();
            let initialize: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(initialize["method"], "initialize");
            assert_eq!(initialize["params"]["clientInfo"]["name"], "muxport");
            server_writer
                .write_all(b"{\"id\":1,\"result\":{\"server\":\"fake\"}}\n")
                .await
                .unwrap();
            let initialized: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(initialized, json!({"method": "initialized"}));
        });

        peer.initialize().await.unwrap();
        server.await.unwrap();
        peer.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn correlates_out_of_order_responses() {
        let (client_io, server_io) = duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let (peer, _incoming) = JsonRpcPeer::from_io(client_reader, client_writer);
        let server = tokio::spawn(async move {
            let mut lines = BufReader::new(server_reader).lines();
            let first: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            let second: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            server_writer
                .write_all(
                    format!(
                        "{{\"id\":{},\"result\":\"second\"}}\n{{\"id\":{},\"result\":\"first\"}}\n",
                        second["id"], first["id"]
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });

        let first = peer.request("first", json!({}), false);
        let second = peer.request("second", json!({}), false);
        let (first, second) = tokio::join!(first, second);
        assert_eq!(first.unwrap(), json!("first"));
        assert_eq!(second.unwrap(), json!("second"));
        server.await.unwrap();
        peer.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn rejects_oversized_incoming_frames() {
        let (client_io, mut server_io) = duplex(128);
        let (client_reader, client_writer) = split(client_io);
        let (peer, mut incoming) =
            JsonRpcPeer::from_io_with_limit(client_reader, client_writer, None, 32);
        server_io.write_all(&vec![b'x'; 33]).await.unwrap();
        match incoming.recv().await.unwrap() {
            Incoming::Failure(detail) => assert!(detail.contains("exceeded 32 bytes")),
            other => panic!("unexpected incoming message: {other:?}"),
        }
        assert!(peer.is_closed());
    }
}
