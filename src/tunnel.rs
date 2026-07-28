use eframe::egui;
use sculk::ErrorCategory;
use sculk::minecraft::probe_server;
use sculk::persist::{self, HostState, TokenRefreshSetting};
use sculk::tunnel::{
    HostConfig, HostedServiceHandle, HostedServiceOptions, HostedServiceStatus, NodeOptions,
    SculkNode, SecretKey, ServiceId, TokenRefreshPolicy, TunnelEvent, TunnelUpdate,
};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

const HOST_START_TIMEOUT: Duration = Duration::from_secs(15);
const MC_PROBE_TIMEOUT: Duration = Duration::from_secs(1);
const MC_HEALTH_INTERVAL: Duration = Duration::from_secs(5);
const MC_HEALTH_FAILURES_MAX: u8 = 3;

/// 启动主机隧道所需的固定参数。
pub(crate) struct HostStart {
    pub(crate) mc_port: u16,
    pub(crate) max_players: Option<u32>,
    pub(crate) secret_key: SecretKey,
    pub(crate) relay_url: Option<sculk::tunnel::RelayUrl>,
    pub(crate) token_refresh: TokenRefreshPolicy,
    pub(crate) state_path: PathBuf,
}

/// 主机后台任务产生的状态更新。
pub(crate) enum HostUpdate {
    Started {
        uri: String,
        status: HostedServiceStatus,
    },
    Status(HostedServiceStatus),
    UriChanged {
        uri: String,
        status: HostedServiceStatus,
    },
    Event(TunnelEvent),
    Error(String),
    MinecraftUnavailable,
    Failed(String),
    Stopped(Result<(), String>),
}

enum HostCommand {
    Rotate,
    Stop { done: Option<std_mpsc::Sender<()>> },
}

/// 持有主机后台任务的命令通道。
pub(crate) struct HostTask {
    commands: mpsc::UnboundedSender<HostCommand>,
}

impl HostTask {
    /// 启动主机后台任务。
    pub(crate) fn spawn(
        rt: &tokio::runtime::Runtime,
        start: HostStart,
        updates: mpsc::UnboundedSender<HostUpdate>,
        repaint: egui::Context,
    ) -> Self {
        let (commands, command_rx) = mpsc::unbounded_channel();
        rt.spawn(async move {
            run_host(start, command_rx, updates).await;
            repaint.request_repaint();
        });
        Self { commands }
    }

    /// 请求刷新主机分享地址。
    pub(crate) fn rotate(&self) -> bool {
        self.commands.send(HostCommand::Rotate).is_ok()
    }

    /// 请求停止主机任务，并可在清理完成后通知调用方。
    pub(crate) fn stop(&self, done: Option<std_mpsc::Sender<()>>) -> bool {
        self.commands.send(HostCommand::Stop { done }).is_ok()
    }
}

/// 将 join 服务的异步更新转发到界面逻辑线程。
pub(crate) fn subscribe_join(
    rt: &tokio::runtime::Runtime,
    mut subscription: sculk::tunnel::TunnelSubscription,
    repaint: egui::Context,
) -> mpsc::UnboundedReceiver<TunnelUpdate> {
    let (tx, rx) = mpsc::unbounded_channel();
    rt.spawn(async move {
        while let Some(update) = subscription.recv().await {
            if tx.send(update).is_err() {
                break;
            }
            repaint.request_repaint();
        }
    });
    rx
}

/// 将持久化设置转换为隧道库的令牌刷新策略。
pub(crate) fn token_refresh_policy(setting: TokenRefreshSetting) -> TokenRefreshPolicy {
    match setting {
        TokenRefreshSetting::Always => TokenRefreshPolicy::Always,
        TokenRefreshSetting::Never => TokenRefreshPolicy::Never,
        TokenRefreshSetting::OneHour => TokenRefreshPolicy::After(Duration::from_secs(60 * 60)),
        TokenRefreshSetting::ThreeHours => {
            TokenRefreshPolicy::After(Duration::from_secs(3 * 60 * 60))
        }
        TokenRefreshSetting::SixHours => {
            TokenRefreshPolicy::After(Duration::from_secs(6 * 60 * 60))
        }
        TokenRefreshSetting::TwelveHours => {
            TokenRefreshPolicy::After(Duration::from_secs(12 * 60 * 60))
        }
        TokenRefreshSetting::TwentyFourHours => {
            TokenRefreshPolicy::After(Duration::from_secs(24 * 60 * 60))
        }
    }
}

