use crate::{agent::AgentHandle, tools::Tools};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{self, BufRead};
use tokio::sync::{broadcast, mpsc, watch};

const MAX_LINE: usize = 4096;
const HELP: &str =
    "Commands: /status /pause /resume /save /errors /mods /budget /world /reload /help /quit";

#[derive(Clone)]
pub struct Player {
    agent: AgentHandle,
    tools: Tools,
    notices: broadcast::Sender<String>,
    quit: watch::Sender<bool>,
}

impl Player {
    pub fn new(agent: AgentHandle, tools: Tools, notices: broadcast::Sender<String>) -> Self {
        Self {
            agent,
            tools,
            notices,
            quit: watch::channel(false).0,
        }
    }

    pub async fn terminal(&self) -> Result<()> {
        let mut notices = self.notices.subscribe();
        let mut quit = self.quit.subscribe();
        let (sender, mut lines) = mpsc::channel(20);
        std::thread::Builder::new()
            .name("spawn-lite-terminal".into())
            .spawn(move || {
                let mut input = io::stdin().lock();
                loop {
                    match read_line(&mut input) {
                        Ok(None) => break,
                        result => {
                            let finished = result.is_err();
                            if sender.blocking_send(result).is_err() || finished {
                                break;
                            }
                        }
                    }
                }
            })?;

        println!("Spawn-lite. Enter a world request or /help.");
        println!("{}", self.status().await?);
        let signal = tokio::signal::ctrl_c();
        tokio::pin!(signal);
        loop {
            tokio::select! {
                result = &mut signal => { result.context("listening for Ctrl-C")?; break; }
                _ = quit.changed() => break,
                line = lines.recv() => {
                    let Some(line) = line else { break; };
                    let response = match line.context("reading terminal input")? {
                        Some(text) => self.respond(&text).await,
                        None => continue,
                    };
                    if let Some(text) = response { println!("{}", console_text(&text)); }
                }
                notice = notices.recv() => match notice {
                    Ok(text) => println!("{}", console_text(&text)),
                    Err(broadcast::error::RecvError::Lagged(n)) => println!("{n} notices were skipped. Use /status or /errors."),
                    Err(broadcast::error::RecvError::Closed) => break,
                },
            }
        }
        Ok(())
    }

    async fn respond(&self, text: &str) -> Option<String> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let result = if text.len() > MAX_LINE {
            Ok(format!("Input exceeds {MAX_LINE} bytes."))
        } else {
            self.process(text).await
        };
        Some(result.unwrap_or_else(|error| format!("{error:#}")))
    }

    async fn process(&self, text: &str) -> Result<String> {
        let response = match text {
            "/help" => HELP.into(),
            "/status" => self.status().await?,
            "/pause" => {
                self.agent.pause().await?;
                "Agent paused.".into()
            }
            "/resume" => {
                self.agent.resume().await?;
                "Agent resumed.".into()
            }
            "/save" => {
                self.tools.save().await?;
                "World saved.".into()
            }
            "/errors" => {
                self.tool_display("get_recent_errors", json!({"n": 20}))
                    .await?
            }
            "/mods" => self.tool_display("list_mods", json!({})).await?,
            "/budget" => serde_json::to_string_pretty(&self.agent.budget())?,
            "/world" => self.tool_display("get_world_snapshot", json!({})).await?,
            "/reload" => self.tool_display("reload_mods", json!({})).await?,
            "/quit" => {
                self.quit.send_replace(true);
                "Saving and shutting down.".into()
            }
            _ if text.starts_with('/') => format!("Unknown command. {HELP}"),
            _ => format!("Task {} queued.", self.agent.submit(text.to_owned()).await?),
        };
        Ok(response)
    }

    async fn tool_display(&self, action: &str, arguments: Value) -> Result<String> {
        let reply = self.tools.execute(action, arguments).await;
        if reply["ok"] == true {
            return Ok(serde_json::to_string_pretty(&reply["data"])?);
        }
        anyhow::bail!(
            "{}",
            reply["error"]["message"]
                .as_str()
                .unwrap_or("World request failed")
        );
    }

    async fn status(&self) -> Result<String> {
        let state = self.tools.execute("get_world_snapshot", json!({})).await;
        let world = if state["ok"] == true {
            format!(
                "World tick {}, {} entities, {} mods.",
                state["data"]["tick"],
                state["data"]["entities"].as_array().map_or(0, Vec::len),
                state["data"]["mods"].as_array().map_or(0, Vec::len)
            )
        } else {
            "World unavailable.".into()
        };
        Ok(format!(
            "{world}\nAgent {}",
            serde_json::to_string(&self.agent.status())?
        ))
    }
}

