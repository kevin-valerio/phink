use std::{
    fs,
    path::{
        Path,
        PathBuf,
    },
};

use frame_support::{
    __private::BasicExternalities,
    pallet_prelude::Weight,
    traits::fungible::Inspect,
};
use migration::v13;
use pallet_contracts::{
    migration,
    Code,
    CollectEvents,
    Config,
    ContractResult,
    DebugInfo,
    Determinism,
    ExecReturnValue,
};
use sp_core::{
    crypto::AccountId32,
    storage::Storage,
    H256,
};
use sp_runtime::DispatchError;
use v13::ContractInfoOf;

use payload::PayloadCrafter;

use crate::{
    cli::config::Configuration,
    contract::{
        payload,
        runtime::{
            runtime_storage,
            AccountId,
            Contracts,
            Runtime,
        },
    },
};

pub type BalanceOf<T> =
    <<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

pub type AccountIdOf<T> = <T as frame_system::Config>::AccountId;
pub type EventRecord = frame_system::EventRecord<
    <Runtime as frame_system::Config>::RuntimeEvent,
    <Runtime as frame_system::Config>::Hash,
>;

pub type FullContractResponse =
    ContractResult<Result<ExecReturnValue, DispatchError>, u128, EventRecord>;

#[derive(Clone)]
pub struct ContractBridge {
    pub genesis: Storage,
    pub contract_address: AccountIdOf<Runtime>,
    pub json_specs: String,
    pub path_to_specs: PathBuf,
}

impl ContractBridge {
    pub const DEFAULT_GAS_LIMIT: Weight =
        Weight::from_parts(100_000_000_000, 3 * 1024 * 1024);
    pub const DEFAULT_DEPLOYER: AccountId32 = AccountId32::new([0u8; 32]);

    /// Create a proper genesis storage, deploy and instantiate a given ink!
    /// contract
    pub fn initialize_wasm(
        wasm_bytes: Vec<u8>,
        path_to_specs: &Path,
        config: Configuration,
    ) -> ContractBridge {
        let mut contract_addr: AccountIdOf<Runtime> = config
            .deployer_address
            .clone()
            .unwrap_or(ContractBridge::DEFAULT_DEPLOYER);

        println!(
            "🛠️Initializing contract address from the origin: {:?}",
            contract_addr
        );

        let json_specs = fs::read_to_string(path_to_specs).unwrap();
        let genesis_storage: Storage = {
            let storage = runtime_storage();
            let mut chain = BasicExternalities::new(storage.clone());
            chain.execute_with(|| {
                let code_hash = Self::upload(&wasm_bytes, contract_addr.clone());

                contract_addr = Self::instantiate(&json_specs, code_hash, contract_addr.clone(), config).expect(
                    "🙅 Can't fetch the contract address because because of incorrect instantiation",
                );

                // We verify if the contract is correctly instantiated
                if !ContractInfoOf::<Runtime>::contains_key(&contract_addr) {
                    panic!(
                        "🚨 Contract Instantiation Failed! 🚨
                            This error is likely due to a misconfigured constructor payload in the configuration file.
                            Please ensure the correct payload for the constructor (selector + parameters) is provided, just as you would for a regular deployment. You can use the `constructor_payload` field inside the TOML configuration file for this purpose.
                            To generate your payload, please use `cargo contract`:
                            Example:
                            ❯ cargo contract encode --message \"new\" --args 4444 123 \"0xe7109741c21967a67e4d4edaf7accb253a5e11455ff9e07bdd16ecb186c94be1\" \"0xe7109741c21967a67e4d4edaf7accb253a5e11455ff9e07bdd16ecb186c94be1\" \"0xe7109741c21967a67e4d4edaf7accb253a5e11455ff9e07bdd16ecb186c94be1\" -- target/ink/multi_contract_caller.json
                            Encoded data: 9BAE9D5E...4BE1"
                    );
                }
            });

            chain.into_storages()
        };

        Self {
            genesis: genesis_storage,
            contract_address: contract_addr,
            json_specs,
            path_to_specs: path_to_specs.to_path_buf(),
        }
    }

    /// Execute a function (`payload`) from the instantiated contract
    ///
    /// # Arguments
    ///
    /// * `payload`: The scale-encoded `data` to pass to the contract
    /// * `who`: AccountId of the caller
    /// * `amount`: Amount to pass to the contract
    pub fn call(
        self,
        payload: &[u8],
        who: u8,
        transfer_value: BalanceOf<Runtime>,
        config: Configuration,
    ) -> FullContractResponse {
        let acc = AccountId32::new([who; 32]);

        let storage_deposit_limit: Option<BalanceOf<Runtime>> =
            Configuration::parse_storage_deposit(&config);

        Contracts::bare_call(
            acc,
            self.contract_address,
            transfer_value,
            config.default_gas_limit.unwrap_or(Self::DEFAULT_GAS_LIMIT),
            storage_deposit_limit,
            payload.to_owned(),
            DebugInfo::UnsafeDebug,
            CollectEvents::UnsafeCollect,
            Determinism::Enforced,
        )
    }

    pub fn upload(wasm_bytes: &[u8], who: AccountId) -> H256 {
        println!("📤 Starting upload of WASM bytes by: {:?}", who);
        let upload_result = Contracts::bare_upload_code(
            who.clone(),
            wasm_bytes.to_owned(),
            None,
            Determinism::Enforced,
        );
        match upload_result {
            Ok(upload_info) => {
                println!(
                    "✅ Upload successful. Code hash: {:?}",
                    upload_info.code_hash
                );
                upload_info.code_hash
            }
            Err(e) => {
                panic!("❌ Upload failed for: {:?} with error: {:?}", who, e);
            }
        }
    }

    pub fn instantiate(
        json_specs: &str,
        code_hash: H256,
        who: AccountId,
        config: Configuration,
    ) -> Option<AccountIdOf<Runtime>> {
        let data: Vec<u8> = if let Some(payload) = config.constructor_payload {
            hex::decode(payload)
                .expect("Impossible to hex-decode this. Check your config file")
        } else {
            PayloadCrafter::get_constructor(json_specs)?.into()
        };

        let instantiate = Contracts::bare_instantiate(
            who.clone(),
            0,
            config.default_gas_limit.unwrap_or(Self::DEFAULT_GAS_LIMIT),
            None,
            Code::Existing(code_hash),
            data,
            vec![],
            DebugInfo::UnsafeDebug,
            CollectEvents::UnsafeCollect,
        );

        println!("🔍 Instantiated the contract, using account {:?}", who);

        Some(instantiate.result.unwrap().account_id)
    }
}
