//! TCP DNS for explicit resolvers; system and private resolvers stay on the local network.
use super::outbound::{invalid, Destination, Outbound};
use hickory_proto::{
    op::{Message, MessageType, Query, ResponseCode},
    rr::{Name, RData, RecordType},
};
use std::{
    io,
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[derive(Clone)]
pub enum DnsConfig {
    System,
    Tcp(IpAddr),
}
impl DnsConfig {
    pub async fn resolve(&self, host: &str) -> io::Result<Vec<IpAddr>> {
        if let Ok(ip) = host.parse() {
            return Ok(vec![ip]);
        }
        match self {
            Self::System => Ok(tokio::net::lookup_host((host, 0))
                .await?
                .map(|a| a.ip())
                .collect()),
            Self::Tcp(ip) => {
                let (v4, v6) = tokio::join!(
                    Self::query(SocketAddr::new(*ip, 53), host, RecordType::A, None),
                    Self::query(SocketAddr::new(*ip, 53), host, RecordType::AAAA, None)
                );
                let mut addresses = v4.unwrap_or_default();
                addresses.extend(v6.unwrap_or_default());
                if addresses.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "DNS resolution failed",
                    ));
                }
                Ok(addresses)
            }
        }
    }
    pub async fn resolve_proxied(
        &self,
        host: &str,
        outbound: &Outbound,
    ) -> io::Result<Vec<IpAddr>> {
        let server = match self {
            Self::System => return self.resolve(host).await,
            Self::Tcp(ip) => SocketAddr::new(*ip, 53),
        };
        self.resolve_at(server, host, outbound).await
    }

    async fn resolve_at(
        &self,
        server: SocketAddr,
        host: &str,
        outbound: &Outbound,
    ) -> io::Result<Vec<IpAddr>> {
        let proxy = (!local_resolver(server.ip())).then_some((outbound, self));
        let (v4, v6) = tokio::join!(
            Self::query(server, host, RecordType::A, proxy),
            Self::query(server, host, RecordType::AAAA, proxy)
        );
        let mut addresses = v4.unwrap_or_default();
        addresses.extend(v6.unwrap_or_default());
        if addresses.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Proxied DNS resolution failed",
            ));
        }
        Ok(addresses)
    }
    async fn query(
        server: SocketAddr,
        host: &str,
        kind: RecordType,
        proxy: Option<(&Outbound, &DnsConfig)>,
    ) -> io::Result<Vec<IpAddr>> {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut request = Message::new();
            let id = u16::from_be_bytes(
                uuid::Uuid::new_v4().as_bytes()[..2]
                    .try_into()
                    .map_err(|_| invalid())?,
            );
            let mut name = Name::from_ascii(host).map_err(|_| invalid())?;
            name.set_fqdn(true);
            request
                .set_id(id)
                .set_recursion_desired(true)
                .add_query(Query::query(name.clone(), kind));
            let bytes = request.to_vec().map_err(|_| invalid())?;
            let mut stream = if let Some((outbound, dns)) = proxy {
                Box::pin(outbound.connect_tcp(
                    &Destination::new(server.ip().to_string(), server.port())?,
                    dns,
                ))
                .await?
            } else {
                TcpStream::connect(server).await?
            };
            stream.write_u16(bytes.len() as u16).await?;
            stream.write_all(&bytes).await?;
            let size = stream.read_u16().await?;
            let mut bytes = vec![0; size as usize];
            stream.read_exact(&mut bytes).await?;
            let response = Message::from_vec(&bytes).map_err(|_| invalid())?;
            if response.id() != id
                || response.message_type() != MessageType::Response
                || response.truncated()
                || response.response_code() != ResponseCode::NoError
                || response.queries() != request.queries()
            {
                return Err(invalid());
            }
            // Only accept answer owners in the queried CNAME chain.
            let mut names = vec![name];
            for _ in 0..16 {
                let mut next = Vec::new();
                for record in response.answers() {
                    if names.contains(record.name()) {
                        if let RData::CNAME(alias) = record.data() {
                            if !names.contains(&alias.0) {
                                next.push(alias.0.clone());
                            }
                        }
                    }
                }
                if next.is_empty() {
                    break;
                }
                names.extend(next);
            }
            Ok(response
                .answers()
                .iter()
                .filter(|r| names.contains(r.name()))
                .filter_map(|r| match r.data() {
                    RData::A(ip) if kind == RecordType::A => Some(IpAddr::V4(ip.0)),
                    RData::AAAA(ip) if kind == RecordType::AAAA => Some(IpAddr::V6(ip.0)),
                    _ => None,
                })
                .collect())
        })
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "DNS query timed out"))?
    }
    pub async fn connect(&self, destination: &Destination) -> io::Result<TcpStream> {
        let addresses = self.resolve(&destination.host).await?;
        for ip in addresses {
            if let Ok(Ok(stream)) = tokio::time::timeout(
                Duration::from_secs(3),
                TcpStream::connect(SocketAddr::new(ip, destination.port)),
            )
            .await
            {
                return Ok(stream);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            "Destination is unavailable",
        ))
    }
}

