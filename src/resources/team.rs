use crate::cli::{TeamAction, TeamMemberAction, TeamMemberRole};
use crate::client::{PdClient, encode_query};
use crate::config::Config;
use crate::filter;
use crate::output::print_value;
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::Path;
use tracing::{debug, instrument};

/// Returns the skeleton YAML if the action requests `--example`, otherwise `None`.
/// Called before `Config::load` so `--example` works without an API token.
pub fn example_if_requested(action: &TeamAction) -> Option<&'static str> {
    match action {
        TeamAction::Create { example: true, .. } => Some(EXAMPLE_YAML),
        _ => None,
    }
}

const EXAMPLE_YAML: &str = include_str!("../../examples/team.yml");

#[derive(Debug, Deserialize, Serialize)]
struct TeamYaml {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

pub async fn handle(action: &TeamAction, client: &PdClient, config: &Config) -> Result<()> {
    match action {
        TeamAction::List { patterns } => list(client, config, patterns).await,
        TeamAction::Get { name_or_id } => get(client, config, name_or_id).await,
        TeamAction::Create {
            name,
            description,
            from_file,
            // example is handled in main() before Config::load runs
            example: _,
        } => {
            create(
                client,
                config,
                name.as_deref(),
                description.as_deref(),
                from_file.as_deref(),
            )
            .await
        }
        TeamAction::Update {
            name_or_id,
            name,
            description,
            from_file,
        } => {
            update(
                client,
                config,
                name_or_id,
                name.as_deref(),
                description.as_deref(),
                from_file.as_deref(),
            )
            .await
        }
        TeamAction::Delete { name_or_id } => delete(client, config, name_or_id).await,
        TeamAction::Member { action } => member(client, config, action).await,
    }
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

#[instrument(skip(client, config))]
async fn list(client: &PdClient, config: &Config, patterns: &[String]) -> Result<()> {
    debug!(patterns_len = patterns.len(), "team list");
    let all = if patterns.is_empty() {
        client.get_all("/teams", "teams").await?
    } else {
        client
            .query_all_patterns("/teams", "teams", patterns)
            .await?
    };
    let filtered = filter::filter_into(all, patterns, team_name);
    let result = json!({ "teams": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn get(client: &PdClient, config: &Config, name_or_id: &str) -> Result<()> {
    let resp = resolve_team(client, name_or_id).await?;
    print_value(&resp, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn create(
    client: &PdClient,
    config: &Config,
    name: Option<&str>,
    description: Option<&str>,
    from_file: Option<&Path>,
) -> Result<()> {
    let (resolved_name, resolved_description) = resolve_team_fields(name, description, from_file)?;
    let resolved_name = resolved_name.ok_or_else(|| {
        eyre::eyre!("`pd team create` requires --name or a --from-file with a name field")
    })?;

    let mut body = json!({ "name": resolved_name });
    if let Some(desc) = resolved_description {
        body["description"] = json!(desc);
    }
    let body = json!({ "team": body });
    let result = client.post("/teams", body).await?;

    // Populate cache with the newly-minted ID so the very next
    // `resolve_team*(resolved_name)` is a cache hit instead of a list scan.
    if let Some(cache) = client.cache()
        && let Some(new_id) = result
            .get("team")
            .and_then(|t| t.get("id"))
            .and_then(|v| v.as_str())
    {
        cache.put("team", &resolved_name, new_id);
    }

    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config, from_file))]
async fn update(
    client: &PdClient,
    config: &Config,
    name_or_id: &str,
    name: Option<&str>,
    description: Option<&str>,
    from_file: Option<&Path>,
) -> Result<()> {
    let resolved = resolve_team(client, name_or_id).await?;
    let id = resolved
        .get("team")
        .and_then(|t| t.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Resolved team missing id field"))?
        .to_string();

    // CLI flags override file contents. Start with the resolved team, then apply
    // file fields, then apply CLI overrides on top.
    let mut current = resolved
        .get("team")
        .cloned()
        .ok_or_else(|| eyre::eyre!("Resolved team missing envelope"))?;

    if let Some(path) = from_file {
        let file = load_team_yaml(path)?;
        current["name"] = json!(file.name);
        if let Some(desc) = file.description {
            current["description"] = json!(desc);
        }
    }

    if let Some(n) = name {
        current["name"] = json!(n);
    }
    if let Some(d) = description {
        current["description"] = json!(d);
    }

    let body = json!({ "team": current });
    let result = client.put(&format!("/teams/{}", id), body).await?;

    // Reap every cached entry pointing at this team's id (the one the user
    // just invoked by, plus any orphan mappings left behind by out-of-band
    // UI renames), then write the canonical new-name -> id mapping so the
    // next `resolve_team*(new_name)` hits the cache.
    if let Some(cache) = client.cache()
        && let Some(new_name) = result
            .get("team")
            .and_then(|t| t.get("name"))
            .and_then(|v| v.as_str())
    {
        cache.invalidate_by_id("team", &id);
        cache.put("team", new_name, &id);
    }

    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn delete(client: &PdClient, config: &Config, name_or_id: &str) -> Result<()> {
    let resolved = resolve_team(client, name_or_id).await?;
    let id = resolved
        .get("team")
        .and_then(|t| t.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre::eyre!("Resolved team missing id field"))?
        .to_string();
    let result = client.delete(&format!("/teams/{}", id)).await?;
    if let Some(cache) = client.cache() {
        cache.invalidate_entry("team", name_or_id);
    }
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// Member subresource
// ---------------------------------------------------------------------------

#[instrument(skip(client, config))]
async fn member(client: &PdClient, config: &Config, action: &TeamMemberAction) -> Result<()> {
    match action {
        TeamMemberAction::List { team, patterns } => {
            member_list(client, config, team, patterns).await
        }
        TeamMemberAction::Add { team, user, role } => {
            member_add(client, config, team, user, role).await
        }
        TeamMemberAction::Remove { team, user } => member_remove(client, config, team, user).await,
    }
}

#[instrument(skip(client, config))]
async fn member_list(
    client: &PdClient,
    config: &Config,
    team: &str,
    patterns: &[String],
) -> Result<()> {
    let team_id = resolve_team_id(client, team).await?;
    let all = client
        .get_all(&format!("/teams/{}/members", team_id), "members")
        .await?;
    // PD members wrap each member as {user, role}; filter by nested user.name.
    let filtered = filter::filter_into(all, patterns, member_user_name);
    let result = json!({ "members": filtered });
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn member_add(
    client: &PdClient,
    config: &Config,
    team: &str,
    user: &str,
    role: &TeamMemberRole,
) -> Result<()> {
    let team_id = resolve_team_id(client, team).await?;
    let user_id = resolve_user_id(client, user).await?;
    let body = json!({ "role": role_string(role) });
    let result = client
        .put(&format!("/teams/{}/users/{}", team_id, user_id), body)
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

#[instrument(skip(client, config))]
async fn member_remove(client: &PdClient, config: &Config, team: &str, user: &str) -> Result<()> {
    let team_id = resolve_team_id(client, team).await?;
    let user_id = resolve_user_id(client, user).await?;
    let result = client
        .delete(&format!("/teams/{}/users/{}", team_id, user_id))
        .await?;
    print_value(&result, &config.output_format);
    Ok(())
}

// ---------------------------------------------------------------------------
// Resolution helpers
// ---------------------------------------------------------------------------

/// Resolve a team identifier (ID, slug, or display name) to the full
/// `{"team": {...}}` envelope. Cache integration lives here (not in the
/// `resolve_team_id` wrapper) so any caller needing the full record gets
/// the cache benefit. See the `resolve_service` doc-comment for the
/// cache/404-recovery flow rationale.
pub async fn resolve_team(client: &PdClient, name_or_id: &str) -> Result<Value> {
    if let Some(resp) = client.try_get(&format!("/teams/{}", name_or_id)).await? {
        return Ok(resp);
    }

    if let Some(cache) = client.cache()
        && let Some(cached_id) = cache.get("team", name_or_id)
    {
        match client.try_get(&format!("/teams/{}", cached_id)).await? {
            Some(resp) => return Ok(resp),
            None => cache.invalidate_entry("team", name_or_id),
        }
    }

    let all = client
        .get_all(
            &format!("/teams?query={}", encode_query(name_or_id)),
            "teams",
        )
        .await?;
    let matches = filter::filter(&all, &[name_or_id.to_string()], team_name);
    match matches.as_slice() {
        [] => eyre::bail!("Team {:?} not found (tried ID and name).", name_or_id),
        [single] => {
            if let Some(cache) = client.cache()
                && let Some(id) = single.get("id").and_then(|v| v.as_str())
                && id != name_or_id
            {
                cache.put("team", name_or_id, id);
            }
            Ok(json!({ "team": *single }))
        }
        many => {
            let ids: Vec<&str> = many
                .iter()
                .filter_map(|t| t.get("id").and_then(|v| v.as_str()))
                .collect();
            eyre::bail!(
                "Team name {:?} matches {} teams: {}. Use the ID to disambiguate.",
                name_or_id,
                ids.len(),
                ids.join(", ")
            )
        }
    }
}

/// Resolve to just the team ID.
pub async fn resolve_team_id(client: &PdClient, name_or_id: &str) -> Result<String> {
    let resolved = resolve_team(client, name_or_id).await?;
    resolved
        .get("team")
        .and_then(|t| t.get("id"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| eyre::eyre!("Resolved team missing id field"))
}

pub async fn resolve_user_id(client: &PdClient, email_or_id: &str) -> Result<String> {
    // Cache hit: verify the cached ID still resolves. A 404 here means the
    // user was deleted (or the ID renamed) since we cached; invalidate and
    // fall through. Without this verify step a stale ID would be returned
    // to the caller, which would then 404 at the next API touch.
    if let Some(cache) = client.cache()
        && let Some(cached_id) = cache.get("user", email_or_id)
    {
        match client.try_get(&format!("/users/{}", cached_id)).await? {
            Some(_) => return Ok(cached_id),
            None => cache.invalidate_entry("user", email_or_id),
        }
    }

    let id = resolve_user_id_uncached(client, email_or_id).await?;
    if let Some(cache) = client.cache()
        && id != email_or_id
    {
        cache.put("user", email_or_id, &id);
    }
    Ok(id)
}

async fn resolve_user_id_uncached(client: &PdClient, email_or_id: &str) -> Result<String> {
    if !email_or_id.contains('@')
        && let Some(resp) = client.try_get(&format!("/users/{}", email_or_id)).await?
    {
        return resp
            .get("user")
            .and_then(|u| u.get("id"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| eyre::eyre!("User response missing id field"));
    }
    let all = client
        .get_all(
            &format!("/users?query={}", encode_query(email_or_id)),
            "users",
        )
        .await?;
    let matches: Vec<&Value> = all
        .iter()
        .filter(|u| {
            u.get("email")
                .and_then(|v| v.as_str())
                .map(|e| e.eq_ignore_ascii_case(email_or_id))
                .unwrap_or(false)
        })
        .collect();
    match matches.as_slice() {
        [single] => single
            .get("id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| eyre::eyre!("User match missing id field")),
        [] => eyre::bail!("No user found matching {:?}", email_or_id),
        many => {
            let ids: Vec<&str> = many
                .iter()
                .filter_map(|u| u.get("id").and_then(|v| v.as_str()))
                .collect();
            eyre::bail!(
                "Multiple users match {:?}: {}. Use the PagerDuty ID to disambiguate.",
                email_or_id,
                ids.join(", ")
            )
        }
    }
}

fn resolve_team_fields(
    name: Option<&str>,
    description: Option<&str>,
    from_file: Option<&Path>,
) -> Result<(Option<String>, Option<String>)> {
    let (mut final_name, mut final_desc) = (None, None);
    if let Some(path) = from_file {
        let file = load_team_yaml(path)?;
        final_name = Some(file.name);
        final_desc = file.description;
    }
    if let Some(n) = name {
        final_name = Some(n.to_string());
    }
    if let Some(d) = description {
        final_desc = Some(d.to_string());
    }
    Ok((final_name, final_desc))
}

fn load_team_yaml(path: &Path) -> Result<TeamYaml> {
    let content = read_path_or_stdin(path)?;
    serde_yaml::from_str::<TeamYaml>(&content)
        .with_context(|| format!("Failed to parse team YAML from {}", display_path(path)))
}

fn read_path_or_stdin(path: &Path) -> Result<String> {
    if path == Path::new("-") {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("Failed to read stdin")?;
        Ok(buf)
    } else {
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", display_path(path)))
    }
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn team_name(value: &Value) -> &str {
    value.get("name").and_then(|v| v.as_str()).unwrap_or("")
}

fn member_user_name(value: &Value) -> &str {
    value
        .get("user")
        .and_then(|u| u.get("summary"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

fn role_string(role: &TeamMemberRole) -> &'static str {
    match role {
        TeamMemberRole::Observer => "observer",
        TeamMemberRole::Responder => "responder",
        TeamMemberRole::Manager => "manager",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn team_name_reads_name_field() {
        let v = json!({"id": "PT1", "name": "Platform"});
        assert_eq!(team_name(&v), "Platform");
    }

    #[test]
    fn member_user_name_reads_user_summary() {
        let v = json!({"user": {"id": "U1", "summary": "Scott Idler"}, "role": "responder"});
        assert_eq!(member_user_name(&v), "Scott Idler");
    }

    #[test]
    fn role_string_matches_pd_vocabulary() {
        assert_eq!(role_string(&TeamMemberRole::Observer), "observer");
        assert_eq!(role_string(&TeamMemberRole::Responder), "responder");
        assert_eq!(role_string(&TeamMemberRole::Manager), "manager");
    }

    #[test]
    fn example_yaml_parses() {
        // Uncomment the description line to exercise both fields
        let body = EXAMPLE_YAML.replace("# description:", "description:");
        let parsed: TeamYaml = serde_yaml::from_str(&body).unwrap();
        assert_eq!(parsed.name, "Platform");
        assert!(parsed.description.as_ref().unwrap().contains("SRE"));
    }

    #[test]
    fn cli_flag_overrides_from_file() {
        // Construct a tempfile-like situation by synthesizing the TeamYaml.
        // resolve_team_fields takes from_file: Option<&Path> — simulate by
        // writing to a temp path.
        let dir = std::env::temp_dir();
        let file_path = dir.join("pd_team_test.yml");
        std::fs::write(&file_path, "name: FromFile\ndescription: from the file\n").unwrap();

        let (name, desc) = resolve_team_fields(Some("CliName"), None, Some(&file_path)).unwrap();
        assert_eq!(name.as_deref(), Some("CliName"));
        assert_eq!(desc.as_deref(), Some("from the file"));

        std::fs::remove_file(&file_path).ok();
    }
}
