use ed25519_dalek::{SigningKey, VerifyingKey};

use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::fs;
use tauri::{AppHandle, Manager};

#[derive(Debug, serde::Serialize, Clone)]
pub struct Identity {
    pub nirakar_id: String,
    pub public_key: String,
    pub name: String,
    #[serde(skip)]
    pub private_key: Vec<u8>,
}

pub fn init_identity(app_handle: &AppHandle) -> Result<Identity, String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;

    fs::create_dir_all(&app_data_dir).map_err(|e| e.to_string())?;

    let db_path = app_data_dir.join("nirakar.db");

    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS identity (
            id INTEGER PRIMARY KEY,
            nirakar_id TEXT NOT NULL,
            private_key BLOB NOT NULL,
            public_key BLOB NOT NULL,
            name TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            peer_id TEXT NOT NULL,
            is_outgoing BOOLEAN NOT NULL,
            content TEXT NOT NULL,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS contacts (
            id TEXT PRIMARY KEY,
            peer_id TEXT NOT NULL,
            name TEXT NOT NULL,
            online BOOLEAN DEFAULT 0
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT nirakar_id, public_key, name, private_key FROM identity LIMIT 1")
        .map_err(|e| e.to_string())?;
        
    let existing_identity = stmt.query_row([], |row| {
        let pk_blob: Vec<u8> = row.get(1)?;
        let hex_pk = hex::encode(pk_blob);
        Ok(Identity {
            nirakar_id: row.get(0)?,
            public_key: hex_pk,
            name: row.get(2)?,
            private_key: row.get(3)?,
        })
    });

    match existing_identity {
        Ok(identity) => Ok(identity),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let mut bytes = [0u8; 32];
            rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
            let signing_key: SigningKey = SigningKey::from_bytes(&bytes);
            let verifying_key: VerifyingKey = (&signing_key).into();

            let pk_bytes = verifying_key.as_bytes();

            let mut hasher = Sha256::new();
            hasher.update(pk_bytes);
            let result = hasher.finalize();
            let hex_hash = hex::encode(result);
            let short_hash = &hex_hash[0..8].to_uppercase();
            let nirakar_id = format!("NK-{}", short_hash);
            let name = "Nirakar User".to_string();

            conn.execute(
                "INSERT INTO identity (nirakar_id, private_key, public_key, name) VALUES (?1, ?2, ?3, ?4)",
                (
                    &nirakar_id,
                    signing_key.to_bytes().as_ref(),
                    pk_bytes.as_ref(),
                    &name,
                ),
            )
            .map_err(|e| e.to_string())?;

            Ok(Identity {
                nirakar_id,
                public_key: hex::encode(pk_bytes),
                name,
                private_key: signing_key.to_bytes().to_vec(),
            })
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub fn get_identity(app: AppHandle) -> Result<Identity, String> {
    init_identity(&app)
}

#[tauri::command]
pub fn update_name(app: AppHandle, new_name: String) -> Result<Identity, String> {
    let mut identity = init_identity(&app)?;
    
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_data_dir.join("nirakar.db");
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
    
    conn.execute(
        "UPDATE identity SET name = ?1 WHERE nirakar_id = ?2",
        (&new_name, &identity.nirakar_id),
    ).map_err(|e| e.to_string())?;
    
    identity.name = new_name;
    Ok(identity)
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct ChatMessage {
    pub id: i64,
    pub peer_id: String,
    pub is_outgoing: bool,
    pub content: String,
    pub timestamp: String,
}

#[tauri::command]
pub fn fetch_messages(app: AppHandle, peer_id: String) -> Result<Vec<ChatMessage>, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_data_dir.join("nirakar.db");
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare("SELECT id, peer_id, is_outgoing, content, timestamp FROM messages WHERE peer_id = ?1 ORDER BY timestamp ASC").map_err(|e| e.to_string())?;
    
    let msg_iter = stmt.query_map([&peer_id], |row| {
        Ok(ChatMessage {
            id: row.get(0)?,
            peer_id: row.get(1)?,
            is_outgoing: row.get(2)?,
            content: row.get(3)?,
            timestamp: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut messages = Vec::new();
    for msg in msg_iter {
        messages.push(msg.map_err(|e| e.to_string())?);
    }
    
    Ok(messages)
}

pub fn save_message(app: &AppHandle, peer_id: &str, is_outgoing: bool, content: &str) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_data_dir.join("nirakar.db");
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO messages (peer_id, is_outgoing, content) VALUES (?1, ?2, ?3)",
        rusqlite::params![peer_id, is_outgoing, content],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct Contact {
    pub id: String, // NK-ID
    pub peer_id: String,
    pub name: String,
    pub online: bool,
}

#[tauri::command]
pub fn fetch_contacts(app: AppHandle) -> Result<Vec<Contact>, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_data_dir.join("nirakar.db");
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare("SELECT id, peer_id, name, online FROM contacts").map_err(|e| e.to_string())?;
    
    let contact_iter = stmt.query_map([], |row| {
        Ok(Contact {
            id: row.get(0)?,
            peer_id: row.get(1)?,
            name: row.get(2)?,
            online: row.get(3)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut contacts = Vec::new();
    for contact in contact_iter {
        contacts.push(contact.map_err(|e| e.to_string())?);
    }
    
    Ok(contacts)
}

pub fn save_contact(app: &AppHandle, id: &str, peer_id: &str, name: &str, online: bool) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_data_dir.join("nirakar.db");
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO contacts (id, peer_id, name, online) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET peer_id = ?2, name = ?3, online = ?4",
        rusqlite::params![id, peer_id, name, online],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn delete_contact(app: AppHandle, contact_id: String) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_data_dir.join("nirakar.db");
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM contacts WHERE id = ?1", [&contact_id])
        .map_err(|e| e.to_string())?;
    // Also delete chat history for this contact
    conn.execute("DELETE FROM messages WHERE peer_id = ?1", [&contact_id])
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn rename_contact(app: AppHandle, contact_id: String, new_name: String) -> Result<(), String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_path = app_data_dir.join("nirakar.db");
    let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE contacts SET name = ?1 WHERE id = ?2",
        rusqlite::params![new_name, contact_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}
