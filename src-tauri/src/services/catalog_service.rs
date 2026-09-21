use crate::{
    app::state::AppState,
    domain::errors::{BackendError, Result},
    storage::repository::{Repository, Rule, Subscription},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn snapshot(
    repo: &Repository,
    latencies: &std::collections::HashMap<String, Value>,
) -> Value {
    json!({
        "version": 1,
        "subscriptions": repo.subscriptions,
        "servers": repo.nodes.iter().map(|n| {
            let cc = country(&n.name);
            let lat = latencies.get(&n.id).cloned().unwrap_or(json!("timeout"));
            json!({
                "id": n.id,
                "subscriptionId": n.subscription_id,
                "countryCode": cc,
                "name": clean_name(&n.name),
                "nameKey": cc,
                "protocol": n.protocol,
                "latency": lat
            })
        }).collect::<Vec<_>>(),
        "rules": repo.rules,
        "selectedServerId": repo.selected_server_id,
        "selectedSubscriptionId": repo.selected_subscription_id,
        "selectedServers": repo.selected_servers,
        "routingMode": repo.routing_mode
    })
}
fn clean_name(name: &str) -> String {
    let s = name.trim();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        if i + 1 < chars.len()
            && ('\u{1f1e6}'..='\u{1f1ff}').contains(&chars[i])
            && ('\u{1f1e6}'..='\u{1f1ff}').contains(&chars[i + 1])
        {
            i += 2;
            continue;
        }
        if chars[i] == '\u{1f3f3}' || chars[i] == '\u{fe0f}' {
            i += 1;
            continue;
        }
        break;
    }
    let res: String = chars[i..].iter().collect();
    let trimmed = res.trim();
    if trimmed.starts_with("Белые Списки / ") {
        let without_flags: String = trimmed.chars().filter(|c| !('\u{1f1e6}'..='\u{1f1ff}').contains(c)).collect();
        without_flags.trim().to_string()
    } else {
        trimmed.to_string()
    }
}
fn country(name: &str) -> String {
    let chars: Vec<_> = name.chars().collect();
    // Look for the LAST pair of Regional Indicator Symbols (country flag emoji).
    // Some names start with 🏳️ (white flag) before the real country flag,
    // or contain multiple flags — prefer the last actual country flag found.
    let mut result = String::new();
    for pair in chars.windows(2) {
        if pair.iter().all(|c| ('\u{1f1e6}'..='\u{1f1ff}').contains(c)) {
            result = pair
                .iter()
                .filter_map(|c| char::from_u32(*c as u32 - 0x1f1e6 + 'a' as u32))
                .collect();
        }
    }
    result
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Mutation {
    SelectServer { id: String },
    SelectSubscription { id: String },
    RenameSubscription { id: String, name: String },
    RemoveSubscription { id: String },
    SetRoutingMode { mode: String },
    SaveRules { rules: Vec<Rule> },
    RemoveRules { ids: Vec<String> },
}
pub async fn mutate(state: &AppState, mutation: Mutation) -> Result<Value> {
    let _guard = state.mutations.lock().await;
    let mut next = state.repository.read().await.clone();
    match mutation {
        Mutation::SelectServer { id } => {
            let node = next
                .nodes
                .iter()
                .find(|n| n.id == id)
                .ok_or_else(|| BackendError::new("INVALID_SERVER", "Unknown server"))?;
            next.selected_subscription_id = Some(node.subscription_id.clone());
            next.selected_server_id = Some(id.clone());
            next.selected_servers.insert(node.subscription_id.clone(), id);
        }
        Mutation::SelectSubscription { id } => {
            if !next.subscriptions.iter().any(|s| s.id == id) {
                return Err(BackendError::invalid());
            }
            next.selected_subscription_id = Some(id.clone());
            let remembered = next.selected_servers.get(&id).and_then(|sid| {
                if next.nodes.iter().any(|n| n.subscription_id == id && n.id == *sid) {
                    Some(sid.clone())
                } else {
                    None
                }
            });
            if let Some(sid) = remembered {
                next.selected_server_id = Some(sid);
            } else if let Some(node) = next.nodes.iter().find(|n| n.subscription_id == id) {
                next.selected_server_id = Some(node.id.clone());
                next.selected_servers.insert(id, node.id.clone());
            } else {
                next.selected_server_id = None;
            }
        }
        Mutation::RenameSubscription { id, name } => {
            if name.trim().is_empty() || name.len() > 320 {
                return Err(BackendError::invalid());
            }
            next.subscriptions
                .iter_mut()
                .find(|s| s.id == id)
                .ok_or_else(BackendError::invalid)?
                .name = name.trim().into();
        }
        Mutation::RemoveSubscription { id } => {
            next.subscriptions.retain(|s| s.id != id);
            next.nodes.retain(|n| n.subscription_id != id);
            next.sources.remove(&id);
            next.selected_servers.remove(&id);
            next.rules.retain(|r| {
                r.server_id == "direct"
                    || r.server_id == "subscription"
                    || next
                        .nodes
                        .iter()
                        .any(|n| n.id == r.server_id || n.subscription_id == r.server_id)
            });
            reconcile(&mut next);
        }
        Mutation::SetRoutingMode { mode } => {
            if !["global", "rules"].contains(&mode.as_str()) {
                return Err(BackendError::new("INVALID_RULE", "Unknown routing mode"));
            }
            next.routing_mode = mode;
        }
        Mutation::SaveRules { rules } => {
            if rules.len() > 4096 {
                return Err(BackendError::new("INVALID_RULE", "Too many routing rules"));
            }
            for rule in rules {
                if rule.id.is_empty() || rule.name.trim().is_empty() || rule.name.len() > 1024 {
                    return Err(BackendError::new("INVALID_RULE", "Invalid rule"));
                }
                if let Some(existing) = next.rules.iter_mut().find(|r| r.id == rule.id) {
                    *existing = rule;
                } else {
                    next.rules.push(rule);
                }
            }
        }
        Mutation::RemoveRules { ids } => next.rules.retain(|r| !ids.contains(&r.id)),
    }
    if let Some(id) = &next.selected_server_id {
        crate::xray::config::build(&next, id, 10808, 10809, false)?;
    }
    state.store.save(&next)?;
    let latencies = state.latencies.read().await;
    let result = snapshot(&next, &latencies);
    *state.repository.write().await = next;
    Ok(result)
}
fn reconcile(repo: &mut Repository) {
    let sub_ids: std::collections::HashSet<_> = repo.subscriptions.iter().map(|s| s.id.clone()).collect();
    repo.selected_servers.retain(|sub_id, server_id| {
        sub_ids.contains(sub_id) && repo.nodes.iter().any(|n| n.subscription_id == *sub_id && n.id == *server_id)
    });

    if !repo
        .nodes
        .iter()
        .any(|n| Some(&n.id) == repo.selected_server_id.as_ref())
    {
        let fallback = repo
            .selected_subscription_id
            .as_ref()
            .and_then(|sid| repo.selected_servers.get(sid).cloned())
            .or_else(|| repo.nodes.first().map(|n| n.id.clone()));
        repo.selected_server_id = fallback;
    }
    repo.selected_subscription_id = repo
        .nodes
        .iter()
        .find(|n| Some(&n.id) == repo.selected_server_id.as_ref())
        .map(|n| n.subscription_id.clone());

    if let (Some(sub_id), Some(srv_id)) = (&repo.selected_subscription_id, &repo.selected_server_id) {
        repo.selected_servers.insert(sub_id.clone(), srv_id.clone());
    }
}
#[derive(Serialize)]
pub struct ImportResult {
    pub catalog: Value,
    pub diagnostics: Vec<crate::subscription::parser::ImportDiagnostic>,
}
pub async fn import(
    state: &AppState,
    input: String,
    name: String,
    refresh_id: Option<String>,
) -> Result<ImportResult> {
    if name.trim().is_empty() || name.len() > 320 {
        return Err(BackendError::invalid());
    }
    let body = crate::subscription::fetcher::fetch(&input).await?;
    let id = refresh_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let parsed = crate::subscription::parser::parse(&body, &id)?;
    let _guard = state.mutations.lock().await;
    let mut next = state.repository.read().await.clone();
    if refresh_id.is_some() && !next.sources.contains_key(&id) {
        return Err(BackendError::invalid());
    }
    next.nodes.retain(|n| n.subscription_id != id);
    next.nodes.extend(parsed.nodes);
    if let Some(sub) = next.subscriptions.iter_mut().find(|s| s.id == id) {
        sub.updated_at = now();
    } else {
        next.subscriptions.push(Subscription {
            id: id.clone(),
            name: name.trim().into(),
            updated_at: now(),
        });
    }
    next.sources.insert(id.clone(), input);
    if refresh_id.is_none() {
        next.selected_server_id = next
            .nodes
            .iter()
            .find(|n| n.subscription_id == id)
            .map(|n| n.id.clone());
    }
    reconcile(&mut next);
    state.store.save(&next)?;
    let latencies = state.latencies.read().await;
    let catalog = snapshot(&next, &latencies);
    *state.repository.write().await = next;
    Ok(ImportResult {
        catalog,
        diagnostics: parsed.diagnostics,
    })
}
