// Signing in and out with an access token, choosing the team calls are for, and
// saying who is signed in.

use std::path::Path;

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{self, at, text, Session};
use crate::config::{self, Account};
use crate::error::{VercelError, VercelResult};

/// Who a token belongs to, as `/v2/user` says.
pub fn account_of(data: &Value) -> VercelResult<Account> {
    let user = at(data, "/user");
    let id = text(user, "/id");
    if id.is_empty() {
        return Err(VercelError::Response(
            "Vercel did not say whose token this is".into(),
        ));
    }
    Ok(Account {
        id,
        username: text(user, "/username"),
        name: text(user, "/name"),
        email: text(user, "/email"),
    })
}

#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub id: String,
    pub slug: String,
    pub name: String,
}

pub fn teams_of(data: &Value) -> Vec<Team> {
    at(data, "/teams")
        .as_array()
        .into_iter()
        .flatten()
        .map(|team| Team {
            id: text(team, "/id"),
            slug: text(team, "/slug"),
            name: text(team, "/name"),
        })
        .filter(|team| !team.id.is_empty())
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub configured: bool,
    pub ok: bool,
    pub auth_failed: bool,
    pub message: Option<String>,
    pub account: Option<Account>,
    pub teams: Vec<Team>,
    /// The team calls use when they name none, by id; none for the account's own projects.
    pub team: Option<String>,
    /// Whether the account has projects of its own beside its teams. Newer accounts
    /// keep theirs in a default team instead, which is then the team chosen.
    pub personal: bool,
}

/// Who is signed in, whether the token still works, and the teams it can see.
pub async fn status(data_dir: &Path) -> Status {
    let config = config::load(data_dir);
    let base = |ok: bool,
                auth_failed: bool,
                message: Option<String>,
                teams: Vec<Team>,
                default_team: Option<String>| Status {
        configured: config.account.is_some(),
        ok,
        auth_failed,
        message,
        account: config.account.clone(),
        personal: default_team.is_none(),
        team: config.team.clone().or(default_team),
        teams,
    };
    let session = match Session::of(data_dir, Some("")).await {
        Ok(session) => session,
        Err(VercelError::Unconfigured) => return base(false, false, None, Vec::new(), None),
        Err(error) => return base(false, false, Some(error.to_string()), Vec::new(), None),
    };
    let answered = futures::future::try_join(
        session.get("/v2/user", &[]),
        session.get("/v2/teams", &[("limit", "100".into())]),
    )
    .await;
    match answered {
        Ok((user, teams)) => {
            let teams = teams_of(&teams);
            let default_team = Some(text(&user, "/user/defaultTeamId"))
                .filter(|id| teams.iter().any(|team| &team.id == id));
            base(true, false, None, teams, default_team)
        }
        Err(error @ VercelError::Auth(_)) => {
            base(false, true, Some(error.to_string()), Vec::new(), None)
        }
        Err(error @ VercelError::RateLimited) => {
            base(true, false, Some(error.to_string()), Vec::new(), None)
        }
        Err(error) => base(false, false, Some(error.to_string()), Vec::new(), None),
    }
}

#[derive(Deserialize)]
pub struct TokenSignIn {
    pub token: String,
}

/// Signs in with an access token, replacing any account signed in before.
pub async fn sign_in(data_dir: &Path, input: TokenSignIn) -> VercelResult<()> {
    let token = input.token.trim().to_string();
    if token.is_empty() {
        return Err(VercelError::BadArg("no access token was given".into()));
    }
    if token.chars().any(char::is_whitespace) {
        return Err(VercelError::BadArg(
            "that is not a Vercel access token; make one at vercel.com/account/tokens".into(),
        ));
    }
    let account = account_of(&client::request(&token, Method::GET, "/v2/user", &[], None).await?)?;
    let data_dir = data_dir.to_path_buf();
    config::blocking(Box::new(move || {
        let mut config = config::load(&data_dir);
        if let Some(previous) = config
            .account
            .as_ref()
            .filter(|previous| previous.id != account.id)
        {
            config::keychain_delete(previous)?;
            config.team = None;
        }
        config::keychain_write(&account, &token)?;
        config.account = Some(account);
        config::save(&data_dir, &config)
    }))
    .await?;
    client::forget();
    Ok(())
}

pub async fn sign_out(data_dir: &Path) -> VercelResult<()> {
    let data_dir = data_dir.to_path_buf();
    config::blocking(Box::new(move || {
        let mut config = config::load(&data_dir);
        if let Some(account) = config.account.take() {
            config::keychain_delete(&account)?;
        }
        config.team = None;
        config::save(&data_dir, &config)
    }))
    .await?;
    client::forget();
    Ok(())
}

#[derive(Deserialize)]
pub struct TeamChoice {
    /// A team's id, or empty for the account's own projects.
    #[serde(default)]
    pub team: Option<String>,
}

/// Makes the team named the one calls use when they name none.
pub async fn set_team(data_dir: &Path, input: TeamChoice) -> VercelResult<()> {
    let team = input
        .team
        .map(|team| team.trim().to_string())
        .filter(|team| !team.is_empty());
    let data_dir = data_dir.to_path_buf();
    config::blocking(Box::new(move || {
        let mut config = config::load(&data_dir);
        if config.account.is_none() {
            return Err(VercelError::Unconfigured);
        }
        config.team = team;
        config::save(&data_dir, &config)
    }))
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_token_belongs_to_its_user_and_sees_their_teams() {
        let user = json!({ "user": { "id": "u1", "username": "ankit", "name": "Ankit", "email": "a@example.com" } });
        assert_eq!(
            account_of(&user).expect("reads"),
            Account {
                id: "u1".into(),
                username: "ankit".into(),
                name: "Ankit".into(),
                email: "a@example.com".into(),
            }
        );
        assert!(account_of(&json!({ "user": {} })).is_err());
        let teams = json!({ "teams": [{ "id": "team_1", "slug": "acme", "name": "Acme" }, { "slug": "no-id" }] });
        assert_eq!(
            teams_of(&teams),
            [Team {
                id: "team_1".into(),
                slug: "acme".into(),
                name: "Acme".into()
            }]
        );
    }
}
