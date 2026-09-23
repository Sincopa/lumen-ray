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
pub fn snapshot(repo: &Repository, latencies: &std::collections::HashMap<String, Value>) -> Value {
    let now_seconds = now() / 1000;
    json!({
        "version": 1,
        "subscriptions": repo.subscriptions.iter().map(|subscription| {
            let status = subscription.expire.filter(|expire| *expire > 0).map(|expire| {
                if expire <= now_seconds { "expired" } else { "active" }
            });
            let days_remaining = subscription.expire.filter(|expire| *expire > 0).map(|expire| {
                expire.saturating_sub(now_seconds) / 86_400
            });
            let traffic = subscription.total.map(|total| {
                if total == 0 {
                    "∞".to_string()
                } else {
                    format_bytes(total.saturating_sub(
                        subscription.upload.unwrap_or(0).saturating_add(
                            subscription.download.unwrap_or(0),
                        ),
                    ))
                }
            });
            json!({
                "id": subscription.id,
                "name": subscription.name,
                "updatedAt": subscription.updated_at,
                "status": status,
                "daysRemaining": days_remaining,
                "traffic": traffic,
            })
        }).collect::<Vec<_>>(),
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

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 7] = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 || value >= 100.0 {
        format!("{value:.0} {}", UNITS[unit])
    } else if value >= 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
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
        let without_flags: String = trimmed
            .chars()
            .filter(|c| !('\u{1f1e6}'..='\u{1f1ff}').contains(c))
            .collect();
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
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Mutation {
    SelectServer { id: String },
    SelectSubscription { id: String },
    RenameSubscription { id: String, name: String },
    RemoveSubscription { id: String },
    SetRoutingMode { mode: String },
    SaveRules { rules: Vec<Rule> },
    RemoveRules { ids: Vec<String> },
    ApplyPreset { preset_id: String },
    RemovePreset { preset_id: String },
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
            next.selected_servers
                .insert(node.subscription_id.clone(), id);
        }
        Mutation::SelectSubscription { id } => {
            if !next.subscriptions.iter().any(|s| s.id == id) {
                return Err(BackendError::invalid());
            }
            next.selected_subscription_id = Some(id.clone());
            let remembered = next.selected_servers.get(&id).and_then(|sid| {
                if next
                    .nodes
                    .iter()
                    .any(|n| n.subscription_id == id && n.id == *sid)
                {
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
            for mut rule in rules {
                if rule.id.is_empty() || rule.name.trim().is_empty() || rule.name.len() > 1024 {
                    return Err(BackendError::new("INVALID_RULE", "Invalid rule"));
                }
                match rule.matcher {
                    crate::storage::repository::RuleMatcher::Geosite => {
                        rule.name =
                            crate::storage::repository::normalize_geo_tag(&rule.name, "geosite")?;
                    }
                    crate::storage::repository::RuleMatcher::Geoip => {
                        rule.name =
                            crate::storage::repository::normalize_geo_tag(&rule.name, "geoip")?;
                    }
                    _ => {}
                }
                rule.is_domain = rule.matcher == crate::storage::repository::RuleMatcher::Domain;
                if let Some(existing) = next.rules.iter_mut().find(|r| r.id == rule.id) {
                    *existing = rule;
                } else {
                    next.rules.push(rule);
                }
            }
        }
        Mutation::RemoveRules { ids } => next.rules.retain(|r| !ids.contains(&r.id)),
        Mutation::ApplyPreset { preset_id } => {
            let target = "subscription";
            if ["russia-optimized", "bypass-blocking"].contains(&preset_id.as_str())
                && next.subscriptions.is_empty()
                && next.nodes.is_empty()
            {
                return Err(BackendError::new(
                    "INVALID_SERVER",
                    "Cannot apply proxy preset without an active server or subscription",
                ));
            }
            let preset_rules = generate_preset_rules(&preset_id, target)?;
            let custom_rules: Vec<Rule> = next
                .rules
                .into_iter()
                .filter(|r| r.preset_id.as_deref() != Some(&preset_id))
                .collect();
            let mut updated_rules = preset_rules;
            updated_rules.extend(custom_rules);
            next.rules = updated_rules;
        }
        Mutation::RemovePreset { preset_id } => {
            next.rules
                .retain(|r| r.preset_id.as_deref() != Some(&preset_id));
        }
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
    let sub_ids: std::collections::HashSet<_> =
        repo.subscriptions.iter().map(|s| s.id.clone()).collect();
    repo.selected_servers.retain(|sub_id, server_id| {
        sub_ids.contains(sub_id)
            && repo
                .nodes
                .iter()
                .any(|n| n.subscription_id == *sub_id && n.id == *server_id)
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

    if let (Some(sub_id), Some(srv_id)) = (&repo.selected_subscription_id, &repo.selected_server_id)
    {
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
    name: Option<String>,
    refresh_id: Option<String>,
) -> Result<ImportResult> {
    if name.as_ref().is_some_and(|name| name.len() > 320) {
        return Err(BackendError::invalid());
    }
    let operation = if refresh_id.is_some() {
        "refresh"
    } else {
        "import"
    };
    let source_kind = if input.trim().starts_with("http://") || input.trim().starts_with("https://")
    {
        "remote"
    } else {
        "inline"
    };
    log::info!("Subscription {operation} started ({source_kind} source)");
    let fetched = state
        .subscription_client
        .fetch_with_metadata(&input)
        .await
        .inspect_err(|error| {
            log::warn!("Subscription {operation} download failed: {}", error.code);
        })?;
    let resolved_name = name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| fetched.suggested_name.clone())
        .or_else(|| {
            url::Url::parse(input.trim())
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .map(|host| host.trim_start_matches("www.").to_owned())
        })
        .unwrap_or_else(|| "Subscription".into());
    let id = refresh_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let parsed = crate::subscription::parser::parse(&fetched.body, &id).inspect_err(|error| {
        log::warn!("Subscription {operation} parsing failed: {}", error.code);
    })?;
    let mut skipped = std::collections::BTreeMap::new();
    for diagnostic in &parsed.diagnostics {
        *skipped.entry(diagnostic.code).or_insert(0usize) += 1;
    }
    log::info!(
        "Subscription {operation} parsed: {} supported nodes, {} skipped ({skipped:?})",
        parsed.nodes.len(),
        parsed.diagnostics.len()
    );
    let _guard = state.mutations.lock().await;
    let mut next = state.repository.read().await.clone();
    if refresh_id.is_some() && !next.sources.contains_key(&id) {
        return Err(BackendError::invalid());
    }
    next.nodes.retain(|n| n.subscription_id != id);
    next.nodes.extend(parsed.nodes);
    if let Some(sub) = next.subscriptions.iter_mut().find(|s| s.id == id) {
        sub.updated_at = now();
        if let Some(info) = fetched.user_info {
            sub.upload = info.upload;
            sub.download = info.download;
            sub.total = info.total;
            sub.expire = info.expire;
        }
    } else {
        let info = fetched.user_info.unwrap_or_default();
        next.subscriptions.push(Subscription {
            id: id.clone(),
            name: resolved_name,
            updated_at: now(),
            upload: info.upload,
            download: info.download,
            total: info.total,
            expire: info.expire,
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

pub fn generate_preset_rules(preset_id: &str, target_proxy: &str) -> Result<Vec<Rule>> {
    use crate::storage::repository::{Rule, RuleMatcher};
    match preset_id {
        "russia-optimized" => Ok(vec![
            Rule {
                id: format!("{preset_id}-geoip-private"),
                name: "private".into(),
                matcher: RuleMatcher::Geoip,
                server_id: "direct".into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
            Rule {
                id: format!("{preset_id}-geosite-antifilter"),
                name: "antifilter".into(),
                matcher: RuleMatcher::Geosite,
                server_id: target_proxy.into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
            Rule {
                id: format!("{preset_id}-geosite-antifilter-community"),
                name: "antifilter-community".into(),
                matcher: RuleMatcher::Geosite,
                server_id: target_proxy.into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
            Rule {
                id: format!("{preset_id}-geoip-antifilter"),
                name: "antifilter".into(),
                matcher: RuleMatcher::Geoip,
                server_id: target_proxy.into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
            Rule {
                id: format!("{preset_id}-geoip-antifilter-community"),
                name: "antifilter-community".into(),
                matcher: RuleMatcher::Geoip,
                server_id: target_proxy.into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
            Rule {
                id: format!("{preset_id}-geosite-category-ru"),
                name: "category-ru".into(),
                matcher: RuleMatcher::Geosite,
                server_id: "direct".into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
            Rule {
                id: format!("{preset_id}-geoip-ru"),
                name: "ru".into(),
                matcher: RuleMatcher::Geoip,
                server_id: "direct".into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
        ]),
        "bypass-blocking" => Ok(vec![
            Rule {
                id: format!("{preset_id}-geosite-antifilter"),
                name: "antifilter".into(),
                matcher: RuleMatcher::Geosite,
                server_id: target_proxy.into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
            Rule {
                id: format!("{preset_id}-geosite-antifilter-community"),
                name: "antifilter-community".into(),
                matcher: RuleMatcher::Geosite,
                server_id: target_proxy.into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
            Rule {
                id: format!("{preset_id}-geoip-antifilter"),
                name: "antifilter".into(),
                matcher: RuleMatcher::Geoip,
                server_id: target_proxy.into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
            Rule {
                id: format!("{preset_id}-geoip-antifilter-community"),
                name: "antifilter-community".into(),
                matcher: RuleMatcher::Geoip,
                server_id: target_proxy.into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
        ]),
        "russia-direct" => Ok(vec![
            Rule {
                id: format!("{preset_id}-geosite-category-ru"),
                name: "category-ru".into(),
                matcher: RuleMatcher::Geosite,
                server_id: "direct".into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
            Rule {
                id: format!("{preset_id}-geoip-ru"),
                name: "ru".into(),
                matcher: RuleMatcher::Geoip,
                server_id: "direct".into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
            Rule {
                id: format!("{preset_id}-geoip-private"),
                name: "private".into(),
                matcher: RuleMatcher::Geoip,
                server_id: "direct".into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            },
        ]),
        _ => Err(BackendError::new("INVALID_PRESET", "Unknown rule preset")),
    }
}
