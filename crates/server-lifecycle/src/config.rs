use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// Listener bind configuration shared by TCP and HTTP servers.
/// @spec apps/agentic-workflow/tech-design/logic/shared-server-substrate-performance-layers.md#logic
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindConfig {
    host: IpAddr,
    port: u16,
}

impl BindConfig {
    /// Bind on `host` and `port`.
    pub const fn new(host: IpAddr, port: u16) -> Self {
        Self { host, port }
    }

    /// Bind on all IPv4 interfaces.
    pub fn any(port: u16) -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port,
        }
    }

    /// Bind on localhost.
    pub fn localhost(port: u16) -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
        }
    }

    /// The address to bind.
    pub const fn host(&self) -> IpAddr {
        self.host
    }

    /// The port to bind; 0 asks the OS for a free one.
    pub const fn port(&self) -> u16 {
        self.port
    }

    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}

impl From<SocketAddr> for BindConfig {
    fn from(addr: SocketAddr) -> Self {
        Self::new(addr.ip(), addr.port())
    }
}

impl Default for BindConfig {
    fn default() -> Self {
        Self::any(0)
    }
}