/// 将隧道事件格式化为人类可读的日志行。
pub(crate) fn format_event(event: &TunnelEvent) -> String {
    match event {
        TunnelEvent::PlayerJoined { id } => t!("player_joined", id = id).to_string(),
        TunnelEvent::PlayerLeft { id, reason } => {
            t!("player_left", id = id, reason = reason).to_string()
        }
        TunnelEvent::Connected => t!("connected_host").to_string(),
        TunnelEvent::Disconnected { reason } => t!("disconnected", reason = reason).to_string(),
        TunnelEvent::PathChanged {
            remote_id,
            is_relay,
            rtt_ms,
        } => {
            let route = if *is_relay {
                t!("relay_route")
            } else {
                t!("direct_route")
            };
            t!("path_changed", id = remote_id, route = route, rtt = rtt_ms).to_string()
        }
        TunnelEvent::Reconnecting { attempt } => t!("reconnecting", n = attempt).to_string(),
        TunnelEvent::Reconnected => t!("reconnected").to_string(),
        TunnelEvent::TokenRotated => t!("token_rotated").to_string(),
        TunnelEvent::TokenRotationFailed { retry_in } => {
            t!("token_rotation_failed", secs = retry_in.as_secs()).to_string()
        }
        TunnelEvent::AuthFailed { id } => t!("auth_failed", id = id).to_string(),
        TunnelEvent::PlayerRejected { id, reason } => {
            t!("rejected", id = id, reason = reason).to_string()
        }
        TunnelEvent::Error { message, .. } => t!("error_msg", msg = message).to_string(),
        other => format!("[?] {other:?}"),
    }
}

