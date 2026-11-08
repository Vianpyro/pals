//! Sign-in by an LDAP bind against the institution's directory over LDAPS only (ADR-0017)

use std::{env, error::Error, sync::Arc, time::Duration};

use contracts::Identity;
use ldap3::{
    Ldap, LdapConnAsync, LdapConnSettings, LdapError, ResultEntry, Scope, SearchEntry, ldap_escape,
};
use rustls::{
    ClientConfig, RootCertStore,
    pki_types::{CertificateDer, pem::PemObject},
};
use tokio::time;

// For the whole sign-in: a directory that does not answer refuses it instead of holding the request
const TIMEOUT: Duration = Duration::from_secs(5);
// Bind result codes (RFC 4511, appendix A)
const SUCCESS: u32 = 0;
const INVALID_CREDENTIALS: u32 = 49;
const NAME: &str = "displayName";
const EMAIL: &str = "mail";

// No Debug: it holds the service account's password
pub(super) struct Directory {
    url: String,
    // Trusts the directory's own CA only, never the system's (ADR-0017)
    tls: Arc<ClientConfig>,
    base_dn: String,
    user_attribute: String,
    bind_dn: String,
    bind_password: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SignInError {
    Refused,
    Unavailable,
}

// The error is dropped: its text can carry the user's DN, which logs must not (Logs)
impl From<LdapError> for SignInError {
    fn from(_: LdapError) -> Self {
        Self::Unavailable
    }
}

impl Directory {
    pub(super) fn from_env() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            url: ldaps(env::var("LDAP_URL")?)?,
            tls: pinned(&env::var("LDAP_CA_FILE")?)?,
            base_dn: env::var("LDAP_BASE_DN")?,
            user_attribute: env::var("LDAP_USER_ATTRIBUTE")?,
            bind_dn: env::var("LDAP_BIND_DN")?,
            bind_password: env::var("LDAP_BIND_PASSWORD")?,
        })
    }

    #[cfg(test)]
    pub(super) fn unreachable() -> Directory {
        let tls = ClientConfig::builder()
            .with_root_certificates(RootCertStore::empty())
            .with_no_client_auth();
        Directory {
            url: "ldaps://127.0.0.1:1".to_string(),
            tls: Arc::new(tls),
            base_dn: String::new(),
            user_attribute: "uid".to_string(),
            bind_dn: String::new(),
            bind_password: String::new(),
        }
    }

    pub(super) async fn sign_in(&self, id: &str, password: &str) -> Result<Identity, SignInError> {
        // An empty password makes an anonymous bind, which succeeds (RFC 4513, section 5.1.2)
        if id.is_empty() || password.is_empty() {
            return Err(SignInError::Refused);
        }

        time::timeout(TIMEOUT, self.authenticate(id, password))
            .await
            .unwrap_or(Err(SignInError::Unavailable))
    }

    async fn authenticate(&self, id: &str, password: &str) -> Result<Identity, SignInError> {
        let settings = LdapConnSettings::new().set_config(Arc::clone(&self.tls));
        let (connection, mut ldap) = LdapConnAsync::with_settings(settings, &self.url).await?;
        ldap3::drive!(connection);
        ldap.simple_bind(&self.bind_dn, &self.bind_password)
            .await?
            .success()?;

        let mut account = self.find(&mut ldap, id).await?;
        password_verdict(ldap.simple_bind(&account.dn, password).await?.rc)?;

        // The directory's spelling of the identifier, whatever case was typed
        let id = take_first(&mut account, &self.user_attribute).ok_or(SignInError::Unavailable)?;
        Ok(Identity {
            id,
            name: take_first(&mut account, NAME),
            email: take_first(&mut account, EMAIL),
        })
    }

    async fn find(&self, ldap: &mut Ldap, id: &str) -> Result<SearchEntry, SignInError> {
        let filter = format!("({}={})", self.user_attribute, ldap_escape(id));
        let attributes = [self.user_attribute.as_str(), NAME, EMAIL];
        let (entries, _) = ldap
            .search(&self.base_dn, Scope::Subtree, &filter, attributes)
            .await?
            .success()?;
        // No account or several: refused like a wrong password
        let [entry]: [ResultEntry; 1] = entries.try_into().map_err(|_| SignInError::Refused)?;
        Ok(SearchEntry::construct(entry))
    }
}

// A password must never cross the network in clear (ADR-0017)
fn ldaps(url: String) -> Result<String, String> {
    if url.starts_with("ldaps://") {
        Ok(url)
    } else {
        Err(format!("LDAP_URL must start with `ldaps://`, not {url}"))
    }
}