fn local_resolver(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickory_proto::rr::{rdata::A, Record};

    #[tokio::test]
    async fn system_dns_does_not_depend_on_remote_proxy() {
        let addresses = DnsConfig::System
            .resolve_proxied("localhost", &Outbound::Block)
            .await
            .unwrap();
        assert!(!addresses.is_empty());
        assert!(addresses.iter().all(|ip| ip.is_loopback()));
    }

    #[tokio::test]
    async fn private_dns_is_direct_and_public_dns_uses_proxy() {
        for proxied in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                for _ in 0..2 {
                    let (mut stream, _) = listener.accept().await.unwrap();
                    if proxied {
                        let headers = super::super::outbound::read_headers(&mut stream)
                            .await
                            .unwrap();
                        assert!(headers.starts_with("CONNECT 1.1.1.1:53 HTTP/1.1"));
                        stream
                            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                            .await
                            .unwrap();
                    }
                    let size = stream.read_u16().await.unwrap();
                    let mut bytes = vec![0; size as usize];
                    stream.read_exact(&mut bytes).await.unwrap();
                    let mut response = Message::from_vec(&bytes).unwrap();
                    let kind = response.queries()[0].query_type();
                    let name = response.queries()[0].name().clone();
                    response
                        .set_message_type(MessageType::Response)
                        .set_recursion_available(true);
                    if kind == RecordType::A {
                        response.add_answer(Record::from_rdata(
                            name,
                            30,
                            RData::A(A("192.0.2.7".parse().unwrap())),
                        ));
                    }
                    let bytes = response.to_vec().unwrap();
                    stream.write_u16(bytes.len() as u16).await.unwrap();
                    stream.write_all(&bytes).await.unwrap();
                }
            });
            let outbound = if proxied {
                Outbound::Http(
                    crate::subscription::uri::parse(&format!("http-proxy://{address}"), "dns-test")
                        .unwrap(),
                )
            } else {
                Outbound::Block // Direct DNS must work even when proxy traffic is blocked.
            };
            let resolver = if proxied {
                "1.1.1.1:53".parse().unwrap()
            } else {
                address
            };
            let addresses = DnsConfig::Tcp(resolver.ip())
                .resolve_at(resolver, "native.example", &outbound)
                .await
                .unwrap();
            assert_eq!(addresses, vec!["192.0.2.7".parse::<IpAddr>().unwrap()]);
            server.await.unwrap();
        }
    }
    #[tokio::test]
    async fn tcp_dns_checks_transaction_question_and_compressed_answers() {
        for wrong_id in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let size = stream.read_u16().await.unwrap();
                let mut bytes = vec![0; size as usize];
                stream.read_exact(&mut bytes).await.unwrap();
                let mut response = Message::from_vec(&bytes).unwrap();
                assert_eq!(response.queries()[0].name().to_ascii(), "native.example.");
                if wrong_id {
                    response.set_id(response.id().wrapping_add(1));
                }
                response
                    .set_message_type(MessageType::Response)
                    .set_recursion_available(true);
                response.add_answer(Record::from_rdata(
                    Name::from_ascii("native.example").unwrap(),
                    30,
                    RData::A(A("192.0.2.7".parse().unwrap())),
                ));
                let bytes = response.to_vec().unwrap();
                stream.write_u16(bytes.len() as u16).await.unwrap();
                stream.write_all(&bytes).await.unwrap();
            });
            let result = DnsConfig::query(address, "native.example", RecordType::A, None).await;
            if wrong_id {
                assert!(result.is_err());
            } else {
                assert_eq!(
                    result.unwrap(),
                    vec!["192.0.2.7".parse::<IpAddr>().unwrap()]
                );
            }
            server.await.unwrap();
        }
    }
}
