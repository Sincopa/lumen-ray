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
    // Last regional-indicator pair. A white flag or a second flag can sit in front.
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
            let provider_rules = crate::subscription::routing::subscription_preset_id(&id);
            next.subscriptions.retain(|s| s.id != id);
            next.nodes.retain(|n| n.subscription_id != id);
            next.sources.remove(&id);
            next.selected_servers.remove(&id);
            next.rules.retain(|r| {
                r.preset_id.as_deref() != Some(provider_rules.as_str())
                    && (r.server_id == "direct"
                        || r.server_id == "subscription"
                        || next
                            .nodes
                            .iter()
                            .any(|n| n.id == r.server_id || n.subscription_id == r.server_id))
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
        crate::xray::config::build(&next, id, 10808, 10809, next.settings.network_mode == "tun")?;
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
/// A downloaded and parsed subscription that has not been applied yet.
pub struct PreparedImport {
    input: String,
    id: String,
    refresh: bool,
    resolved_name: String,
    user_info: Option<crate::subscription::fetcher::SubscriptionUserInfo>,
    parsed: crate::subscription::parser::ParsedSubscription,
}
#[cfg(test)]
pub async fn import(
    state: &AppState,
    input: String,
    name: Option<String>,
    refresh_id: Option<String>,
) -> Result<ImportResult> {
    let prepared = prepare_import(state, input, name, refresh_id).await?;
    apply_import(state, prepared).await
}
/// Network and parsing only: callers do not hold the maintenance transaction
/// while a subscription downloads, so connecting is never blocked by it.
pub async fn prepare_import(
    state: &AppState,
    input: String,
    name: Option<String>,
    refresh_id: Option<String>,
) -> Result<PreparedImport> {
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
    if let Some(header) = fetched.routing.as_deref() {
        use crate::subscription::routing::HeaderRouting;
        match crate::subscription::routing::header_routing(header) {
            HeaderRouting::Off => {
                parsed.rules.clear();
                parsed.routing_off = true;
            }
            HeaderRouting::Profile(profile) if parsed.rules.is_empty() && !parsed.routing_off => {
                parsed.rules = crate::subscription::routing::from_profiles(
                    std::slice::from_ref(&profile),
                    &id,
                    &parsed.nodes,
                );
            }
            HeaderRouting::Profile(_) | HeaderRouting::Absent => {}
        }
    }
    let mut skipped = std::collections::BTreeMap::new();
    for diagnostic in &parsed.diagnostics {
        *skipped.entry(diagnostic.code).or_insert(0usize) += 1;
    }
    log::info!(
        "Subscription {operation} parsed: {} supported nodes, {} skipped ({skipped:?})",
        parsed.nodes.len(),
        parsed.diagnostics.len()
    );
    Ok(PreparedImport {
        input,
        id,
        refresh: refresh_id.is_some(),
        resolved_name,
        user_info: fetched.user_info,
        parsed,
    })
}
pub async fn apply_import(state: &AppState, prepared: PreparedImport) -> Result<ImportResult> {
    let PreparedImport {
        input,
        id,
        refresh,
        resolved_name,
        user_info,
        mut parsed,
    } = prepared;
    let _guard = state.mutations.lock().await;
    let mut next = state.repository.read().await.clone();
    if refresh && !next.sources.contains_key(&id) {
        return Err(BackendError::invalid());
    }
    let change = reconcile_nodes(
        next.nodes.iter().filter(|n| n.subscription_id == id),
        &mut parsed.nodes,
        &id,
    );
    next.subscription_changes.push(change);
    let overflow = next.subscription_changes.len().saturating_sub(100);
    next.subscription_changes.drain(..overflow);
    next.nodes.retain(|n| n.subscription_id != id);
    next.nodes.extend(parsed.nodes);
    if let Some(sub) = next.subscriptions.iter_mut().find(|s| s.id == id) {
        sub.updated_at = now();
        if let Some(info) = user_info {
            sub.upload = info.upload;
            sub.download = info.download;
            sub.total = info.total;
            sub.expire = info.expire;
        }
    } else {
        let info = user_info.unwrap_or_default();
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
    let provider_rules = crate::subscription::routing::subscription_preset_id(&id);
    let previous_provider: Vec<Rule> = next
        .rules
        .iter()
        .filter(|rule| rule.preset_id.as_deref() == Some(provider_rules.as_str()))
        .cloned()
        .collect();
    let incoming = if parsed.routing_off {
        Vec::new()
    } else {
        std::mem::take(&mut parsed.rules)
    };
    if incoming != previous_provider {
        next.rules
            .retain(|rule| rule.preset_id.as_deref() != Some(provider_rules.as_str()));
        if !incoming.is_empty() {
            // Provider rules only run in Rules mode. Global mode ignores them and
            // would send Russian banks through the VPN as well.
            next.routing_mode = "rules".into();
        }
        next.rules.extend(incoming);
        next.rules = ordered_rules(&next.rules).into_iter().cloned().collect();
    }
    if !refresh {
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
            let engine = state.session.lock().await.engine();
            crate::engine::backend(engine)
                .validate_runtime(state, &next, &active)
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
        // Russian sites and the rest of the system stay direct. Only the
        // blocking lists, YouTube, Telegram and the Discord process use the VPN.
        // A final MATCH would tunnel the whole system, and VLESS Vision cannot
        // carry Discord's voice UDP, so that traffic must not be forced in.
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
            // The desktop client, including voice, is blocked and must use the
            // VPN. A domain list misses the voice-server addresses.
            Rule {
                id: format!("{preset_id}-application-Discord.exe"),
                name: "Discord.exe".into(),
                matcher: RuleMatcher::Application,
                server_id: target_proxy.into(),
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
        // Telegram is throttled by DPI but absent from the antifilter lists, so
        // t.me and web.telegram.org time out on the direct path without these.
        // Discord is blocked too; its gateway and CDN hosts are not all in antifilter.
        for (name, matcher) in [
            ("youtube", RuleMatcher::Geosite),
            ("discord", RuleMatcher::Geosite),
            ("telegram", RuleMatcher::Geosite),
            ("telegram", RuleMatcher::Geoip),
        ] {
            let kind = if matcher == RuleMatcher::Geosite {
                "geosite"
            } else {
                "geoip"
            };
            rules.push(Rule {
                id: format!("{preset_id}-{kind}-{name}"),
                name: name.into(),
                matcher,
                server_id: target_proxy.into(),
                enabled: true,
                preset_id: Some(preset_id.into()),
                is_domain: false,
            });
        }
    }
    Ok(ordered_rules(&rules).into_iter().cloned().collect())
}

/// Only built-in preset rows are sorted. Manual rows retain their relative order and priority.
/// Compile saved rows as-is: do not recreate deleted/disabled rows when loading or connecting.
///
/// An application routed through a VPN matches every destination, so it is evaluated after
/// the built-in presets: Russian services that reject foreign exits (ФНС, РЖД, Почта, ФССП)
/// still go direct from that application, and blocked ones still use the VPN. Manual
/// direct/block applications and manual domain/IP/geo rows keep the highest priority.
pub fn ordered_rules(rules: &[Rule]) -> Vec<&Rule> {
    use crate::storage::repository::RuleMatcher;
    let mut rows: Vec<_> = rules.iter().collect();
    rows.sort_by_key(|r| {
        let builtin = matches!(
            r.preset_id.as_deref(),
            Some("russia-optimized" | "bypass-blocking" | "russia-direct")
        );
        // Russian domains and IPs come before every proxy list. Otherwise a
        // site such as 2ip.ru that also appears in a blocking list, or that
        // resolves to a foreign CDN address, leaves through the VPN.
        let rank = if r.matcher == RuleMatcher::Final {
            9
        } else if builtin
            && r.matcher == RuleMatcher::Application
            && !["direct", "block"].contains(&r.server_id.as_str())
        {
            // A preset application (Discord) talks only to blocked services. Its
            // voice servers are bare addresses, so a Russian GeoIP match must not
            // send them to the direct path where they drop after a few minutes.
            1
        } else if !builtin {
            if r.matcher == RuleMatcher::Application
                && !["direct", "block"].contains(&r.server_id.as_str())
            {
                7
            } else {
                0
            }
        } else if r.matcher == RuleMatcher::Geoip && r.name == "private" {
            1
        } else if r.matcher == RuleMatcher::Geosite && r.server_id == "direct" {
            2
        } else if r.matcher == RuleMatcher::Geoip && r.server_id == "direct" {
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

/// Replace a preset that still tunnels every connection, or that does not send
/// Discord.exe through the VPN.
pub fn upgrade_russia_optimized(repo: &mut Repository) -> bool {
    use crate::storage::repository::RuleMatcher;
    let preset = "russia-optimized";
    let current: Vec<Rule> = repo
        .rules
        .iter()
        .filter(|rule| rule.preset_id.as_deref() == Some(preset))
        .cloned()
        .collect();
    // An earlier version shipped the Discord.exe row disabled, which left voice on
    // the blocked direct path.
    let discord_via_vpn = current.iter().any(|rule| {
        rule.matcher == RuleMatcher::Application
            && rule.enabled
            && rule.name.eq_ignore_ascii_case("Discord.exe")
            && !matches!(rule.server_id.as_str(), "direct" | "block")
    });
    let has = |name: &str, matcher: RuleMatcher| {
        current
            .iter()
            .any(|rule| rule.matcher == matcher && rule.name == name)
    };
    let russian_domains_lose = current.iter().position(|rule| rule.name == "youtube")
        < current.iter().position(|rule| rule.name == "category-ru");
    let stale = current
        .iter()
        .any(|rule| rule.matcher == RuleMatcher::Final)
        || !discord_via_vpn
        || !has("discord", RuleMatcher::Geosite)
        || !has("youtube", RuleMatcher::Geosite)
        || russian_domains_lose;
    if current.is_empty() || !stale {
        return false;
    }
    let target = current
        .iter()
        .find(|rule| !matches!(rule.server_id.as_str(), "direct" | "block"))
        .map(|rule| rule.server_id.clone())
        .unwrap_or_else(|| "subscription".into());
    let Ok(rules) = generate_preset_rules(preset, &target) else {
        return false;
    };
    repo.rules
        .retain(|rule| rule.preset_id.as_deref() != Some(preset));
    repo.rules.extend(rules);
    repo.rules = ordered_rules(&repo.rules).into_iter().cloned().collect();
    true
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
        session.started().map(|_| session.proxy_port())
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

fn reconcile_nodes<'a>(
    old: impl IntoIterator<Item = &'a crate::domain::node::VpnNode>,
    incoming: &mut [crate::domain::node::VpnNode],
    subscription_id: &str,
) -> crate::storage::repository::SubscriptionChange {
    use crate::domain::node::{Protocol, VpnNode};
    use std::collections::{HashMap, HashSet};
    fn endpoint(node: &VpnNode) -> (&str, u16, Protocol, &str) {
        (&node.address, node.port, node.protocol, &node.name)
    }
    let old = old.into_iter();
    let mut by_id = HashMap::with_capacity(old.size_hint().0);
    let mut by_endpoint: HashMap<_, Vec<_>> = HashMap::new();
    let mut old_count = 0;
    for previous in old {
        old_count += 1;
        by_id.entry(previous.id.as_str()).or_insert(previous);
        by_endpoint
            .entry(endpoint(previous))
            .or_default()
            .push(previous);
    }
    let mut claimed = HashSet::with_capacity(old_count);
    let mut added = 0;
    let mut updated = 0;
    for node in incoming {
        let same_id = by_id
            .get(node.id.as_str())
            .copied()
            .filter(|previous| !claimed.contains(previous.id.as_str()));
        let previous = same_id.or_else(|| {
            let mut matches = by_endpoint
                .get(&endpoint(node))?
                .iter()
                .copied()
                .filter(|previous| !claimed.contains(previous.id.as_str()));
            let first = matches.next()?;
            // Ambiguous endpoints must never merge two different accounts.
            matches.next().is_none().then_some(first)
        });
        if let Some(previous) = previous {
            node.id = previous.id.clone();
            if previous != node {
                updated += 1;
            }
            claimed.insert(previous.id.as_str());
        } else {
            added += 1;
        }
    }
    crate::storage::repository::SubscriptionChange {
        subscription_id: subscription_id.into(),
        at: now(),
        added,
        updated,
        removed: old_count.saturating_sub(claimed.len()),
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;

    // Preserve the previous algorithm as an oracle for ambiguous endpoints,
    // duplicate IDs and rotations that have already claimed a previous row.
    fn reference_reconcile(
        old: &[crate::domain::node::VpnNode],
        incoming: &mut [crate::domain::node::VpnNode],
    ) -> (usize, usize, usize) {
        let mut claimed = std::collections::HashSet::new();
        let (mut added, mut updated) = (0, 0);
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
            let previous = same_id.or_else(|| (matches.len() == 1).then(|| matches[0]));
            if let Some(previous) = previous {
                node.id = previous.id.clone();
                updated += usize::from(previous != node);
                claimed.insert(previous.id.clone());
            } else {
                added += 1;
            }
        }
        (added, updated, old.len().saturating_sub(claimed.len()))
    }

    #[test]
    fn indexed_refresh_matches_previous_algorithm_on_collisions_and_reordering() {
        let old: Vec<_> = (0..12)
            .map(|i| {
                crate::subscription::uri::parse(
                    &format!(
                        "trojan://password-{i}@host-{}.example:443#Server-{}",
                        i / 3,
                        i / 3
                    ),
                    "sub",
                )
                .unwrap()
            })
            .collect();
        for duplicate_ids in [false, true] {
            let mut old = old.clone();
            if duplicate_ids {
                old[1].id = old[0].id.clone();
            }
            for rotation in 0..old.len() {
                let mut incoming = old.clone();
                incoming.rotate_left(rotation);
                incoming.truncate(9);
                for (i, node) in incoming.iter_mut().enumerate() {
                    if i % 2 == 0 {
                        node.credential = "rotated-password".into();
                        node.identify().unwrap();
                    }
                }
                incoming.push(old[0].clone());
                incoming.push(
                    crate::subscription::uri::parse("socks5://new.example:1080", "sub").unwrap(),
                );
                let mut expected = incoming.clone();
                let counts = reference_reconcile(&old, &mut expected);
                let change = reconcile_nodes(&old, &mut incoming, "sub");
                assert!(incoming == expected);
                assert_eq!((change.added, change.updated, change.removed), counts);
            }
        }
    }

    #[test]
    #[ignore = "Performance diagnostic; compares a 4096-server refresh with the previous algorithm"]
    fn indexed_refresh_benchmark() {
        let template = crate::subscription::uri::parse("socks5://example.org:1080", "sub").unwrap();
        let old: Vec<_> = (0..4096)
            .map(|i| {
                let mut node = template.clone();
                node.id = i.to_string();
                node.address = format!("server-{i}.example");
                node
            })
            .collect();
        let mut incoming = old.clone();
        incoming.reverse();
        let mut expected = incoming.clone();
        let before = std::time::Instant::now();
        let counts = reference_reconcile(&old, &mut expected);
        let baseline = before.elapsed();
        let before = std::time::Instant::now();
        let change = reconcile_nodes(&old, &mut incoming, "sub");
        let optimized = before.elapsed();
        assert!(incoming == expected);
        assert_eq!((change.added, change.updated, change.removed), counts);
        println!("4096-server refresh: previous={baseline:?}, indexed={optimized:?}");
    }
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
