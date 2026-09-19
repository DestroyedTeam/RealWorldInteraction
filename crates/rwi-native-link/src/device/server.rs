//! Bounded authenticated connections, with opt-in sequential framed keepalive.
use super::*;

pub async fn serve<H, F>(
    listener: TcpListener,
    tls: Arc<rustls::ServerConfig>,
    handler: H,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> io::Result<()>
where
    H: Fn(PeerPin, Request) -> F + Send + Sync + 'static,
    F: std::future::Future<Output = Reply> + Send + 'static,
{
    let mut config = (*tls).clone();
    config.alpn_protocols = vec![KEEPALIVE_ALPN.to_vec()];
    let tls = Arc::new(config);
    let handler = Arc::new(handler);
    let limit = Arc::new(tokio::sync::Semaphore::new(32));
    let mut tasks = tokio::task::JoinSet::new();
    let mut shutdown = shutdown;
    loop {
        tokio::select! {
            _ = shutdown.changed() => { break; },
            Some(_) = tasks.join_next(), if !tasks.is_empty() => {},
            incoming = listener.accept() => {
                let (tcp, _) = incoming?;
                let Ok(permit) = limit.clone().try_acquire_owned() else { drop(tcp); continue; };
                let tls = tls.clone(); let handler = handler.clone();
                tasks.spawn(async move {
                    let _permit = permit;
                    tcp.set_nodelay(true)?;
                    let (peer, request, mut stream, persistent) = tokio::time::timeout(CONNECT_TIMEOUT, async {
                        let stream = TlsAcceptor::from(tls).accept(tcp).await?;
                        let peer = rwi_link_trust::peer_sha(stream.get_ref().1.peer_certificates())?;
                        let persistent = stream.get_ref().1.alpn_protocol() == Some(KEEPALIVE_ALPN);
                        let mut stream = BufReader::new(stream);
                        let raw = crate::wire::read_frame(&mut stream, REQUEST_CAP).await?.ok_or_else(|| invalid("empty_request"))?;
                        Ok::<_, io::Error>((peer, raw, stream, persistent))
                    }).await.map_err(|_| invalid("handshake_deadline"))??;
                    let mut request = request;
                    loop {
                        let reply = match decode_request(&request)? {
                            Ok(request) => handler(peer.clone(), request).await,
                            Err(rejection) => rejection,
                        };
                        tokio::time::timeout(Duration::from_secs(10), write(&mut stream, &reply, RESPONSE_CAP)).await.map_err(|_| invalid("reply_timeout"))??;
                        if !persistent { break; }
                        match tokio::time::timeout(SERVER_IDLE_TIMEOUT, crate::wire::read_frame(&mut stream, REQUEST_CAP)).await {
                            Ok(Ok(Some(raw))) => request = raw,
                            Ok(Ok(None)) | Err(_) => break,
                            Ok(Err(error)) => return Err(error),
                        }
                    }
                    let _ = stream.shutdown().await;
                    Ok::<_, io::Error>(())
                });
            }
        }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    Ok(())
}