async fn run_host(
    start: HostStart,
    mut commands: mpsc::UnboundedReceiver<HostCommand>,
    updates: mpsc::UnboundedSender<HostUpdate>,
) {
    let target_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, start.mc_port));
    if !minecraft_available(target_addr).await {
        let _ = updates.send(HostUpdate::MinecraftUnavailable);
        return;
    }
    let saved = match persist::load_host_state(&start.state_path) {
        Ok(saved) => saved,
        Err(error) => {
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
    };
    let service_id = saved
        .as_ref()
        .map_or_else(ServiceId::generate, |state| state.service_id);
    let token_state = saved.map(|state| state.token_state);
    let node_options = NodeOptions {
        secret_key: Some(start.secret_key),
        relay_url: start.relay_url,
        ..NodeOptions::default()
    };
    let node = match tokio::time::timeout(HOST_START_TIMEOUT, SculkNode::bind(node_options)).await {
        Ok(Ok(node)) => node,
        Ok(Err(error)) => {
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
        Err(_) => {
            let _ = updates.send(HostUpdate::Failed(
                "node startup timed out; check relay settings".to_owned(),
            ));
            return;
        }
    };
    let host = match node
        .start_service(HostedServiceOptions {
            service_id,
            target_addr,
            token_state,
            token_refresh: start.token_refresh,
            config: HostConfig::new().max_players(start.max_players),
        })
        .await
    {
        Ok(host) => host,
        Err(error) => {
            node.close().await;
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
    };
    let mut events = match host.subscribe().await {
        Ok(events) => events,
        Err(error) => {
            node.close().await;
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
    };
    let mut statuses = match host.subscribe_status().await {
        Ok(statuses) => statuses,
        Err(error) => {
            node.close().await;
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
    };
    if let Err(error) = persist_host_state(&start.state_path, &host).await {
        node.close().await;
        let _ = updates.send(HostUpdate::Failed(error));
        return;
    }
    let status = match host.status().await {
        Ok(status) => status,
        Err(error) => {
            node.close().await;
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
    };
    let uri = match expose_host_uri(&host).await {
        Ok(uri) => uri,
        Err(error) => {
            node.close().await;
            let _ = updates.send(HostUpdate::Failed(error));
            return;
        }
    };
    let mut uri_generation = status.uri_generation;
    if updates.send(HostUpdate::Started { uri, status }).is_err() {
        node.close().await;
        return;
    }

    let first_health_check = tokio::time::Instant::now() + MC_HEALTH_INTERVAL;
    let mut health_checks = tokio::time::interval_at(first_health_check, MC_HEALTH_INTERVAL);
    health_checks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut health_failures = 0_u8;
    let mut pending_target_error = None;
    loop {
        tokio::select! {
            command = commands.recv() => {
                match command {
                    Some(HostCommand::Rotate) => {
                        if let Err(error) = host.rotate_token().await {
                            let _ = updates.send(HostUpdate::Error(error.to_string()));
                        }
                    }
                    Some(HostCommand::Stop { done }) => {
                        let result = host.stop().await.map_err(|error| error.to_string());
                        node.close().await;
                        let _ = updates.send(HostUpdate::Stopped(result));
                        if let Some(done) = done {
                            let _ = done.send(());
                        }
                        return;
                    }
                    None => {
                        node.close().await;
                        return;
                    }
                }
            }
            event = events.recv() => {
                match event {
                    Ok(event) => {
                        if is_target_unavailable_event(&event) {
                            pending_target_error.get_or_insert(event);
                        } else {
                            let _ = updates.send(HostUpdate::Event(event));
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(count)) => {
                        let _ = updates.send(HostUpdate::Error(
                            format!("missed {count} host events")
                        ));
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        node.close().await;
                        let _ = updates.send(HostUpdate::Failed(
                            "host event channel closed unexpectedly".to_owned()
                        ));
                        return;
                    }
                }
            }
            status = statuses.recv() => {
                let Some(status) = status else {
                    node.close().await;
                    let _ = updates.send(HostUpdate::Failed(
                        "host status channel closed unexpectedly".to_owned()
                    ));
                    return;
                };
                if status.uri_generation > uri_generation {
                    uri_generation = status.uri_generation;
                    if let Err(error) = persist_host_state(&start.state_path, &host).await {
                        node.close().await;
                        let _ = updates.send(HostUpdate::Failed(error));
                        return;
                    }
                    match expose_host_uri(&host).await {
                        Ok(uri) => {
                            let _ = updates.send(HostUpdate::UriChanged { uri, status });
                        }
                        Err(error) => {
                            node.close().await;
                            let _ = updates.send(HostUpdate::Failed(error));
                            return;
                        }
                    }
                } else {
                    let _ = updates.send(HostUpdate::Status(status));
                }
            }
            _ = health_checks.tick() => {
                let available = minecraft_available(target_addr).await;
                if available
                    && let Some(event) = pending_target_error.take()
                {
                    let _ = updates.send(HostUpdate::Event(event));
                }
                if !record_health_check(&mut health_failures, available) {
                    continue;
                }

                let stop_result = host.stop().await.map_err(|error| error.to_string());
                node.close().await;
                let update = match stop_result {
                    Ok(()) => HostUpdate::MinecraftUnavailable,
                    Err(error) => HostUpdate::Failed(error),
                };
                let _ = updates.send(update);
                return;
            }
        }
    }
}

async fn minecraft_available(addr: SocketAddr) -> bool {
    tokio::task::spawn_blocking(move || probe_server(addr, MC_PROBE_TIMEOUT))
        .await
        .is_ok_and(|result| result.is_ok())
}

fn record_health_check(failures: &mut u8, available: bool) -> bool {
    if available {
        *failures = 0;
        return false;
    }
    *failures = failures.saturating_add(1);
    *failures >= MC_HEALTH_FAILURES_MAX
}

fn is_target_unavailable_event(event: &TunnelEvent) -> bool {
    matches!(
        event,
        TunnelEvent::Error {
            category: ErrorCategory::TargetUnavailable,
            ..
        }
    )
}

async fn persist_host_state(path: &Path, host: &HostedServiceHandle) -> Result<(), String> {
    let token_state = host
        .token_state()
        .await
        .map_err(|error| error.to_string())?;
    persist::save_host_state(
        path,
        &HostState {
            service_id: host.service_id(),
            token_state,
        },
    )
    .map_err(|error| error.to_string())
}

async fn expose_host_uri(host: &HostedServiceHandle) -> Result<String, String> {
    host.join_uri()
        .await
        .map_err(|error| error.to_string())?
        .expose_secret_uri()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_timed_refresh_policy() {
        assert_eq!(
            token_refresh_policy(TokenRefreshSetting::ThreeHours),
            TokenRefreshPolicy::After(Duration::from_secs(3 * 60 * 60))
        );
    }

    #[test]
    fn requires_three_consecutive_minecraft_probe_failures() {
        let mut failures = 0;
        assert!(!record_health_check(&mut failures, false));
        assert!(!record_health_check(&mut failures, false));
        assert!(record_health_check(&mut failures, false));
    }

    #[test]
    fn successful_minecraft_probe_resets_failures() {
        let mut failures = 2;
        assert!(!record_health_check(&mut failures, true));
        assert_eq!(failures, 0);
        assert!(!record_health_check(&mut failures, false));
    }

    #[test]
    fn identifies_target_unavailable_host_errors_only() {
        let target_unavailable = TunnelEvent::Error {
            category: ErrorCategory::TargetUnavailable,
            message: "target unavailable".to_owned(),
        };
        let internal = TunnelEvent::Error {
            category: ErrorCategory::Internal,
            message: "internal error".to_owned(),
        };

        assert!(is_target_unavailable_event(&target_unavailable));
        assert!(!is_target_unavailable_event(&internal));
        assert!(!is_target_unavailable_event(&TunnelEvent::Connected));
    }
}
