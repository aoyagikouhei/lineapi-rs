use base64::Engine;
use sha2::{Digest, Sha256};
use url::Url;

use crate::error::Error;

pub mod get_friendship_v1_status;
pub mod get_oauth2_v2_1_userinfo;
pub mod get_oauth2_v2_1_verify;
pub mod get_v2_profile;
pub mod post_oauth2_v2_1_revoke;
pub mod post_oauth2_v2_1_token;
pub mod post_oauth2_v2_1_verify;
pub mod post_user_v1_deauthorize;

pub enum Scope {
    Profile,
    OpenId,
    Email,
}

fn make_scope_string(scopes: Vec<Scope>) -> String {
    scopes
        .into_iter()
        .map(|scope| match scope {
            Scope::Profile => "profile",
            Scope::OpenId => "openid",
            Scope::Email => "email",
        })
        .collect::<Vec<&str>>()
        .join(" ")
}

pub fn oauth_url(
    client_id: impl Into<String>,
    redirect_uri: impl Into<String>,
    scopes: Vec<Scope>,
    state: impl Into<String>,
    code_verifier: Option<impl Into<String>>,
) -> Result<String, Box<Error>> {
    let mut url = Url::parse("https://access.line.me/oauth2/v2.1/authorize").unwrap();
    {
        let mut query_pairs_mut = url.query_pairs_mut();
        query_pairs_mut
            .append_pair("response_type", "code")
            .append_pair("client_id", &client_id.into())
            .append_pair("redirect_uri", &redirect_uri.into())
            .append_pair("state", &state.into())
            .append_pair("scope", &make_scope_string(scopes));

        if let Some(code_verifier) = code_verifier {
            let code_verifier = code_verifier.into();
            if !(43..=128).contains(&code_verifier.len()) {
                return Err(Box::new(Error::Invalid(
                    "code_verifier is length invalid".to_string(),
                )));
            }
            let code_challenge = Sha256::digest(code_verifier.as_bytes());
            let code_challenge =
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(code_challenge);
            query_pairs_mut.append_pair("code_challenge", &code_challenge);
            query_pairs_mut.append_pair("code_challenge_method", "S256");
        }
    }
    Ok(url.to_string())
}

// テスト
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_oauth_url() -> anyhow::Result<()> {
        let url = oauth_url(
            "client_id",
            "redirect_uri",
            vec![Scope::Profile],
            "state",
            Some("wJKN8qz5t8SSI9lMFhBB6qwNkQBkuPZoCxzRhwLRUo1"),
        )?;
        println!("URL: {url}");
        Ok(())
    }
}
