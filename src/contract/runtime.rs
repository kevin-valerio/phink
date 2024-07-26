use crate::contract::remote::BalanceOf;
use frame_support::{
    construct_runtime,
    derive_impl,
    parameter_types,
    traits,
    traits::{
        ConstU16,
        ConstU32,
    },
    weights::{
        constants::RocksDbWeight,
        ConstantMultiplier,
        IdentityFee,
    },
};
use frame_system::EnsureSigned;
pub use pallet_transaction_payment::{
    CurrencyAdapter,
    Multiplier,
    TargetedFeeAdjustment,
};
use sp_core::ConstBool;
use sp_runtime::{
    generic,
    testing::H256,
    traits::{
        BlakeTwo256,
        Bounded,
        IdentifyAccount,
        IdentityLookup,
        Verify,
    },
    FixedPointNumber,
    MultiSignature,
    Perbill,
    Perquintill,
};

pub type BlockNumber = u32;

pub type Signature = MultiSignature;

pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

pub type Balance = u128;

pub type Moment = u64;

pub type Nonce = u32;

pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

pub type SignedExtra = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);

pub type Address = sp_runtime::MultiAddress<AccountId, ()>;

pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;

pub type Block = generic::Block<Header, UncheckedExtrinsic>;

pub const MILLISECS_PER_BLOCK: Moment = 3000;
pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;
pub const MILLICENTS: Balance = 1_000_000_000;

impl pallet_insecure_randomness_collective_flip::Config for Runtime {}
parameter_types! {
    pub static DepositPerByte: BalanceOf<Runtime> = 1;
    pub const DepositPerItem: BalanceOf<Runtime> = 2;
    pub static DefaultDepositLimit: BalanceOf<Runtime> = 10_000_000;
    pub const MaxDelegateDependencies: u32 = 32;
    pub const CodeHashLockupDepositPercent: Perbill = Perbill::from_percent(10);
    pub Schedule: pallet_contracts::Schedule<Runtime> = Default::default();
    pub const MinimumPeriod: Moment = SLOT_DURATION / 2;
        pub const TransactionByteFee: Balance = 10 * MILLICENTS;
    pub const OperationalFeeMultiplier: u8 = 5;
    pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
    pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(1, 100_000);
    pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
    pub MaximumMultiplier: Multiplier = Bounded::max_value();
         pub static ExistentialDeposit: u64 = 1;
    pub const MaxLocks: u32 = 50;
    pub const MaxReserves: u32 = 50;
    pub const BlockHashCount: BlockNumber = 100;

}
#[derive_impl(frame_system::config_preludes::SolochainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
    type BaseCallFilter = traits::Everything;
    type BlockWeights = ();
    type BlockLength = ();
    type DbWeight = RocksDbWeight;
    type Nonce = Nonce;
    type Hash = H256;
    type Hashing = BlakeTwo256;

    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Block = Block;
    type BlockHashCount = BlockHashCount;
    type Version = ();
    type AccountData = pallet_balances::AccountData<Balance>;
    type SystemWeightInfo = frame_system::weights::SubstrateWeight<Runtime>;
    type SS58Prefix = ConstU16<42>;
    type MaxConsumers = ConstU32<16>;
}

impl pallet_balances::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type WeightInfo = pallet_balances::weights::SubstrateWeight<Runtime>;
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = frame_system::Pallet<Runtime>;
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxLocks = MaxLocks;
    type MaxReserves = MaxReserves;
    type MaxFreezes = ConstU32<1>;
}

impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = CurrencyAdapter<Balances, ()>;
    type WeightToFee = IdentityFee<Balance>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = TargetedFeeAdjustment<
        Self,
        TargetBlockFullness,
        AdjustmentVariable,
        MinimumMultiplier,
        MaximumMultiplier,
    >;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
}

impl pallet_timestamp::Config for Runtime {
    type Moment = Moment;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
    type WeightInfo = pallet_timestamp::weights::SubstrateWeight<Runtime>;
}

impl pallet_contracts::Config for Runtime {
    /// This must be `true` in order to get proper coverage feedback
    /// As a developper, feel free to change any `type` EXCEPT
    /// `UnsafeUnstableInterface`
    type UnsafeUnstableInterface = ConstBool<true>;
    type Time = Timestamp;
    type Randomness = Randomness;
    type Currency = Balances;
    type RuntimeEvent = RuntimeEvent;
    type MaxTransientStorageSize = ConstU32<{ 1 * 1024 * 1024 }>;
    type RuntimeCall = RuntimeCall;
    /// The safest default is to allow no calls at all.
    ///
    /// Runtimes should whitelist dispatchables that are allowed to be called
    /// from contracts and make sure they are stable. Dispatchables exposed
    /// to contracts are not allowed to change because that would break
    /// already deployed contracts. The `Call` structure itself is not
    /// allowed to change the indices of existing pallets, too.
    type CallFilter = frame_support::traits::Nothing;
    type WeightPrice = pallet_transaction_payment::Pallet<Self>;
    type WeightInfo = pallet_contracts::weights::SubstrateWeight<Self>;
    // type ChainExtension = LocalChainExtensions<Self, UnifiedAccounts, Xvm>;
    type ChainExtension = ();
    type Schedule = Schedule;
    type CallStack = [pallet_contracts::Frame<Self>; 5];
    type DepositPerByte = DepositPerByte;
    type DefaultDepositLimit = DefaultDepositLimit;
    type DepositPerItem = DepositPerItem;
    type CodeHashLockupDepositPercent = CodeHashLockupDepositPercent;
    type AddressGenerator = pallet_contracts::DefaultAddressGenerator;
    type MaxCodeLen = ConstU32<{ 123 * 1024 }>;
    type MaxStorageKeyLen = ConstU32<128>;
    type MaxDelegateDependencies = MaxDelegateDependencies;
    type MaxDebugBufferLen = ConstU32<{ 2 * 1024 * 1024 }>;
    type UploadOrigin = EnsureSigned<Self::AccountId>;
    type InstantiateOrigin = EnsureSigned<Self::AccountId>;
    type RuntimeHoldReason = RuntimeHoldReason;
    type Migrations = ();
    type Debug = ();
    type Environment = ();
    // pallet_contracts::Environment<Self>
    type ApiVersion = ();
    type Xcm = ();
}

construct_runtime!(
    pub enum Runtime {
        System: frame_system,
        Timestamp: pallet_timestamp,
        Balances: pallet_balances,
        TransactionPayment: pallet_transaction_payment,
        Randomness: pallet_insecure_randomness_collective_flip,
        Contracts: pallet_contracts
    }
);
