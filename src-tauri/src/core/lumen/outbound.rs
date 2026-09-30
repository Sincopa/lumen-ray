//! Native TCP outbounds. SOCKS negotiation follows RFC 1928 and RFC 1929.
use super::dns::DnsConfig;
use crate::domain::node::{Protocol, VpnNode};
use base64::Engine;
use std::{io, net::IpAddr};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid proxy message")
}

#[derive(Clone, Debug)]
pub struct Destination {
    pub host: String,
    pub port: u16,
}
impl Destination {
    pub fn new(host: String, port: u16) -> io::Result<Self> {
        if port == 0
            || host.is_empty()
            || host.len() > 253
            || host.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            return Err(invalid());
        }
        let host = host.trim_matches(['[', ']']).to_string();
        let host = if host.parse::<IpAddr>().is_ok() {
            host
        } else {
            match url::Host::parse(&host).map_err(|_| invalid())? {
                url::Host::Domain(name) => name.trim_end_matches('.').to_lowercase(),
                _ => return Err(invalid()),
            }
        };
        Ok(Self { host, port })
    }
    pub fn authority(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

#[derive(Clone)]
pub enum Outbound {
    Direct,
    Block,
    Socks(VpnNode),
    Http(VpnNode),
}
impl Outbound {
    pub async fn connect_tcp(
        &self,
        destination: &Destination,
        dns: &DnsConfig,
    ) -> io::Result<TcpStream> {
        match self {
            Self::Block => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "Blocked by routing rule",
            )),
            Self::Direct => dns.connect(destination).await,
            Self::Socks(node) => {
                let mut stream = dns
                    .connect(&Destination::new(node.address.clone(), node.port)?)
                    .await?;
                socks_connect(&mut stream, destination, &node.method, &node.credential).await?;
                Ok(stream)
            }
            Self::Http(node) => {
                let mut stream = dns
                    .connect(&Destination::new(node.address.clone(), node.port)?)
                    .await?;
                let authority = destination.authority();
                let auth = if node.method.is_empty() {
                    String::new()
                } else {
                    format!(
                        "Proxy-Authorization: Basic {}\r\n",
                        base64::engine::general_purpose::STANDARD
                            .encode(format!("{}:{}", node.method, node.credential))
                    )
                };
                stream
                    .write_all(
                        format!("CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n{auth}\r\n")
                            .as_bytes(),
                    )
                    .await?;
                let headers = read_headers(&mut stream).await?;
                let status = headers.split("\r\n").next().ok_or_else(invalid)?;
                let mut fields = status.split_whitespace();
                if !matches!(fields.next(), Some("HTTP/1.0" | "HTTP/1.1"))
                    || !fields
                        .next()
                        .and_then(|field| field.parse::<u16>().ok())
                        .is_some_and(|s| (200..300).contains(&s))
                {
                    return Err(invalid());
                }
                Ok(stream)
            }
        }
    }
    pub fn from_node(node: &VpnNode) -> Option<Self> {
        match node.protocol {
            Protocol::Socks => Some(Self::Socks(node.clone())),
            Protocol::Http => Some(Self::Http(node.clone())),
            _ => None,
        }
    }
}

pub async fn read_headers(stream: &mut TcpStream) -> io::Result<String> {
    // Read only through the header terminator: tunnel bytes must remain unread.
    const MAX_HEADERS: usize = 16_384;
    let mut bytes = Vec::with_capacity(1024);
    let mut buffer = [0; 1024];
    while bytes.len() < MAX_HEADERS {
        let limit = buffer.len().min(MAX_HEADERS - bytes.len());
        let count = stream.peek(&mut buffer[..limit]).await?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Incomplete proxy headers",
            ));
        }
        let previous = bytes.len();
        bytes.extend_from_slice(&buffer[..count]);
        // Include the previous three bytes to find a terminator split across reads.
        let start = previous.saturating_sub(3);
        let end = bytes[start..]
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|offset| start + offset + 4);
        let consumed = end.map_or(count, |end| end - previous);
        stream.read_exact(&mut buffer[..consumed]).await?;
        if let Some(end) = end {
            bytes.truncate(end);
            return String::from_utf8(bytes).map_err(|_| invalid());
        }
    }
    Err(invalid())
}

