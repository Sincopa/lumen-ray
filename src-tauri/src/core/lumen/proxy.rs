use super::{
    dns::DnsConfig,
    outbound::{invalid, read_address, read_headers, Destination},
    routing::RoutingTable,
    Counters,
};
use std::{
    io,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::RwLock,
    task::JoinSet,
};

pub async fn serve(
    listener: TcpListener,
    routing: Arc<RwLock<Arc<RoutingTable>>>,
    dns: DnsConfig,
    counters: Arc<Counters>,
    mut stop: tokio::sync::watch::Receiver<bool>,
) {
    // Dropping the JoinSet aborts all flows, including pending handshakes and DNS.
    let mut flows = JoinSet::new();
    loop {
        tokio::select! {
            _ = stop.changed() => { flows.shutdown().await; break; }
            result = listener.accept(), if flows.len() < 256 => {
                let Ok((stream,_)) = result else { break; };
                let table = routing.read().await.clone();
                let dns = dns.clone(); let counters = counters.clone();
                flows.spawn(async move { let _ = handle(stream,table,dns,counters).await; });
            }
            _ = flows.join_next(), if !flows.is_empty() => {},
        }
    }
}
async fn connect(
    table: &RoutingTable,
    dns: &DnsConfig,
    destination: &Destination,
) -> io::Result<TcpStream> {
    let addresses = if table.needs_ip_lookup(destination) {
        dns.resolve_proxied(&destination.host, &table.outbounds[2])
            .await?
    } else {
        vec![]
    };
    if addresses.is_empty() {
        let target = table.target(destination, &[]);
        return table.outbounds[target].connect_tcp(destination, dns).await;
    }
    // Match and pin each attempted address. Never retry a blocked address via
    // another DNS answer, but allow IPv4 fallback when IPv6 is unavailable.
    let mut failure = invalid();
    for ip in addresses {
        let target = table.target(destination, &[ip]);
        let endpoint = Destination {
            host: ip.to_string(),
            port: destination.port,
        };
        match table.outbounds[target].connect_tcp(&endpoint, dns).await {
            Ok(stream) => return Ok(stream),
            Err(error) if error.kind() == io::ErrorKind::PermissionDenied => return Err(error),
            Err(error) => failure = error,
        }
    }
    Err(failure)
}
async fn handle(
    mut client: TcpStream,
    table: Arc<RoutingTable>,
    dns: DnsConfig,
    counters: Arc<Counters>,
) -> io::Result<()> {
    let Connected {
        mut upstream,
        forward_body,
    } = tokio::time::timeout(Duration::from_secs(15), async {
        let mut first = [0];
        if client.peek(&mut first).await? == 0 {
            return Err(invalid());
        }
        if first[0] == 5 {
            Ok(Connected {
                upstream: socks(&mut client, &table, &dns).await?,
                forward_body: None,
            })
        } else {
            http(&mut client, &table, &dns, &counters).await
        }
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Proxy handshake timed out"))??;
    let (cr, cw) = client.split();
    let (ur, uw) = upstream.split();
    if let Some(length) = forward_body {
        tokio::try_join!(
            copy_body(cr, uw, length, &counters.upload),
            copy(ur, cw, &counters.download)
        )?;
    } else {
        tokio::try_join!(
            copy(cr, uw, &counters.upload),
            copy(ur, cw, &counters.download)
        )?;
    }
    Ok(())
}
struct Connected {
    upstream: TcpStream,
    // A forward HTTP request has exactly one body. CONNECT and SOCKS are tunnels.
    forward_body: Option<u64>,
}
async fn copy_body(
    reader: impl AsyncRead + Unpin,
    mut writer: impl AsyncWrite + Unpin,
    length: u64,
    counter: &AtomicU64,
) -> io::Result<()> {
    let mut remaining = reader.take(length);
    let sent = tokio::time::timeout(
        Duration::from_secs(300),
        tokio::io::copy(&mut remaining, &mut writer),
    )
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "HTTP body timed out"))??;
    if sent != length {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Incomplete HTTP body",
        ));
    }
    counter.fetch_add(sent, Ordering::Relaxed);
    writer.shutdown().await
}
async fn copy(
    mut reader: impl AsyncRead + Unpin,
    mut writer: impl AsyncWrite + Unpin,
    counter: &AtomicU64,
) -> io::Result<()> {
    let mut buffer = [0; 16_384];
    loop {
        let count = tokio::time::timeout(Duration::from_secs(300), reader.read(&mut buffer))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Idle flow"))??;
        if count == 0 {
            return writer.shutdown().await;
        }
        tokio::time::timeout(Duration::from_secs(300), writer.write_all(&buffer[..count]))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Stalled flow"))??;
        counter.fetch_add(count as u64, Ordering::Relaxed);
    }
}
async fn socks(
    client: &mut TcpStream,
    table: &RoutingTable,
    dns: &DnsConfig,
) -> io::Result<TcpStream> {
    if client.read_u8().await? != 5 {
        return Err(invalid());
    }
    let count = client.read_u8().await? as usize;
    let mut methods = vec![0; count];
    client.read_exact(&mut methods).await?;
    if !methods.contains(&0) {
        client.write_all(&[5, 255]).await?;
        return Err(invalid());
    }
    client.write_all(&[5, 0]).await?;
    let mut header = [0; 4];
    client.read_exact(&mut header).await?;
    if header[0] != 5 || header[2] != 0 {
        return Err(invalid());
    }
    let host = read_address(client, header[3]).await?;
    let port = client.read_u16().await?;
    if header[1] != 1 {
        // Explicitly reject BIND and UDP ASSOCIATE; there is no fake UDP path.
        client.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
        return Err(invalid());
    }
    let destination = Destination::new(host, port)?;
    let result = connect(table, dns, &destination).await;
    let code = match &result {
        Ok(_) => 0,
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => 2,
        Err(_) => 5,
    };
    client.write_all(&[5, code, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
    result
}
async fn http(
    client: &mut TcpStream,
    table: &RoutingTable,
    dns: &DnsConfig,
    counters: &Counters,
) -> io::Result<Connected> {
    let headers = read_headers(client).await?;
    let mut lines = headers.split("\r\n");
    let mut fields = lines.next().ok_or_else(invalid)?.split_whitespace();
    let request = [
        fields.next().ok_or_else(invalid)?,
        fields.next().ok_or_else(invalid)?,
        fields.next().ok_or_else(invalid)?,
    ];
    if fields.next().is_some()
        || !matches!(request[2], "HTTP/1.0" | "HTTP/1.1")
        || !request[0].bytes().all(|b| b.is_ascii_uppercase())
    {
        return Err(invalid());
    }
    let tunnel = request[0] == "CONNECT";
    let url = url::Url::parse(&if tunnel {
        format!("http://{}", request[1])
    } else {
        request[1].into()
    })
    .map_err(|_| invalid())?;
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    if tunnel && (url.path() != "/" || url.query().is_some() || !request[1].contains(':')) {
        return Err(invalid());
    }
    let destination = Destination::new(
        url.host_str().ok_or_else(invalid)?.into(),
        url.port_or_known_default().ok_or_else(invalid)?,
    )?;
    let mut forward_body = None;
    let mut forward_head = String::new();
    if !tunnel {
        let mut content_length = None;
        for line in lines.filter(|line| !line.is_empty()) {
            let (name, value) = line.split_once(':').ok_or_else(invalid)?;
            if name.is_empty()
                || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                || value.chars().any(|c| c.is_control() && c != '\t')
            {
                return Err(invalid());
            }
            match name {
                name if name.eq_ignore_ascii_case("content-length") => {
                    if content_length.is_some() {
                        return Err(invalid());
                    }
                    content_length = Some(value.trim().parse::<u64>().map_err(|_| invalid())?);
                    forward_head.push_str(line);
                    forward_head.push_str("\r\n");
                }
                name if [
                    "host",
                    "proxy-connection",
                    "proxy-authorization",
                    "proxy-authenticate",
                    "keep-alive",
                ]
                .iter()
                .any(|header| name.eq_ignore_ascii_case(header)) => {}
                name if name.eq_ignore_ascii_case("connection") => {
                    if !value.split(',').all(|token| {
                        let token = token.trim();
                        token.eq_ignore_ascii_case("close")
                            || token.eq_ignore_ascii_case("keep-alive")
                    }) {
                        return Err(invalid());
                    }
                }
                name if ["transfer-encoding", "te", "trailer", "upgrade", "expect"]
                    .iter()
                    .any(|header| name.eq_ignore_ascii_case(header)) =>
                {
                    return Err(invalid())
                }
                _ => {
                    forward_head.push_str(line);
                    forward_head.push_str("\r\n");
                }
            }
        }
        forward_body = Some(content_length.unwrap_or(0));
    }
    let mut upstream = match connect(table, dns, &destination).await {
        Ok(s) => s,
        Err(error) => {
            client
                .write_all(
                    b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
                )
                .await?;
            return Err(error);
        }
    };
    if tunnel {
        client
            .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .await?;
    } else {
        let mut head = format!(
            "{} {}{} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
            request[0],
            url.path(),
            url.query().map(|q| format!("?{q}")).unwrap_or_default(),
            destination.authority()
        );
        head.push_str(&forward_head);
        head.push_str("\r\n");
        upstream.write_all(head.as_bytes()).await?;
        counters
            .upload
            .fetch_add(head.len() as u64, Ordering::Relaxed);
    }
    Ok(Connected {
        upstream,
        forward_body,
    })
}
