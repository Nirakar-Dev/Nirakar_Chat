use libp2p::{
    identity::Keypair,
    kad::{store::MemoryStore, Behaviour as Kademlia, Config as KademliaConfig, Event as KademliaEvent},
    noise,
    request_response::{self, ProtocolSupport},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, SwarmBuilder, StreamProtocol,
    mdns
};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use futures::StreamExt;
use libp2p::kad::{Record, RecordKey};
use libp2p_stream as stream;
use tauri_plugin_dialog::DialogExt;
use std::error::Error;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct ChatMessageReq {
    pub content: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct ChatMessageRes {
    pub delivered: bool,
}

#[derive(NetworkBehaviour)]
pub struct NirakarBehaviour {
    pub kademlia: Kademlia<MemoryStore>,
    pub request_response: request_response::cbor::Behaviour<ChatMessageReq, ChatMessageRes>,
    pub stream: stream::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct NetworkStatus {
    pub status: String,
    pub peer_id: String,
    pub message: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct DhtRecordValue {
    pub peer_id: String,
}

pub enum Command {
    AddContact { nirakar_id: String, multiaddr_str: Option<String> },
    SendMessage { peer_id: String, content: String },
    SendFile { peer_id: String, file_path: String },
    PublishIdentity { nirakar_id: String },
}

pub fn start_network(
    app: AppHandle,
    private_key_bytes: Vec<u8>,
    mut command_receiver: mpsc::Receiver<Command>,
) -> Result<(), Box<dyn Error>> {
    let mut key_bytes = private_key_bytes.clone();
    let id_keys = Keypair::ed25519_from_bytes(&mut key_bytes).expect("Valid private key");
    let local_peer_id = id_keys.public().to_peer_id();

    tauri::async_runtime::spawn(async move {
        let mut swarm = SwarmBuilder::with_existing_identity(id_keys)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        ).unwrap()
        .with_quic()
        .with_behaviour(|key| {
            let store = MemoryStore::new(key.public().to_peer_id());
            let cfg = KademliaConfig::default();
            
            let req_res = request_response::cbor::Behaviour::new(
                [(StreamProtocol::new("/nirakar/chat/1.0.0"), ProtocolSupport::Full)],
                request_response::Config::default(),
            );
            
            let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id()).unwrap();
            
            NirakarBehaviour {
                kademlia: Kademlia::with_config(key.public().to_peer_id(), store, cfg),
                request_response: req_res,
                stream: stream::Behaviour::new(),
                mdns,
            }
        }).unwrap()
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

        // Listen on random port
        if let Err(e) = swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap()) {
            eprintln!("Failed to listen on tcp: {}", e);
        }
        if let Err(e) = swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse().unwrap()) {
            eprintln!("Failed to listen on quic: {}", e);
        }

        app.emit("network_status", NetworkStatus {
            status: "Connecting".to_string(),
            peer_id: local_peer_id.to_string(),
            message: "Starting libp2p swarm...".to_string(),
        }).ok();

        loop {
            tokio::select! {
                Some(cmd) = command_receiver.recv() => {
                    match cmd {
                        Command::AddContact { nirakar_id, multiaddr_str } => {
                            if let Some(maddr_str) = multiaddr_str {
                                if let Ok(maddr) = maddr_str.parse::<Multiaddr>() {
                                    if let Err(e) = swarm.dial(maddr.clone()) {
                                        app.emit("network_status", NetworkStatus {
                                            status: "Error".to_string(),
                                            peer_id: local_peer_id.to_string(),
                                            message: format!("Failed to dial: {}", e),
                                        }).ok();
                                    } else {
                                        app.emit("network_status", NetworkStatus {
                                            status: "Connecting".to_string(),
                                            peer_id: local_peer_id.to_string(),
                                            message: format!("Dialing {}", maddr),
                                        }).ok();
                                    }
                                }
                            } else {
                                let key = RecordKey::new(&nirakar_id.as_bytes());
                                swarm.behaviour_mut().kademlia.get_record(key);
                                app.emit("network_status", NetworkStatus { 
                                    status: "Searching DHT".into(), 
                                    peer_id: "".into(), 
                                    message: format!("Looking up {}...", nirakar_id) 
                                }).ok();
                            }
                        }
                        Command::PublishIdentity { nirakar_id } => {
                            let key = RecordKey::new(&nirakar_id.as_bytes());
                            let value = serde_json::to_vec(&DhtRecordValue {
                                peer_id: swarm.local_peer_id().to_string(),
                            }).unwrap();
                            let record = Record {
                                key,
                                value,
                                publisher: None,
                                expires: None,
                            };
                            let _ = swarm.behaviour_mut().kademlia.put_record(record, libp2p::kad::Quorum::One);
                        }
                        Command::SendMessage { peer_id, content } => {
                            if let Ok(peer) = peer_id.parse::<PeerId>() {
                                let req = ChatMessageReq { content: content.clone() };
                                swarm.behaviour_mut().request_response.send_request(&peer, req);
                                
                                // Save outgoing message
                                crate::identity::save_message(&app, &peer_id, true, &content).ok();
                                app.emit("chat_update", peer_id).ok();
                            }
                        }
                        Command::SendFile { peer_id, file_path } => {
                            if let Ok(peer) = peer_id.parse::<PeerId>() {
                                // Background task to open stream and send file
                                let mut stream_ctrl = swarm.behaviour().stream.new_control();
                                let app_clone = app.clone();
                                tokio::spawn(async move {
                                    if let Ok(stream) = stream_ctrl.open_stream(peer, libp2p::StreamProtocol::new("/nirakar/file/1.0.0")).await {
                                        use tokio::io::AsyncWriteExt;
                                        use tokio_util::compat::FuturesAsyncWriteCompatExt;
                                        let mut stream = stream.compat_write();
                                        
                                        // Read file name
                                        let file_name = std::path::Path::new(&file_path)
                                            .file_name()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .into_owned();
                                            
                                        // Send file name first (simple length-prefixed)
                                        let name_bytes = file_name.as_bytes();
                                        let _ = stream.write_u32(name_bytes.len() as u32).await;
                                        let _ = stream.write_all(name_bytes).await;
                                        
                                        // Stream file chunks
                                        if let Ok(mut file) = tokio::fs::File::open(&file_path).await {
                                            let _ = tokio::io::copy(&mut file, &mut stream).await;
                                            let _ = stream.flush().await;
                                            
                                            // Save message about sent file
                                            let content = format!("[File Sent: {}]", file_name);
                                            crate::identity::save_message(&app_clone, &peer.to_string(), true, &content).ok();
                                            app_clone.emit("chat_update", peer.to_string()).ok();
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
                event = swarm.select_next_some() => match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        app.emit("network_status", NetworkStatus {
                            status: "Connected".to_string(),
                            peer_id: local_peer_id.to_string(),
                            message: format!("Listening on {}", address),
                        }).ok();
                        
                        // Accept incoming files
                        let mut stream_ctrl = swarm.behaviour().stream.new_control();
                        let app_clone = app.clone();
                        tokio::spawn(async move {
                            if let Ok(mut incoming) = stream_ctrl.accept(libp2p::StreamProtocol::new("/nirakar/file/1.0.0")) {
                                while let Some((peer, stream)) = incoming.next().await {
                                    let app_c = app_clone.clone();
                                    tokio::spawn(async move {
                                        use tokio::io::AsyncReadExt;
                                        use tokio_util::compat::FuturesAsyncReadCompatExt;
                                        let mut stream = stream.compat();
                                        
                                        // Read file name
                                        let name_len = stream.read_u32().await.unwrap_or(0);
                                        if name_len > 0 && name_len < 1024 {
                                            let mut name_buf = vec![0u8; name_len as usize];
                                            let _ = stream.read_exact(&mut name_buf).await;
                                            let file_name = String::from_utf8_lossy(&name_buf).into_owned();
                                            
                                            app_c.emit("network_status", NetworkStatus {
                                                status: "Transferring".into(),
                                                peer_id: peer.to_string(),
                                                message: format!("Receiving file: {}", file_name),
                                            }).ok();
                                            
                                            let (tx, rx) = tokio::sync::oneshot::channel();
                                            let app_dialog = app_c.clone();
                                            
                                            // Dialogs must be invoked on the main thread / asynchronously 
                                            app_dialog.dialog().file().set_file_name(&file_name).save_file(move |file_path| {
                                                let _ = tx.send(file_path);
                                            });

                                            if let Ok(Some(file_path)) = rx.await {
                                                if let Ok(path_buf) = file_path.into_path() {
                                                    if let Ok(mut file) = tokio::fs::File::create(&path_buf).await {
                                                        let _ = tokio::io::copy(&mut stream, &mut file).await;
                                                        
                                                        // Save message about received file
                                                        let content = format!("[File Received: {}]", file_name);
                                                        crate::identity::save_message(&app_c, &peer.to_string(), false, &content).ok();
                                                        app_c.emit("chat_update", peer.to_string()).ok();
                                                        
                                                        if let Some(tray) = app_c.tray_by_id("main") {
                                                            let _ = tray.set_tooltip(Some("Nirakar Chat (Unread!)"));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    });
                                }
                            }
                        });
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        app.emit("network_status", NetworkStatus {
                            status: "Connected".to_string(),
                            peer_id: peer_id.to_string(),
                            message: format!("Connected to {}", endpoint.get_remote_address()),
                        }).ok();
                        
                        // Add to routing table
                        swarm.behaviour_mut().kademlia.add_address(&peer_id, endpoint.get_remote_address().clone());
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        app.emit("network_status", NetworkStatus {
                            status: "Offline".to_string(),
                            peer_id: peer_id.to_string(),
                            message: "Connection closed".to_string(),
                        }).ok();
                    }
                    SwarmEvent::Behaviour(NirakarBehaviourEvent::Kademlia(KademliaEvent::OutboundQueryProgressed { result, .. })) => {
                        match result {
                            libp2p::kad::QueryResult::GetRecord(Ok(libp2p::kad::GetRecordOk::FoundRecord(record))) => {
                                if let Ok(val) = serde_json::from_slice::<DhtRecordValue>(&record.record.value) {
                                    let nk_id = String::from_utf8_lossy(&record.record.key.to_vec()).to_string();
                                    
                                    // We found them! Save to SQLite contacts
                                    crate::identity::save_contact(&app, &nk_id, &val.peer_id, "Unknown Contact", true).ok();
                                    
                                    // Connect to them if possible, Kademlia might already have their addresses
                                    if let Ok(peer_id) = val.peer_id.parse::<PeerId>() {
                                        let _ = swarm.dial(peer_id);
                                    }

                                    app.emit("network_status", NetworkStatus { 
                                        status: "Found Peer".into(), 
                                        peer_id: val.peer_id, 
                                        message: format!("Found {} in DHT", nk_id) 
                                    }).ok();
                                }
                            }
                            libp2p::kad::QueryResult::GetRecord(Err(e)) => {
                                app.emit("network_status", NetworkStatus { 
                                    status: "DHT Error".into(), 
                                    peer_id: "".into(), 
                                    message: format!("Lookup failed: {:?}", e) 
                                }).ok();
                            }
                            _ => {}
                        }
                    }
                    SwarmEvent::Behaviour(NirakarBehaviourEvent::RequestResponse(request_response::Event::Message { peer, message, .. })) => {
                        match message {
                            request_response::Message::Request { request, channel, .. } => {
                                // Save incoming message
                                let content = request.content;
                                crate::identity::save_message(&app, &peer.to_string(), false, &content).ok();
                                app.emit("chat_update", peer.to_string()).ok();
                                
                                if let Some(tray) = app.tray_by_id("main") {
                                    let _ = tray.set_tooltip(Some("Nirakar Chat (Unread!)"));
                                }
                                
                                // Send delivery confirmation
                                let _ = swarm.behaviour_mut().request_response.send_response(
                                    channel,
                                    ChatMessageRes { delivered: true }
                                );
                            }
                            request_response::Message::Response { .. } => {
                                // Handled delivery
                            }
                        }
                    }
                    SwarmEvent::Behaviour(NirakarBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer_id, multiaddr) in list {
                            swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr);
                            let _ = swarm.dial(peer_id);
                            app.emit("network_status", NetworkStatus {
                                status: "Discovered".to_string(),
                                peer_id: peer_id.to_string(),
                                message: "Discovered peer on local network".to_string(),
                            }).ok();
                        }
                    }
                    SwarmEvent::Behaviour(NirakarBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                        for (peer_id, multiaddr) in list {
                            swarm.behaviour_mut().kademlia.remove_address(&peer_id, &multiaddr);
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    Ok(())
}