pub async fn read_address(stream: &mut TcpStream, atyp: u8) -> io::Result<String> {
    Ok(match atyp {
        1 => {
            let mut b = [0; 4];
            stream.read_exact(&mut b).await?;
            std::net::Ipv4Addr::from(b).to_string()
        }
        4 => {
            let mut b = [0; 16];
            stream.read_exact(&mut b).await?;
            std::net::Ipv6Addr::from(b).to_string()
        }
        3 => {
            let size = stream.read_u8().await?;
            if size == 0 {
                return Err(invalid());
            }
            let mut b = vec![0; size as usize];
            stream.read_exact(&mut b).await?;
            String::from_utf8(b).map_err(|_| invalid())?
        }
        _ => return Err(invalid()),
    })
}
pub fn encode_address(destination: &Destination, bytes: &mut Vec<u8>) {
    match destination.host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            bytes.push(1);
            bytes.extend(ip.octets());
        }
        Ok(IpAddr::V6(ip)) => {
            bytes.push(4);
            bytes.extend(ip.octets());
        }
        Err(_) => {
            bytes.extend([3, destination.host.len() as u8]);
            bytes.extend(destination.host.as_bytes());
        }
    }
    bytes.extend(destination.port.to_be_bytes());
}
pub async fn socks_connect(
    stream: &mut TcpStream,
    destination: &Destination,
    username: &str,
    password: &str,
) -> io::Result<()> {
    let auth = if username.is_empty() { 0 } else { 2 };
    stream.write_all(&[5, 1, auth]).await?;
    let mut reply = [0; 2];
    stream.read_exact(&mut reply).await?;
    if reply != [5, auth] {
        return Err(invalid());
    }
    if auth == 2 {
        if username.len() > 255 || password.len() > 255 || password.is_empty() {
            return Err(invalid());
        }
        let mut request = vec![1, username.len() as u8];
        request.extend(username.as_bytes());
        request.push(password.len() as u8);
        request.extend(password.as_bytes());
        stream.write_all(&request).await?;
        stream.read_exact(&mut reply).await?;
        if reply != [1, 0] {
            return Err(invalid());
        }
    }
    let mut request = vec![5, 1, 0];
    encode_address(destination, &mut request);
    stream.write_all(&request).await?;
    let mut header = [0; 4];
    stream.read_exact(&mut header).await?;
    if header[..3] != [5, 0, 0] {
        return Err(invalid());
    }
    read_address(stream, header[3]).await?;
    stream.read_u16().await?;
    Ok(())
}

#[cfg(test)]
mod header_tests {
    use super::*;

    async fn streams() -> (TcpStream, TcpStream) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (client, server) = tokio::join!(
            TcpStream::connect(listener.local_addr().unwrap()),
            listener.accept()
        );
        (client.unwrap(), server.unwrap().0)
    }

    #[tokio::test]
    async fn header_boundaries_preserve_coalesced_tunnel_bytes() {
        for length in [4, 1023, 1024, 1025, 2049, 16_384] {
            let (mut client, mut server) = streams().await;
            let head = format!("{}\r\n\r\n", "x".repeat(length - 4));
            server
                .write_all(format!("{head}payload").as_bytes())
                .await
                .unwrap();
            assert_eq!(read_headers(&mut client).await.unwrap(), head);
            let mut payload = [0; 7];
            client.read_exact(&mut payload).await.unwrap();
            assert_eq!(&payload, b"payload");
        }
    }

    #[tokio::test]
    async fn fragmented_headers_limits_invalid_utf8_and_eof() {
        let (mut client, mut server) = streams().await;
        let sender = tokio::spawn(async move {
            for byte in b"HTTP/1.1 200 OK\r\n\r\ndata" {
                server.write_all(&[*byte]).await.unwrap();
                tokio::task::yield_now().await;
            }
        });
        assert_eq!(
            read_headers(&mut client).await.unwrap(),
            "HTTP/1.1 200 OK\r\n\r\n"
        );
        let mut data = [0; 4];
        client.read_exact(&mut data).await.unwrap();
        assert_eq!(&data, b"data");
        sender.await.unwrap();
        for (bytes, kind) in [
            (vec![b'x'; 16_384], io::ErrorKind::InvalidData),
            (b"\xff\r\n\r\n".to_vec(), io::ErrorKind::InvalidData),
            (
                b"HTTP/1.1 200 OK\r\n\r".to_vec(),
                io::ErrorKind::UnexpectedEof,
            ),
        ] {
            let (mut client, mut server) = streams().await;
            server.write_all(&bytes).await.unwrap();
            server.shutdown().await.unwrap();
            assert_eq!(read_headers(&mut client).await.unwrap_err().kind(), kind);
        }
    }
}
