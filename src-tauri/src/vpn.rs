use serde::Serialize;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    server_id: String,
    ip: String,
    protocol: String,
    connected_at: u64,
    demo: bool,
}
#[derive(Default)]
pub struct VpnState(pub Mutex<Option<ConnectionInfo>>);
#[derive(Serialize, Debug)]
pub struct AppError {
    code: &'static str,
}
fn connect_state(state: &VpnState, server_id: String) -> Result<(), AppError> {
    if server_id.is_empty()
        || server_id.len() > 128
        || !server_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(AppError {
            code: "invalidServer",
        });
    }
    let mut current = state.0.lock().map_err(|_| AppError {
        code: "unavailable",
    })?;
    if current.is_some() {
        return Err(AppError {
            code: "unavailable",
        });
    }
    let connected_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError {
            code: "unavailable",
        })?
        .as_millis() as u64;
    *current = Some(ConnectionInfo {
        server_id,
        ip: String::new(),
        protocol: "Vless".into(),
        connected_at,
        demo: true,
    });
    Ok(())
}
#[tauri::command]
pub fn connect(state: tauri::State<'_, VpnState>, server_id: String) -> Result<(), AppError> {
    connect_state(&state, server_id)
}
#[tauri::command]
pub fn disconnect(state: tauri::State<'_, VpnState>) -> Result<(), AppError> {
    *state.0.lock().map_err(|_| AppError {
        code: "unavailable",
    })? = None;
    Ok(())
}
#[tauri::command]
pub fn reconnect(state: tauri::State<'_, VpnState>) -> Result<(), AppError> {
    let mut current = state.0.lock().map_err(|_| AppError {
        code: "unavailable",
    })?;
    let info = current.as_mut().ok_or(AppError {
        code: "invalidServer",
    })?;
    info.connected_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError {
            code: "unavailable",
        })?
        .as_millis() as u64;
    Ok(())
}
#[tauri::command]
pub fn get_status(state: tauri::State<'_, VpnState>) -> Result<&'static str, AppError> {
    Ok(
        if state
            .0
            .lock()
            .map_err(|_| AppError {
                code: "unavailable",
            })?
            .is_some()
        {
            "connected"
        } else {
            "disconnected"
        },
    )
}
#[tauri::command]
pub fn get_connection_info(
    state: tauri::State<'_, VpnState>,
) -> Result<Option<ConnectionInfo>, AppError> {
    Ok(state
        .0
        .lock()
        .map_err(|_| AppError {
            code: "unavailable",
        })?
        .clone())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connects_to_requested_server_and_marks_simulation() {
        let state = VpnState::default();
        connect_state(&state, "demo-nl".into()).unwrap();
        let info = state.0.lock().unwrap().clone().unwrap();
        assert_eq!(info.server_id, "demo-nl");
        assert!(info.demo);
        assert!(info.ip.is_empty());
        assert!(info.connected_at > 0);
        assert!(connect_state(&state, "demo-de".into()).is_err());
    }
    #[test]
    fn rejects_invalid_identifiers() {
        for id in ["", "../file", "with space"] {
            assert!(connect_state(&VpnState::default(), id.into()).is_err());
        }
    }
    #[test]
    fn serializes_frontend_contract() {
        let state = VpnState::default();
        connect_state(&state, "demo-fi".into()).unwrap();
        let json = serde_json::to_value(state.0.lock().unwrap().as_ref()).unwrap();
        assert_eq!(json["serverId"], "demo-fi");
        assert!(json["connectedAt"].is_u64());
        assert_eq!(json["demo"], true);
    }
}
