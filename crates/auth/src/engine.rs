//! Narrow policy wrapper around the mature WebAuthn verifier. Never accepts
//! client supplied ceremony state; exact origin and UV are mandatory.
use crate::{Identity, StoredCredential};
use anyhow::{Result, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use webauthn_rs_core::{WebauthnCore, proto::*};
pub enum CeremonyState {
    Register(RegistrationState),
    Login(AuthenticationState),
}
pub struct Engine {
    core: WebauthnCore,
    user: Vec<u8>,
    user_name: String,
}
impl Engine {
    pub fn new(identity: &Identity, rp_name: &str, user_name: &str, origin: &str) -> Result<Self> {
        let user = B64.decode(&identity.user_id)?;
        ensure!(
            (16..=64).contains(&user.len()),
            "User Handle must have 16–64 bytes"
        );
        Ok(Self {
            core: WebauthnCore::new_unsafe_experts_only(
                rp_name,
                &identity.rp_id,
                vec![url::Url::parse(origin)?],
                std::time::Duration::from_secs(300),
                Some(false),
                Some(false),
            ),
            user,
            user_name: user_name.into(),
        })
    }
    pub fn register_begin(
        &self,
        credentials: &[StoredCredential],
    ) -> Result<(serde_json::Value, CeremonyState)> {
        let creds: Vec<Credential> = credentials
            .iter()
            .map(|c| serde_json::from_value(c.data.clone()))
            .collect::<Result<_, _>>()?;
        let builder = self
            .core
            .new_challenge_register_builder(&self.user, &self.user_name, &self.user_name)?
            .user_verification_policy(UserVerificationPolicy::Required)
            .require_resident_key(true)
            .attestation(AttestationConveyancePreference::None)
            .exclude_credentials(Some(creds.into_iter().map(|c| c.cred_id).collect()));
        let (options, state) = self.core.generate_challenge_register(builder)?;
        Ok((
            serde_json::to_value(options)?,
            CeremonyState::Register(state),
        ))
    }
    pub fn register_finish(
        &self,
        response: serde_json::Value,
        state: CeremonyState,
    ) -> Result<StoredCredential> {
        let CeremonyState::Register(state) = state else {
            anyhow::bail!("wrong ceremony type")
        };
        let credential =
            self.core
                .register_credential(&serde_json::from_value(response)?, &state, None)?;
        ensure!(credential.user_verified, "UV required");
        Ok(StoredCredential {
            id: B64.encode(&credential.cred_id),
            name: "Passkey".into(),
            data: serde_json::to_value(credential)?,
            version: 1,
        })
    }
    pub fn login_begin(
        &self,
        credentials: &[StoredCredential],
    ) -> Result<(serde_json::Value, CeremonyState)> {
        ensure!(!credentials.is_empty(), "authentication unavailable");
        let creds = credentials
            .iter()
            .map(|c| serde_json::from_value(c.data.clone()))
            .collect::<Result<Vec<Credential>, _>>()?;
        let builder = self
            .core
            .new_challenge_authenticate_builder(creds, Some(UserVerificationPolicy::Required))?;
        let (options, state) = self.core.generate_challenge_authenticate(builder)?;
        Ok((serde_json::to_value(options)?, CeremonyState::Login(state)))
    }
    pub fn login_finish(
        &self,
        response: serde_json::Value,
        state: CeremonyState,
        credentials: &[StoredCredential],
    ) -> Result<StoredCredential> {
        let CeremonyState::Login(state) = state else {
            anyhow::bail!("wrong ceremony type")
        };
        let response: PublicKeyCredential = serde_json::from_value(response)?;
        if let Some(handle) = response.response.user_handle.as_ref() {
            ensure!(
                handle.as_ref() == self.user.as_slice(),
                "user handle mismatch"
            );
        }
        let result = self.core.authenticate_credential(&response, &state)?;
        ensure!(result.user_verified(), "UV required");
        let id = B64.encode(result.cred_id());
        let mut stored = credentials
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| anyhow::anyhow!("credential unavailable"))?
            .clone();
        let mut c: Credential = serde_json::from_value(stored.data)?;
        c.counter = c.counter.max(result.counter());
        c.backup_state = result.backup_state();
        c.backup_eligible = result.backup_eligible();
        stored.data = serde_json::to_value(c)?;
        Ok(stored)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_and_handle() {
        let identity = Identity {
            rp_id: "localhost".into(),
            user_id: crate::random(),
        };
        let engine = Engine::new(&identity, "Test", "owner", "http://localhost:8765").unwrap();
        let (o, _) = engine.register_begin(&[]).unwrap();
        assert_eq!(o["publicKey"]["user"]["id"], identity.user_id);
        assert_eq!(
            o["publicKey"]["authenticatorSelection"]["residentKey"],
            "required"
        );
        assert_eq!(
            o["publicKey"]["authenticatorSelection"]["userVerification"],
            "required"
        );
    }
}
