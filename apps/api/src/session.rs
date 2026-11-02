//! Sessions opened by a sign-in, stored in PortgreSQL (ADR-0017)

use std::error::Error;

use sqlx::PgPool;

// The token is 128 bits from the system's CSPRNG, written as 32 hexadecimal digits
pub(super) async fn open(pool: &PgPool, account: &str) -> Result<String, Box<dyn Error>> {
    let mut random = [0; 16];
    getrandom::fill(&mut random)?;
    let token = format!("{:032x}", u128::from_ne_bytes(random));

    // Only the token's hash is stored: reading the table, or a backup of it, opens no session
    sqlx::query!(
        "INSERT INTO sessions (token_hash, account) VALUES (sha256($1), $2)",
        token.as_bytes(),
        account
    )
    .execute(pool)
    .await?;
    Ok(token)
}

// Becomes the session check of every route; tests only until then
#[cfg(test)]
pub(super) async fn account(pool: &PgPool, token: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT account FROM sessions WHERE token_hash = sha256($1)")
        .bind(token.as_bytes())
        .fetch_optional(pool)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PortgreSQL"]
    async fn session_is_found_by_its_token(pool: PgPool) {
        let token = open(&pool, "etudiant1").await.unwrap();
        let account = account(&pool, &token).await.unwrap();
        assert_eq!(account.as_deref(), Some("etudiant1"));
    }
    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PortgreSQL"]
    async fn unknown_token_has_no_session(pool: PgPool) {
        open(&pool, "etudiant1").await.unwrap();
        let account = account(&pool, &"0".repeat(32)).await.unwrap();
        assert_eq!(account, None);
    }

    #[sqlx::test(migrations = "../../db/migrations")]
    #[ignore = "needs PortgreSQL"]
    async fn each_session_has_its_own_token(pool: PgPool) {
        let first = open(&pool, "etudiant1").await.unwrap();
        let second = open(&pool, "etudiant1").await.unwrap();
        assert_ne!(first, second);
        assert_eq!(first.len(), 32);
    }
}
