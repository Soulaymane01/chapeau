use crate::errors::ChapeauError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Transport protocol for a listening service port.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PortProtocol {
    Tcp,
    Udp,
}

impl PortProtocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }

    pub fn all() -> &'static [PortProtocol] {
        &[PortProtocol::Tcp, PortProtocol::Udp]
    }
}

impl fmt::Display for PortProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PortProtocol {
    type Err = ChapeauError;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "tcp" => Ok(Self::Tcp),
            "udp" => Ok(Self::Udp),
            _ => Err(ChapeauError::Validation(format!(
                "invalid port protocol: {value}"
            ))),
        }
    }
}

/// A listening port attributed to a service.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServicePort {
    pub item_id: String,
    pub protocol: PortProtocol,
    /// Bind address, e.g. `127.0.0.1`, `0.0.0.0`, or `::`.
    pub address: String,
    pub port: u16,
    /// Owning process ID, when visible.
    pub pid: Option<u32>,
    /// Owning process name, when visible.
    pub process_name: Option<String>,
    /// Where the fact came from: `ss`, `socket`, or `config`.
    pub source: String,
    /// ISO-8601 timestamp of the last observation.
    pub last_seen: String,
}

impl ServicePort {
    pub fn new(
        item_id: impl Into<String>,
        protocol: PortProtocol,
        address: impl Into<String>,
        port: u16,
        source: impl Into<String>,
    ) -> Self {
        Self {
            item_id: item_id.into(),
            protocol,
            address: address.into(),
            port,
            pid: None,
            process_name: None,
            source: source.into(),
            last_seen: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn endpoint(&self) -> String {
        if self.address.contains(':') {
            format!("[{}]:{}", self.address, self.port)
        } else {
            format!("{}:{}", self.address, self.port)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_roundtrip() {
        for protocol in PortProtocol::all() {
            assert_eq!(
                PortProtocol::from_str(protocol.as_str()).unwrap(),
                *protocol
            );
        }
    }

    #[test]
    fn invalid_protocol_is_rejected() {
        assert!(PortProtocol::from_str("sctp").is_err());
    }

    #[test]
    fn new_port_has_defaults() {
        let port = ServicePort::new("item-1", PortProtocol::Tcp, "127.0.0.1", 5432, "ss");
        assert_eq!(port.port, 5432);
        assert!(port.pid.is_none());
        assert_eq!(port.source, "ss");
        assert!(!port.last_seen.is_empty());
        assert_eq!(port.endpoint(), "127.0.0.1:5432");
    }

    #[test]
    fn ipv6_endpoint_is_bracketed() {
        let port = ServicePort::new("item-1", PortProtocol::Tcp, "::1", 8080, "ss");
        assert_eq!(port.endpoint(), "[::1]:8080");
    }

    #[test]
    fn serialization_roundtrip() {
        let mut port = ServicePort::new("item-1", PortProtocol::Udp, "0.0.0.0", 53, "ss");
        port.pid = Some(1234);
        port.process_name = Some("dnsmasq".into());
        let json = serde_json::to_string(&port).unwrap();
        let back: ServicePort = serde_json::from_str(&json).unwrap();
        assert_eq!(port, back);
    }
}
