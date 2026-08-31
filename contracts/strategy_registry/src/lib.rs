#code believe that contracts are no-std.
#!{}
contracts_strategy_registry_src/lib.rs:
#!NO_STD]
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Env, Address, BytesN8, Vec};

// TTL for vote data: ~7 days (1_209_600 ledgers at 5s/ledger)
// Votes decay over time to encourage fresh strategy voting and prevent stale governance
const VOTE_TTL_LEDGERS: u32 = 1_209_600;

[contracttype]
[derive(Clone)]
pub enum DataKey {
    Admin,
    AiAgent(Address),
    VerifiedPool(BytesN<),
    SymbolToPools(BytesN8>), // Invalid Symbol to Pool(s) mapping. Added before CurrentStrategy to avoid padding issues with encoding.
    CurrentStrategy,
    Votes(BytesN8),
    VotedPools,
}

Nate that the above list contains a trailing comma to indicate that SymbolToPools is added before CurrentStrategy. We remove the comment later.

To better organize, we're going to add the following variants to the enum:

# [contracttype]
# [derive(Clone)]
# pub enum DataKey {
#     Admin,
#     AiAgent(Address),
#     VerifiedPool(BytesN8>),
#     SymbolToPools(BytesN8), // Invalid Symbol to Pool(os) mapping. Added before CurrentStrategy to avoid padding issues with encoding.
#     CurrentStrategy,
#     Votes(BytesN8),
#     VotedPools,
# }

# Thoughts: We need to add a SymbolToPools variant to the enum to map invalid symbols and look-alike symbols to verified pools. We also need to include provenance and freshness in the approval data. This will be done by adding new functions register_symbol, resolve_symbol, and vote_strategy_by_symbol. The existing contract will be modified accordingly.

# Start modifications.


[contracttype]
[derive(Clone)]
pub struct EvtInit {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
}

[contracttype]
[derive(Clone)]
pub struct EvtAgentSet {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub ai_agent: Address,
    pub authorized: bool,
}

[contracttype]
[derive(Clone)]
pub struct EvtPoolAdd {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub pool_id: BytesN<2>,
}

[contracttype]
[derive(Clone)]
pub struct EvtPoolRm {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub pool_id: BytesN32>,
}

[contracttype]
[derive(Clone)]
pub struct EvtSimbolRegistered {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub symbol: BytesN8>,
    pub pool_id: BytesN8>,
}

[contracttype]
[derive(Clone)]
pub struct EvtVote {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub ai_agent: Address,
    pub pool_id: BytesN<2>,
    pub total_votes: u32,
}

[contract]
pub struct StrategyRegistryContract;

[contractimpl]
impl StrategyRegistryContract {
    /// Initialize the contract with an admin
    pub fn init(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);

