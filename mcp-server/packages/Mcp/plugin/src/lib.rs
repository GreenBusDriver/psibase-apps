#[allow(warnings)]
mod bindings;

use bindings::accounts::plugin::api as AccountsApi;
use bindings::auth_sig::plugin as AuthSig;
use bindings::exports::mcp::plugin::agent_sign::Guest as AgentSign;
use bindings::exports::mcp::plugin::approvals::Guest as Approvals;
use bindings::exports::mcp::plugin::setup::Guest as Setup;
use bindings::exports::transact_hook_user_auth::{Claim, Guest as HookUserAuth, Proof};
use bindings::host::accounts::active_app as HostActiveApp;
use bindings::host::accounts::api as HostAccountsApi;
use bindings::host::client::api as Client;
use bindings::host::http::api as Server;
use bindings::host::types::types::{Error, Pem};
use bindings::profiles::plugin::api as Profiles;
use bindings::profiles::plugin::types::Profile as PluginProfile;
use bindings::staged_tx::plugin::respondent as StagedTx;
use bindings::tokens::plugin::user_config as TokensUserConfig;
use bindings::transact::plugin::api::{add_action_to_transaction, set_propose_latch};
use mcp::action_structs::*;
use psibase::fracpack::Pack;
use psibase::services::staged_tx as staged_tx_svc;
use psibase::{AccountNumber, Action};

psibase::plugin_error! {
    pub ErrorType
    Unauthorized(app: String) => "Only the mcp app may call this plugin (caller: {app})",
    InvalidAccount(name: String) => "Invalid account name: {name}",
    QueryFailed(msg: String) => "query failed: {msg}",
    BadActions(msg: String) => "invalid actions JSON: {msg}",
    ImportFailed(msg: String) => "import-agent failed: {msg}",
    HexDecode(msg: String) => "hex decode failed: {msg}",
    NeedsHost(msg: String) => "{msg}",
}

use crate::trust::*;

psibase::define_trust! {
    descriptions {
        Low => "
            - Read your mcp setup status and agent list
        ",
        Medium => "
            - Create agent accounts (auth-sig)
            - Enable agent token auto-debit
        ",
        High => "
            - Switch your auth service to mcp
            - Sign as your mcp auth service (via auth-sig)
            - Propose staged transactions and profiles on your behalf
            - Approve or reject staged agent transactions
        ",
    }
    functions {
        None => [],
        Low => [current_user, has_signing_key, my_agents, needs_approval],
        Medium => [create_agent_account, confirm_agent_key, enable_agent_auto_debit],
        High => [
            bootstrap,
            approve,
            reject,
            enforce,
            approve_and_grant,
            propose_as,
            propose_set_profile,
            track,
            import_agent,
            import_enforcer,
            restore_parent,
        ],
    }
}

fn only_mcp_app() -> Result<(), Error> {
    let caller = Client::get_sender();
    if caller != "mcp" {
        return Err(ErrorType::Unauthorized(caller).into());
    }
    Ok(())
}

fn account(name: &str) -> Result<AccountNumber, Error> {
    AccountNumber::from_exact(name).map_err(|_| ErrorType::InvalidAccount(name.to_string()).into())
}

fn decode_hex(s: &str) -> Result<Vec<u8>, Error> {
    let s = s.trim().strip_prefix("0x").unwrap_or(s.trim());
    if s.len() % 2 != 0 {
        return Err(ErrorType::HexDecode("odd length".into()).into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|e| ErrorType::HexDecode(e.to_string()).into())
        })
        .collect()
}

fn host_import_hint(account: &str) -> Error {
    ErrorType::NeedsHost(format!(
        "Import account `{account}` (and its key) in Host first, connect it to mcp, then retry"
    ))
    .into()
}

#[derive(serde::Deserialize)]
struct ActionJson {
    service: String,
    method: String,
    #[serde(rename = "rawData")]
    raw_data: String,
}

struct McpPlugin;