fn pinned(ca_file: &str) -> Result<Arc<ClientConfig>, Box<dyn Error>> {
    let mut roots = RootCertStore::empty();
    let certificates =
        CertificateDer::pem_file_iter(ca_file).map_err(|error| format!("{ca_file}: {error}"))?;
    for certificate in certificates {
        roots.add(certificate?)?;
    }
    if roots.is_empty() {
        return Err(format!("{ca_file}: no certificate").into());
    }

    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    Ok(Arc::new(config))
}

fn password_verdict(rc: u32) -> Result<(), SignInError> {
    match rc {
        SUCCESS => Ok(()),
        INVALID_CREDENTIALS => Err(SignInError::Refused),
        _ => Err(SignInError::Unavailable),
    }
}

// LDAP attributes are multi-valued: the first value is kept
fn take_first(entry: &mut SearchEntry, attribute: &str) -> Option<String> {
    entry.attrs.remove(attribute)?.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORD: &str = "log791-dev";

    fn local() -> Directory {
        Directory::from_env().unwrap()
    }

    // A CA that signed nothing the tests reach
    fn unrelated_ca() -> String {
        format!("{}/fixtures/unrelated-ca.pem", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn right_password_passes() {
        assert_eq!(password_verdict(SUCCESS), Ok(()));
    }

    #[test]
    fn credentials_code_is_refused() {
        assert_eq!(
            password_verdict(INVALID_CREDENTIALS),
            Err(SignInError::Refused)
        );
    }

    #[test]
    fn other_bind_failure_is_unavailable() {
        let unwilling_to_perform = 53;
        assert_eq!(
            password_verdict(unwilling_to_perform),
            Err(SignInError::Unavailable)
        );
    }

    #[test]
    fn ldaps_url_is_kept() {
        let url = "ldaps://ldap:6360".to_string();
        assert_eq!(ldaps(url.clone()), Ok(url));
    }

    #[test]
    fn ldap_url_is_refused() {
        assert!(ldaps("ldap://ldap:3890".to_string()).is_err());
    }

    #[test]
    fn ca_file_is_read() {
        assert!(pinned(&unrelated_ca()).is_ok());
    }

    #[test]
    fn file_without_ca_is_refused() {
        let manifest = format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"));
        assert!(pinned(&manifest).is_err());
    }

    #[test]
    fn missing_ca_file_is_refused() {
        assert!(pinned("/nonexistent/ca.pem").is_err());
    }

    #[tokio::test]
    async fn empty_password_never_binds() {
        assert_eq!(
            Directory::unreachable().sign_in("etudiant1", "").await,
            Err(SignInError::Refused)
        );
    }

    #[tokio::test]
    async fn unreachable_directory_is_unavailable() {
        assert_eq!(
            Directory::unreachable()
                .sign_in("etudiant1", PASSWORD)
                .await,
            Err(SignInError::Unavailable)
        );
    }

    #[tokio::test]
    #[ignore = "needs LDAP"]
    async fn right_password_gives_the_identity() {
        assert_eq!(
            local().sign_in("etudiant1", PASSWORD).await,
            Ok(Identity {
                id: "etudiant1".to_string(),
                name: Some("Céline Dion".to_string()),
                email: Some("etudiant1@log791.ca".to_string()),
            })
        );
    }

    #[tokio::test]
    #[ignore = "needs LDAP"]
    async fn wrong_password_is_refused() {
        assert_eq!(
            local().sign_in("etudiant1", "wrong").await,
            Err(SignInError::Refused)
        );
    }

    #[tokio::test]
    #[ignore = "needs LDAP"]
    async fn unknown_account_is_refused() {
        assert_eq!(
            local().sign_in("inconnu", PASSWORD).await,
            Err(SignInError::Refused)
        );
    }

    #[tokio::test]
    #[ignore = "needs LDAP"]
    async fn wildcard_matches_no_account() {
        assert_eq!(
            local().sign_in("etudiant1*", PASSWORD).await,
            Err(SignInError::Refused)
        );
    }

    #[tokio::test]
    #[ignore = "needs LDAP"]
    async fn other_ca_is_unavailable() {
        let directory = Directory {
            tls: pinned(&unrelated_ca()).unwrap(),
            ..local()
        };
        assert_eq!(
            directory.sign_in("etudiant1", PASSWORD).await,
            Err(SignInError::Unavailable)
        );
    }
}