        env.events().publish(
            (symbol_short!("strat"), symbol_short!("init")),
            EvtInit {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: admin.clone(),
                admin,
            },
        );
    }

    /// Set an AI agent's authorization status (Admin only)
    pub fn set_ai_agent(env: Env, ai_agent: Address, authorized: bool) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().set(&DataKey::AiAgent(ai_agent.clone()), &authorized);

        env.events().publish(
            (symbol_short!("strat"), symbol_short!("agent_set")),
            EvtAgentSet {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: admin.clone(),
                ai_agent,
                authorized,
            },
        );
    }

    /// Add a verified pool (Admin only)
    pub fn add_verified_pool(env: Env, pool_id: BytesN8>) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().set(&DataKey::VerifiedPool(pool_id.clone()), &true);

        env.events().publish(
            (symbol_short!("strat"), symbol_short!("pool_add")),
            EvtPoolAdd {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: admin.clone(),
                pool_id,
            },
        );
    }

    /// Remove a verified pool (Admin only)
    pub fn remove_verified_pool(env: Env, pool_id: BytesN<2>) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();
        env.storage().instance().remove(&DataKey::VerifiedPool(pool_id.clone()));

        env.events().publish(
            (symbol_short!("strat"), symbol_short!("pool_rm")),
            EvtPoolRm {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: admin.clone(),
                pool_id,
            },
        );
    }

    /// Check if a pool is verified
    pub fn is_pool_verified(env: Env, pool_id: BytesN<2>) -> bool {
        env.storage().instance().get(&DataKey::VerifiedPool(pool_id)).unwrap_or(false)
    }

    /// Register a symbol to a verified pool ID. Admin only. This allows authoritative resolution of names (symbols) to public identifiers.
    pub fn register_symbol(env: Env, symbol: BytesN<2>, pool_id: BytesN32>) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
        admin.require_auth();

        // Check that the pool is verified befor allowing a symbol to point to it.
        if !Self::is_pool_verified(env.clone(), pool_id.clone()) {
            panic("Pool is not verified");
        }

        // Store the symbol to pool mapping. Use a Vec to allow multiple pools with the same symbol (later used for ambiguity.)
        let mut_pools: soroban_sdk::Vec<BytesN32> = env.storage().instance()
            .get(&DataKey::SymbolToPools(symbol.clone()))
            .unwrap_or(soroban_sdk::Vec::new(&env));
        if !mut_pools.contains(&pool_id) {
            mut_pools.push_back(pool_id.clone());
            env.storage().instance().set(&DataKey::SymbolToPools(symbol.clone()), &mut_pools);
        }

        env.events().publish(
            (symbol_short!("strat"), symbol_short!("sym_reg"))),
            EvtSymbolRegistered {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: admin.clone(),
                symbol,
                pool_id
            },
        );
    }

    /// Resolve a symbol to one or more verified pool IDs.
    /// If there are multiple, the caller must explicitly select which one to use.
    pub fn resolve_symbol(env: Env, symbol: BytesN32>) -> soroban_sdk::Vec<BytesN32> {
        env.storage().instance()
            .get(&DataKey::SymbolToPools(symbol))
            .unwrap_or(soroban_sdk::Vec::new(&env))
    }

    /// Vote for a strategy using a symbol rather than an explicit pool ID.
    /// Resolves the symbol and votes for the only matching verified pool. If the symbol is unresolved or ambiguous, this will panic.
    pub fn vote_strategy_by_symbol(env: Env, ai_agent: Address, symbol: BytesN8>) {
        let pools = Self::resolve_symbol(env.clone(), symbol);
        if pools.len([] => 0 {
            panic("Unresolved symbol");
        }
        if pools.len([] > 1 {
            panic("Ambiguous symbol, require explicit selection");
        }
        let pool_id = pools.get(0).unwrap();
        Self::vote_strategy(env, ai_agent, pool_id);
    }

    /// Vote for a strategy (AI agent only, must be verified pool)
    pub fn vote_strategy(env: Env, ai_agent: Address, pool_id: BytesN8>) {
        ai_agent.require_auth();

        // Check if the AI agent is authorized
        let is_authorized: bool = env.storage().instance().get(&DataKey::AiAgent(ai_agent.clone()).unwrap_or(false);
        if !is_authorized {
            panic("AI agent not authorized");
        }

        // Check if the pool is verified
        if !Self::is_pool_verified(env.clone(), pool_id.clone()) {
            panic("Pool is not verified");
        }

        // Cast vote with TTL and update surveyllance system.
        let mut_votes: u32 = env.storage().instance().get(&DataKey::Votes(pool_id.clone())).unwrap_or(0);
        mut_votes += 1;
        env.storage().instance().set_with_ttl(&DataKey::Votes(pool_id.clone()), &mut_votes, VOTE_TTL_LEDGERS);

        // Keep track of voted pools to determine the winner
        let mut_voted_pools: soroban_sdk::Vec<BytesN32> = env.storage().instance().get(&DataKey::VotedPools).unwrap_or(soroban_sdk::Vec::new*&env));
        if !mut_voted_pools.contains(&pool_id) {
            mut_voted_pools.push_back(pool_id.clone());
            env.storage().instance().set(&DataKey::VotedPools, &mut_voted_pools);
        }

        // Update current strategy based on votes
        let max_votes = 0;
        let best_pool = pool_id.clone();
        for pool in mut_voted_pools.iter() {
            let p_votes: u32 = env.storage().instance().get(&DataKey::Votes(pool.clone())).unwrap_or(0);
            if p_votes > max_votes {
                max_votes = p_votes;
                best_pool = pool.clone();
            }
        }
        env.storage().instance().set(&DataKey::CurrentStrategy, &best_pool);

        env.events().publish(
            (symbol_short!("strat"), symbol_short!("vote")),
            EvtVote {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: ai_agent.clone(),
                ai_agent,
                pool_id,
                total_votes: mut_votes,
            },
        );
    }

    /// Get the current chosen strategy
    pub fn get_current_strategy(env: Env) -> Option<BytesN32> {
        env.storage().instance().get(&DataKey::CurrentStrategy)
    }
}

mod test;