impl Approvals for McpPlugin {
    fn needs_approval() -> Result<String, Error> {
        assert_authorized(FunctionName::needs_approval)?;
        only_mcp_app()?;
        let owner = HostAccountsApi::get_current_user()
            .ok_or_else(|| ErrorType::QueryFailed("not logged in".into()))?;
        account(&owner)?;
        let mut ids = Vec::new();
        let mut after = String::new();
        loop {
            let q = format!(
                r#"{{ getIdsForParty(party: "{owner}", first: 100{after}) {{ edges {{ node {{ id }} }} pageInfo {{ hasNextPage endCursor }} }} }}"#
            );
            let response = bindings::staged_tx::plugin::authorized::graphql(&q)?;
            let v: serde_json::Value = serde_json::from_str(&response)
                .map_err(|e| ErrorType::QueryFailed(e.to_string()))?;
            if v.get("errors").is_some() {
                return Err(ErrorType::QueryFailed(v.to_string()).into());
            }
            let page = &v["data"]["getIdsForParty"];
            if let Some(edges) = page["edges"].as_array() {
                for edge in edges {
                    if let Some(id) = edge["node"]["id"].as_u64() {
                        ids.push(id);
                    }
                }
            }
            if page["pageInfo"]["hasNextPage"].as_bool() != Some(true) {
                break;
            }
            after = format!(", after: {}", page["pageInfo"]["endCursor"]);
        }
        let query = format!(
            r#"query {{ needsApproval(ids: {}) {{
                pending {{ account id txid proposer actions {{ sender service method rawData }} }}
                missing inspection canEnforce
            }} }}"#,
            serde_json::to_string(&ids).unwrap()
        );
        let body = Server::post_graphql_get_json(&query)?;
        let v: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| ErrorType::QueryFailed(e.to_string()))?;
        if let Some(errs) = v.get("errors") {
            return Err(ErrorType::QueryFailed(errs.to_string()).into());
        }
        Ok(v["data"]["needsApproval"].to_string())
    }

    fn approve(account_name: String, id: u32) -> Result<(), Error> {
        assert_authorized(FunctionName::approve)?;
        only_mcp_app()?;
        let account = account(&account_name)?;
        StagedTx::accept(id)?;
        add_action_to_transaction(untrack::ACTION_NAME, &untrack { account, id }.packed())
    }

    fn approve_and_grant(account_name: String, id: u32, rules_json: String) -> Result<(), Error> {
        assert_authorized(FunctionName::approve_and_grant)?;
        only_mcp_app()?;
        let account = account(&account_name)?;
        #[derive(serde::Deserialize)]
        struct RuleInput {
            service: AccountNumber,
            method: psibase::MethodNumber,
            limits: Vec<mcp::tables::ParamLimit>,
        }
        let rules: Vec<RuleInput> =
            serde_json::from_str(&rules_json).map_err(|e| ErrorType::BadActions(e.to_string()))?;
        for rule in rules {
            add_action_to_transaction(
                set_limits::ACTION_NAME,
                &set_limits {
                    service: rule.service,
                    method: rule.method,
                    limits: rule.limits,
                }
                .packed(),
            )?;
        }
        add_action_to_transaction(
            grant_missing::ACTION_NAME,
            &grant_missing { account, id }.packed(),
        )?;
        StagedTx::accept(id)
    }

    fn reject(account_name: String, id: u32) -> Result<(), Error> {
        assert_authorized(FunctionName::reject)?;
        only_mcp_app()?;
        let account = account(&account_name)?;
        StagedTx::reject(id)?;
        add_action_to_transaction(untrack::ACTION_NAME, &untrack { account, id }.packed())
    }

    fn enforce(owner: String, id: u32) -> Result<(), Error> {
        assert_authorized(FunctionName::enforce)?;
        only_mcp_app()?;
        if HostAccountsApi::get_current_user().as_deref() != Some(&owner) {
            return Err(ErrorType::Unauthorized("watcher requires owner session".into()).into());
        }
        let rows: serde_json::Value = serde_json::from_str(&Self::needs_approval()?)
            .map_err(|e| ErrorType::QueryFailed(e.to_string()))?;
        let row = rows
            .as_array()
            .and_then(|rows| {
                rows.iter()
                    .find(|r| r["pending"]["id"].as_u64() == Some(id as u64))
            })
            .ok_or_else(|| ErrorType::QueryFailed("proposal no longer pending".into()))?;
        if row["canEnforce"].as_bool() != Some(true) {
            return Err(ErrorType::QueryFailed("owner review required".into()).into());
        }
        let cfg = Server::post_graphql_get_json(&format!(
            r#"{{ agent(account: "{owner}") {{ enforcer }} }}"#
        ))?;
        let cfg: serde_json::Value =
            serde_json::from_str(&cfg).map_err(|e| ErrorType::QueryFailed(e.to_string()))?;
        let enforcer = cfg["data"]["agent"]["enforcer"]
            .as_str()
            .ok_or_else(|| ErrorType::QueryFailed("no enforcer".into()))?;
        let txid = serde_json::from_value(row["pending"]["txid"].clone())
            .map_err(|e| ErrorType::QueryFailed(e.to_string()))?;
        HostActiveApp::login(enforcer).map_err(|_| host_import_hint(enforcer))?;
        add_action_to_transaction(
            enforce::ACTION_NAME,
            &enforce {
                account: account(&owner)?,
                id,
                txid,
            }
            .packed(),
        )?;
        StagedTx::accept(id)
    }
}