fn read_line(input: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut line = Vec::new();
    let mut oversized = false;
    loop {
        let chunk = input.fill_buf()?;
        if chunk.is_empty() {
            return if line.is_empty() && !oversized {
                Ok(None)
            } else {
                Ok(Some(decode_line(line, oversized)))
            };
        }
        let newline = chunk.iter().position(|&byte| byte == b'\n');
        let count = newline.map_or(chunk.len(), |offset| offset + 1);
        if !oversized && line.len() + count <= MAX_LINE + 2 {
            line.extend_from_slice(&chunk[..count]);
        } else {
            oversized = true;
        }
        input.consume(count);
        if newline.is_some() {
            return Ok(Some(decode_line(line, oversized)));
        }
    }
}

fn decode_line(line: Vec<u8>, oversized: bool) -> String {
    if oversized {
        return "x".repeat(MAX_LINE + 1);
    }
    String::from_utf8_lossy(&line)
        .trim_end_matches(['\r', '\n'])
        .to_owned()
}

fn console_text(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
        .collect()
}

#[cfg(feature = "http")]
pub async fn start_http(
    player: Player,
    addr: std::net::SocketAddr,
) -> Result<(
    std::net::SocketAddr,
    tokio::task::JoinHandle<io::Result<()>>,
)> {
    use axum::{routing::get, Router};
    if !addr.ip().is_loopback() {
        anyhow::bail!("WebSocket address must be loopback");
    }
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let address = listener.local_addr()?;
    let state = ws::SocketState {
        player,
        slots: std::sync::Arc::new(tokio::sync::Semaphore::new(8)),
    };
    let router = Router::new()
        .route("/ws", get(ws::upgrade))
        .with_state(state);
    let task = tokio::spawn(async move { axum::serve(listener, router).await });
    Ok((address, task))
}

#[cfg(feature = "http")]
mod ws {
    use super::*;
    use axum::{
        extract::{
            ws::{Message, WebSocket, WebSocketUpgrade},
            State,
        },
        http::{header, HeaderMap, StatusCode},
        response::{IntoResponse, Response},
    };
    use std::{sync::Arc, time::Duration};
    use tokio::sync::Semaphore;

    #[derive(Clone)]
    pub(super) struct SocketState {
        pub player: Player,
        pub slots: Arc<Semaphore>,
    }

    pub(super) async fn upgrade(
        State(state): State<SocketState>,
        headers: HeaderMap,
        handshake: WebSocketUpgrade,
    ) -> Response {
        if !local_origin(&headers) {
            return StatusCode::FORBIDDEN.into_response();
        }
        let Ok(slot) = state.slots.try_acquire_owned() else {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        handshake
            .max_frame_size(MAX_LINE)
            .max_message_size(MAX_LINE)
            .on_upgrade(move |socket| async move {
                let _slot = slot;
                serve(socket, state.player).await;
            })
    }

    fn local_origin(headers: &HeaderMap) -> bool {
        let Some(origin) = headers.get(header::ORIGIN) else {
            return true;
        };
        let Ok(origin) = origin.to_str() else {
            return false;
        };
        let Ok(origin) = reqwest::Url::parse(origin) else {
            return false;
        };
        let host = origin.host_str().unwrap_or_default();
        if !matches!(host, "localhost" | "127.0.0.1" | "[::1]")
            || !matches!(origin.scheme(), "http" | "https")
        {
            return false;
        }
        let authority = match origin.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.into(),
        };
        headers
            .get(header::HOST)
            .and_then(|host| host.to_str().ok())
            == Some(authority.as_str())
    }

    async fn serve(mut socket: WebSocket, player: Player) {
        let mut notices = player.notices.subscribe();
        let mut quit = player.quit.subscribe();
        while !*quit.borrow() {
            let (kind, text) = tokio::select! {
                _ = quit.changed() => break,
                notice = notices.recv() => match notice {
                    Ok(text) => ("notice", text),
                    Err(broadcast::error::RecvError::Lagged(n)) => ("notice", format!("{n} notices were skipped.")),
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                incoming = socket.recv() => match incoming {
                    Some(Ok(Message::Text(text))) => match player.respond(&text).await {
                        Some(text) => ("reply", text),
                        None => continue,
                    },
                    Some(Ok(Message::Binary(_))) => ("reply", "Send a UTF-8 text frame.".into()),
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                    _ => break,
                },
            };
            let response = json!({"type":kind,"text":text}).to_string();
            if !matches!(
                tokio::time::timeout(
                    Duration::from_secs(5),
                    socket.send(Message::Text(response.into()))
                )
                .await,
                Ok(Ok(()))
            ) {
                break;
            }
        }
    }
}
