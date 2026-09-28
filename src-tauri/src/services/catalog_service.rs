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
            let has_user_info = subscription.total.is_some()
                || subscription.upload.is_some()
                || subscription.download.is_some();
            let is_infinite_days = subscription.expire == Some(0)
                || (subscription.expire.is_none() && has_user_info)
                || subscription
                    .expire
                    .is_some_and(|expire| expire.saturating_sub(now_seconds) / 86_400 >= 3650);

            let status = if is_infinite_days {
                Some("active")
            } else {
                subscription.expire.filter(|expire| *expire > 0).map(|expire| {
                    if expire <= now_seconds {
                        "expired"
                    } else {
                        "active"
                    }
                })
            };
            let days_remaining: Option<Value> = if is_infinite_days {
                Some(json!("∞"))
            } else {
                subscription.expire.filter(|expire| *expire > 0).map(|expire| {
                    json!(expire.saturating_sub(now_seconds) / 86_400)
                })
            };
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
    let mut chars = name.trim().chars();
    loop {
        let rest = chars.as_str();
        let Some(c1) = chars.next() else {
            break;
        };
        if c1.is_whitespace() || c1 == '\u{1f3f3}' || c1 == '\u{fe0f}' {
            continue;
        }
        if ('\u{1f1e6}'..='\u{1f1ff}').contains(&c1) {
            let mut lookahead = chars.clone();
            if let Some(c2) = lookahead.next() {
                if ('\u{1f1e6}'..='\u{1f1ff}').contains(&c2) {
                    chars = lookahead;
                    continue;
                }
            }
        }
        let trimmed = rest.trim();
        if trimmed.starts_with("Белые Списки / ") {
            let without_flags: String = trimmed
                .chars()
                .filter(|c| !('\u{1f1e6}'..='\u{1f1ff}').contains(c))
                .collect();
            return without_flags.trim().to_string();
        }
        return trimmed.to_string();
    }
    String::new()
}
fn country(name: &str) -> String {
    // Look for the LAST pair of Regional Indicator Symbols (country flag emoji).
    // Some names start with 🏳️ (white flag) before the real country flag,
    // or contain multiple flags — prefer the last actual country flag found.
    let mut last_pair: Option<(char, char)> = None;
    let mut prev: Option<char> = None;
    for c in name.chars() {
        if ('\u{1f1e6}'..='\u{1f1ff}').contains(&c) {
            if let Some(p) = prev {
                if let (Some(a), Some(b)) = (
                    char::from_u32(p as u32 - 0x1f1e6 + 'a' as u32),
                    char::from_u32(c as u32 - 0x1f1e6 + 'a' as u32),
                ) {
                    last_pair = Some((a, b));
                }
            }
            prev = Some(c);
        } else {
            prev = None;
        }
    }
    if let Some((a, b)) = last_pair {
        let mut result = String::with_capacity(2);
        result.push(a);
        result.push(b);
        result
    } else {
        String::new()
    }
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
            let mut updated_rules = custom_rules;
            updated_rules.extend(preset_rules);
            next.rules = ordered_rules(&updated_rules).into_iter().cloned().collect();
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
    let fetched = fetch_subscription(state, &input)
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
            url::Url::parse(input.trim()).ok().and_then(|url| {
                if let Some(fragment) = url.fragment().map(str::trim).filter(|f| !f.is_empty()) {
                    let decoded =
                        percent_encoding::percent_decode_str(fragment).decode_utf8_lossy();
                    let cleaned = decoded.trim();
                    if !cleaned.is_empty() {
                        return Some(cleaned.to_string());
                    }
                }
                for (k, v) in url.query_pairs() {
                    if (k.eq_ignore_ascii_case("name") || k.eq_ignore_ascii_case("title"))
                        && !v.trim().is_empty()
                    {
                        return Some(v.trim().to_string());
                    }
                }
                url.host_str()
                    .map(|host| host.trim_start_matches("www.").to_owned())
            })
        })
        .unwrap_or_else(|| "Subscription".into());
    let id = refresh_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut parsed =
        crate::subscription::parser::parse(&fetched.body, &id).inspect_err(|error| {
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
    let old: Vec<_> = next
        .nodes
        .iter()
        .filter(|n| n.subscription_id == id)
        .cloned()
        .collect();
    let change = reconcile_nodes(&old, &mut parsed.nodes, &id);
    next.subscription_changes.push(change);
    let overflow = next.subscription_changes.len().saturating_sub(100);
    next.subscription_changes.drain(..overflow);
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
    if let Some(active) = state
        .snapshot
        .read()
        .await
        .info
        .as_ref()
        .map(|info| info.server_id.clone())
    {
        if next.nodes.iter().any(|node| node.id == active) {
            let config = crate::xray::config::build(
                &next,
                &active,
                10808,
                10809,
                next.settings.network_mode == "tun",
            )?;
            super::vpn_service::validate(
                &state.core.read().await,
                &serde_json::to_vec(&config).map_err(|_| BackendError::invalid())?,
            )
            .await?;
        }
    }
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
    let mut rules = match preset_id {
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
    }?;
    if matches!(preset_id, "russia-optimized" | "bypass-blocking") {
        rules.push(Rule {
            id: format!("{preset_id}-geosite-youtube"),
            name: "youtube".into(),
            matcher: RuleMatcher::Geosite,
            server_id: target_proxy.into(),
            enabled: true,
            preset_id: Some(preset_id.into()),
            is_domain: false,
        });
    }
    Ok(ordered_rules(&rules).into_iter().cloned().collect())
}

/// Only built-in preset rows are sorted. Manual rows retain their relative order and priority.
/// Compile saved rows as-is: do not recreate deleted/disabled rows when loading or connecting.
pub fn ordered_rules(rules: &[Rule]) -> Vec<&Rule> {
    use crate::storage::repository::RuleMatcher;
    let mut rows: Vec<_> = rules.iter().collect();
    rows.sort_by_key(|r| {
        let builtin = matches!(
            r.preset_id.as_deref(),
            Some("russia-optimized" | "bypass-blocking" | "russia-direct")
        );
        let rank = if !builtin {
            0
        } else if r.matcher == RuleMatcher::Geoip && r.name == "private" {
            1
        } else if r.matcher == RuleMatcher::Geosite && r.name == "youtube" {
            2
        } else if r.matcher == RuleMatcher::Geosite && r.server_id != "direct" {
            3
        } else if r.matcher == RuleMatcher::Geosite {
            4
        } else if r.server_id != "direct" {
            5
        } else {
            6
        };
        (rank, if builtin { r.id.as_str() } else { "" })
    });
    rows
}

async fn fetch_subscription(
    state: &AppState,
    input: &str,
) -> Result<crate::subscription::fetcher::FetchedSubscription> {
    if !input.trim().starts_with("https://") && !input.trim().starts_with("http://") {
        return state.subscription_client.fetch_with_metadata(input).await;
    }
    let transport = state
        .repository
        .read()
        .await
        .settings
        .subscription_transport
        .clone();
    let port = {
        let session = state.session.lock().await;
        session.started.map(|_| session.proxy_port)
    };
    if transport == "proxy" {
        let port = port.ok_or_else(|| {
            BackendError::new(
                "NOT_CONNECTED",
                "Connect before downloading through the VPN",
            )
        })?;
        return state
            .subscription_client
            .through_proxy(port)?
            .fetch_with_metadata(input)
            .await;
    }
    match state.subscription_client.fetch_with_metadata(input).await {
        Err(error)
            if transport == "auto"
                && error.code == "SUBSCRIPTION_FETCH_FAILED"
                && port.is_some() =>
        {
            state
                .subscription_client
                .through_proxy(port.unwrap())?
                .fetch_with_metadata(input)
                .await
        }
        result => result,
    }
}

fn reconcile_nodes(
    old: &[crate::domain::node::VpnNode],
    incoming: &mut [crate::domain::node::VpnNode],
    subscription_id: &str,
) -> crate::storage::repository::SubscriptionChange {
    let mut claimed = std::collections::HashSet::new();
    let mut added = 0;
    let mut updated = 0;
    for node in incoming {
        let same_id = old
            .iter()
            .find(|previous| previous.id == node.id && !claimed.contains(&previous.id));
        let matches: Vec<_> = old
            .iter()
            .filter(|previous| {
                !claimed.contains(&previous.id)
                    && previous.address == node.address
                    && previous.port == node.port
                    && previous.protocol == node.protocol
                    && previous.name == node.name
            })
            .collect();
        let previous = same_id.or_else(|| {
            if matches.len() == 1 {
                matches.first().copied()
            } else {
                None
            }
        });
        if let Some(previous) = previous {
            node.id = previous.id.clone();
            if previous != node {
                updated += 1;
            }
            claimed.insert(previous.id.clone());
        } else {
            added += 1;
        }
    }
    crate::storage::repository::SubscriptionChange {
        subscription_id: subscription_id.into(),
        at: now(),
        added,
        updated,
        removed: old.len().saturating_sub(claimed.len()),
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;
    #[test]
    fn credential_rotation_preserves_selection_and_reports_change() {
        let old =
            crate::subscription::uri::parse("trojan://old-password@example.org:443#Server", "sub")
                .unwrap();
        let new =
            crate::subscription::uri::parse("trojan://new-password@example.org:443#Server", "sub")
                .unwrap();
        let mut incoming = vec![new];
        let change = reconcile_nodes(std::slice::from_ref(&old), &mut incoming, "sub");
        assert_eq!(incoming[0].id, old.id);
        assert_eq!((change.added, change.updated, change.removed), (0, 1, 0));
        let change = reconcile_nodes(&incoming.clone(), &mut incoming, "sub");
        assert_eq!((change.added, change.updated, change.removed), (0, 0, 0));
    }
}