impl Setup for McpPlugin {
    fn create_agent_account(agent_name: String) -> Result<Pem, Error> {
        assert_authorized(FunctionName::create_agent_account)?;
        only_mcp_app()?;
        account(&agent_name)?;
        AuthSig::actions::create_account(&agent_name)
    }

    fn confirm_agent_key(agent_name: String, private_key: Pem) -> Result<bool, Error> {
        assert_authorized(FunctionName::confirm_agent_key)?;
        only_mcp_app()?;
        account(&agent_name)?;
        Ok(AuthSig::api::can_authorize(&private_key, &agent_name))
    }

    fn bootstrap(agent_name: String, enforcer_name: String) -> Result<(), Error> {
        assert_authorized(FunctionName::bootstrap)?;
        only_mcp_app()?;
        let agent = account(&agent_name)?;
        let enforcer = Some(account(&enforcer_name)?);
        HostAccountsApi::get_current_user()
            .ok_or_else(|| ErrorType::ImportFailed("not logged in".into()))?;
        add_action_to_transaction(
            bootstrap::ACTION_NAME,
            &bootstrap { agent, enforcer }.packed(),
        )?;
        AccountsApi::set_auth_service(&mcp::SERVICE.to_string())
    }

    fn import_enforcer(enforcer_name: String, private_key: Pem) -> Result<(), Error> {
        assert_authorized(FunctionName::import_enforcer)?;
        only_mcp_app()?;
        account(&enforcer_name)?;
        if !AuthSig::api::can_authorize(&private_key, &enforcer_name) {
            return Err(ErrorType::ImportFailed("enforcer key does not match".into()).into());
        }
        // Vault import is Host's job; verify only, then require Host import.
        Err(host_import_hint(&enforcer_name))
    }

    fn current_user() -> Result<String, Error> {
        assert_authorized(FunctionName::current_user)?;
        only_mcp_app()?;
        HostAccountsApi::get_current_user()
            .ok_or_else(|| ErrorType::ImportFailed("not logged in".into()).into())
    }

    fn has_signing_key() -> Result<bool, Error> {
        assert_authorized(FunctionName::has_signing_key)?;
        only_mcp_app()?;
        let owner = HostAccountsApi::get_current_user()
            .ok_or_else(|| ErrorType::ImportFailed("not logged in".into()))?;
        Ok(AuthSig::queries::get_pubkey(&owner)?.is_some())
    }

    fn enable_agent_auto_debit(agent_name: String, agent_key: Pem) -> Result<(), Error> {
        assert_authorized(FunctionName::enable_agent_auto_debit)?;
        only_mcp_app()?;
        account(&agent_name)?;
        if !AuthSig::api::can_authorize(&agent_key, &agent_name) {
            return Err(ErrorType::ImportFailed("agent key does not match".into()).into());
        }
        // Agent key must already be in Host vault and connected to mcp.
        HostActiveApp::login(&agent_name).map_err(|_| host_import_hint(&agent_name))?;
        TokensUserConfig::enable_user_auto_debit(true)?;
        Ok(())
    }

    fn restore_parent(parent: String) -> Result<(), Error> {
        assert_authorized(FunctionName::restore_parent)?;
        only_mcp_app()?;
        account(&parent)?;
        HostActiveApp::login(&parent)
    }

    fn my_agents() -> Result<String, Error> {
        assert_authorized(FunctionName::my_agents)?;
        only_mcp_app()?;
        let agents_body = Server::post_graphql_get_json(
            r#"query { myAgents { account agent enforcer } }"#,
        )?;
        let agents_v: serde_json::Value = serde_json::from_str(&agents_body)
            .map_err(|e| ErrorType::QueryFailed(e.to_string()))?;
        if let Some(errs) = agents_v.get("errors") {
            return Err(ErrorType::QueryFailed(errs.to_string()).into());
        }
        let history = match Server::post_graphql_get_json(
            r#"query { history { account seq kind actor service method stagedId atSec detail } }"#,
        ) {
            Ok(body) => {
                let v: serde_json::Value =
                    serde_json::from_str(&body).unwrap_or(serde_json::json!({}));
                if v.get("errors").is_some() {
                    serde_json::json!([])
                } else {
                    v["data"]["history"].clone()
                }
            }
            Err(_) => serde_json::json!([]),
        };
        Ok(serde_json::json!({
            "agents": agents_v["data"]["myAgents"],
            "history": history,
        })
        .to_string())
    }
}

