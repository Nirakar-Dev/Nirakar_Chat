mod identity;
mod network;
#[tauri::command]
fn add_contact(
    tx: tauri::State<'_, tokio::sync::mpsc::Sender<network::Command>>,
    nirakar_id: String,
    multiaddr_str: Option<String>,
) -> Result<(), String> {
    let tx_clone = tx.inner().clone();
    tauri::async_runtime::spawn(async move {
        let _ = tx_clone.send(network::Command::AddContact {
            nirakar_id,
            multiaddr_str,
        }).await;
    });
    Ok(())
}

#[tauri::command]
fn send_message(
    tx: tauri::State<'_, tokio::sync::mpsc::Sender<network::Command>>,
    peer_id: String,
    content: String,
) -> Result<(), String> {
    let tx_clone = tx.inner().clone();
    tauri::async_runtime::spawn(async move {
        let _ = tx_clone.send(network::Command::SendMessage {
            peer_id,
            content,
        }).await;
    });
    Ok(())
}

#[tauri::command]
fn publish_identity(
    tx: tauri::State<'_, tokio::sync::mpsc::Sender<network::Command>>,
    nirakar_id: String,
) -> Result<(), String> {
    let tx_clone = tx.inner().clone();
    tauri::async_runtime::spawn(async move {
        let _ = tx_clone.send(network::Command::PublishIdentity {
            nirakar_id,
        }).await;
    });
    Ok(())
}

#[tauri::command]
fn send_file(
    tx: tauri::State<'_, tokio::sync::mpsc::Sender<network::Command>>,
    peer_id: String,
    file_path: String,
) -> Result<(), String> {
    let tx_clone = tx.inner().clone();
    tauri::async_runtime::spawn(async move {
        let _ = tx_clone.send(network::Command::SendFile {
            peer_id,
            file_path,
        }).await;
    });
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    tauri::Builder::default()
        .setup(move |app| {
            use tauri::Manager;
            let identity = identity::init_identity(app.handle()).unwrap();
            app.manage(tx.clone());
            network::start_network(app.handle().clone(), identity.private_key.clone(), rx).unwrap();
            
            // Set up native system tray
            if let Some(icon) = app.default_window_icon() {
                let _ = tauri::tray::TrayIconBuilder::with_id("main")
                    .icon(icon.clone())
                    .tooltip("Nirakar Chat")
                    .build(app);
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            identity::get_identity,
            identity::update_name,
            identity::fetch_messages,
            identity::fetch_contacts,
            identity::delete_contact,
            identity::rename_contact,
            add_contact,
            send_message,
            send_file,
            publish_identity
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
