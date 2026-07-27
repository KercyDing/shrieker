use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::num::NonZeroU16;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const LAN_DESTINATION: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(224, 0, 2, 60), 4445);
const LAN_INTERVAL: Duration = Duration::from_millis(1_500);
const MOTD_CHARS_MAX: usize = 64;
const MOTD_FALLBACK: &str = "shrieker";

/// Periodically advertises a local Java Edition server until stopped.
pub(crate) struct LanBroadcaster {
    stop_tx: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<io::Result<()>>>,
}

impl LanBroadcaster {
    pub(crate) fn start(name: &str, port: NonZeroU16) -> io::Result<Self> {
        let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        socket.set_multicast_loop_v4(true)?;
        socket.set_multicast_ttl_v4(1)?;
        Self::start_with_socket(
            socket,
            SocketAddr::V4(LAN_DESTINATION),
            LAN_INTERVAL,
            build_packet(name, port),
        )
    }

    fn start_with_socket(
        socket: UdpSocket,
        destination: SocketAddr,
        interval: Duration,
        packet: Vec<u8>,
    ) -> io::Result<Self> {
        send_packet(&socket, destination, &packet)?;

        let (stop_tx, stop_rx) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("shrieker-lan-broadcast".to_owned())
            .spawn(move || broadcast_loop(socket, destination, interval, packet, stop_rx))?;
        Ok(Self {
            stop_tx: Some(stop_tx),
            thread: Some(thread),
        })
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }

    pub(crate) fn stop(mut self) -> io::Result<()> {
        self.stop_inner()
    }

    fn stop_inner(&mut self) -> io::Result<()> {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        thread
            .join()
            .map_err(|_| io::Error::other("LAN broadcast thread panicked"))?
    }
}

impl Drop for LanBroadcaster {
    fn drop(&mut self) {
        let _ = self.stop_inner();
    }
}

fn broadcast_loop(
    socket: UdpSocket,
    destination: SocketAddr,
    interval: Duration,
    packet: Vec<u8>,
    stop_rx: mpsc::Receiver<()>,
) -> io::Result<()> {
    loop {
        match stop_rx.recv_timeout(interval) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                send_packet(&socket, destination, &packet)?;
            }
        }
    }
}

fn send_packet(socket: &UdpSocket, destination: SocketAddr, packet: &[u8]) -> io::Result<()> {
    let size = socket.send_to(packet, destination)?;
    if size != packet.len() {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "incomplete LAN broadcast datagram",
        ));
    }
    Ok(())
}

fn build_packet(name: &str, port: NonZeroU16) -> Vec<u8> {
    format!("[MOTD]{}[/MOTD][AD]{}[/AD]", sanitize_name(name), port).into_bytes()
}

fn sanitize_name(name: &str) -> String {
    let cleaned = name
        .chars()
        .take(MOTD_CHARS_MAX)
        .map(|character| match character {
            '[' | ']' => ' ',
            character if character.is_control() => ' ',
            character => character,
        })
        .collect::<String>();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.is_empty() {
        MOTD_FALLBACK.to_owned()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn test_port() -> NonZeroU16 {
        NonZeroU16::new(25_565).unwrap()
    }

    #[test]
    fn builds_java_edition_lan_packet() {
        assert_eq!(
            build_packet("shrieker", test_port()),
            b"[MOTD]shrieker[/MOTD][AD]25565[/AD]"
        );
    }

    #[test]
    fn sanitizes_lan_display_name() {
        assert_eq!(sanitize_name("  房间[一]\n[/MOTD]  "), "房间 一 /MOTD");
        assert_eq!(sanitize_name("\0\n"), MOTD_FALLBACK);

        let long_name = "界".repeat(MOTD_CHARS_MAX + 1);
        assert_eq!(sanitize_name(&long_name).chars().count(), MOTD_CHARS_MAX);
    }

    #[test]
    fn broadcasts_immediately_and_repeatedly() {
        let listener = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let packet = build_packet("shrieker", test_port());
        let broadcaster = LanBroadcaster::start_with_socket(
            sender,
            listener.local_addr().unwrap(),
            Duration::from_millis(20),
            packet.clone(),
        )
        .unwrap();

        let mut buffer = [0_u8; 128];
        for _ in 0..2 {
            let (size, _) = listener.recv_from(&mut buffer).unwrap();
            assert_eq!(&buffer[..size], packet);
        }
        broadcaster.stop().unwrap();
    }

    #[test]
    fn stop_interrupts_broadcast_wait() {
        let listener = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        listener
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let broadcaster = LanBroadcaster::start_with_socket(
            sender,
            listener.local_addr().unwrap(),
            Duration::from_secs(2),
            build_packet("shrieker", test_port()),
        )
        .unwrap();

        let mut buffer = [0_u8; 128];
        listener.recv_from(&mut buffer).unwrap();
        let started = Instant::now();
        broadcaster.stop().unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));

        let error = listener.recv_from(&mut buffer).unwrap_err();
        assert!(matches!(
            error.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ));
    }
}