impl AgentSign for McpPlugin {
    fn import_agent(account_name: String, private_key: Pem) -> Result<(), Error> {
        assert_authorized(FunctionName::import_agent)?;
        only_mcp_app()?;
        account(&account_name)?;
        if !AuthSig::api::can_authorize(&private_key, &account_name) {
            return Err(ErrorType::ImportFailed("agent key does not match".into()).into());
        }
        HostActiveApp::login(&account_name).map_err(|_| host_import_hint(&account_name))?;
        Ok(())
    }

    fn propose_as(owner: String, actions_json: String) -> Result<u32, Error> {
        assert_authorized(FunctionName::propose_as)?;
        only_mcp_app()?;
        account(&owner)?;
        let parsed: Vec<ActionJson> = serde_json::from_str(&actions_json)
            .map_err(|e| ErrorType::BadActions(e.to_string()))?;
        if parsed.is_empty() {
            return Err(ErrorType::BadActions("empty actions".into()).into());
        }
        let mut actions = Vec::with_capacity(parsed.len());
        for a in parsed {
            account(&a.service)?;
            actions.push(Action {
                sender: AccountNumber::from_exact(&owner)
                    .map_err(|_| ErrorType::InvalidAccount(owner.clone()))?,
                service: AccountNumber::from_exact(&a.service)
                    .map_err(|_| ErrorType::InvalidAccount(a.service.clone()))?,
                method: a.method.as_str().into(),
                rawData: decode_hex(&a.raw_data)?.into(),
            });
        }
        // Ordinary staged-tx propose (no Max-gated transact::propose shortcut).
        let packed = staged_tx_svc::action_structs::propose {
            actions,
            auto_exec: true,
        }
        .packed();
        add_action_to_transaction(
            staged_tx_svc::action_structs::propose::ACTION_NAME,
            &packed,
        )?;
        Ok(0)
    }

    fn propose_set_profile(owner: String, display_name: String, bio: String) -> Result<(), Error> {
        assert_authorized(FunctionName::propose_set_profile)?;
        only_mcp_app()?;
        account(&owner)?;
        set_propose_latch(Some(&owner))?;
        let result = Profiles::set_profile(&PluginProfile {
            display_name: Some(display_name),
            bio: Some(bio),
        });
        set_propose_latch(None)?;
        result
    }

    fn track(id: u32) -> Result<(), Error> {
        assert_authorized(FunctionName::track)?;
        only_mcp_app()?;
        add_action_to_transaction(track::ACTION_NAME, &track { id }.packed())
    }
}

impl HookUserAuth for McpPlugin {
    fn on_user_auth_claim(account_name: String) -> Result<Option<Claim>, Error> {
        if Client::get_sender() != psibase::services::transact::SERVICE.to_string() {
            return Err(ErrorType::Unauthorized(Client::get_sender()).into());
        }
        let Some(pubkey) = AuthSig::queries::get_pubkey(&account_name)? else {
            return Err(
                ErrorType::QueryFailed(format!("{account_name} has no auth-sig key")).into(),
            );
        };
        Ok(Some(Claim {
            verify_service: psibase::services::verify_sig::SERVICE.to_string(),
            raw_data: AuthSig::keyvault::to_der(&pubkey)?,
        }))
    }

    fn on_user_auth_proof(
        account_name: String,
        transaction_hash: Vec<u8>,
    ) -> Result<Option<Proof>, Error> {
        if Client::get_sender() != psibase::services::transact::SERVICE.to_string() {
            return Err(ErrorType::Unauthorized(Client::get_sender()).into());
        }
        let Some(pubkey) = AuthSig::queries::get_pubkey(&account_name)? else {
            return Err(
                ErrorType::QueryFailed(format!("{account_name} has no auth-sig key")).into(),
            );
        };
        let der = AuthSig::keyvault::to_der(&pubkey)?;
        let signature = AuthSig::keyvault::sign(&transaction_hash, &der)?;
        Ok(Some(Proof { signature }))
    }
}

bindings::export!(McpPlugin with_types_in bindings);
