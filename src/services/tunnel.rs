use crate::app::UiMsg;
use sculk::tunnel::{HostConfig, IrohTunnel, JoinConfig, SecretKey, Ticket};
use sculk::types::RelayUrl;
use std::sync::Arc;
use tokio::sync::mpsc;

/// 异步启动 Host 隧道，通过 tx 回传结果。
pub fn spawn_host(
    rt: &tokio::runtime::Runtime,
    tx: mpsc::UnboundedSender<UiMsg>,
    port: u16,
    secret_key: Option<SecretKey>,
    relay_url: Option<RelayUrl>,
    config: HostConfig,
) {
    rt.spawn(async move {
        match IrohTunnel::host(port, secret_key, relay_url, config).await {
            Ok((tunnel, ticket, events)) => {
                let ticket_str = ticket.to_string();
                let _ = tx.send(UiMsg::Log(t!("host_ready").to_string()));
                let _ = tx.send(UiMsg::HostReady {
                    tunnel: Arc::new(tunnel),
                    ticket: ticket_str,
                    events,
                });
            }
            Err(e) => {
                let _ = tx.send(UiMsg::Log(t!("host_failed", err = e).to_string()));
            }
        }
    });
}

/// 异步加入隧道，通过 tx 回传结果。
pub fn spawn_join(
    rt: &tokio::runtime::Runtime,
    tx: mpsc::UnboundedSender<UiMsg>,
    ticket_str: String,
    port: u16,
    config: JoinConfig,
) {
    rt.spawn(async move {
        let ticket = match ticket_str.parse::<Ticket>() {
            Ok(t) => t,
            Err(e) => {
                let _ = tx.send(UiMsg::Log(t!("invalid_ticket", err = e).to_string()));
                return;
            }
        };
        match IrohTunnel::join(&ticket, port, config).await {
            Ok((tunnel, events)) => {
                let _ = tx.send(UiMsg::Log(t!("joined").to_string()));
                let _ = tx.send(UiMsg::JoinReady {
                    tunnel: Arc::new(tunnel),
                    events,
                });
            }
            Err(e) => {
                let _ = tx.send(UiMsg::Log(t!("join_failed", err = e).to_string()));
            }
        }
    });
}

/// 异步关闭隧道，通过 tx 回传关闭消息。
pub fn spawn_close(
    rt: &tokio::runtime::Runtime,
    tx: mpsc::UnboundedSender<UiMsg>,
    tunnel: Arc<IrohTunnel>,
) {
    rt.spawn(async move {
        tunnel.close().await;
        let _ = tx.send(UiMsg::Log(t!("tunnel_closed").to_string()));
    });
}
