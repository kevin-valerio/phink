#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod dns {
    use ink::prelude::{vec, vec::Vec};
    use ink::storage::Mapping;

    use ink::storage::StorageVec;
    /// Emitted whenever a new name is being registered.
    #[ink(event)]
    pub struct Register {
        #[ink(topic)]
        name: Hash,
        #[ink(topic)]
        from: AccountId,
    }

    /// Emitted whenever an address changes.
    #[ink(event)]
    pub struct SetAddress {
        #[ink(topic)]
        name: Hash,
        from: AccountId,
        #[ink(topic)]
        old_address: Option<AccountId>,
        #[ink(topic)]
        new_address: AccountId,
    }

    /// Emitted whenever a name is being transferred.
    #[ink(event)]
    pub struct Transfer {
        #[ink(topic)]
        name: Hash,
        from: AccountId,
        #[ink(topic)]
        old_owner: Option<AccountId>,
        #[ink(topic)]
        new_owner: AccountId,
    }

    const FORBIDDEN_DOMAIN: [u8; 32] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 1,
    ]; //we forbid it :/

    #[ink(storage)]
    pub struct DomainNameService {
        /// A hashmap to store all name to addresses mapping.
        name_to_address: Mapping<Hash, AccountId>,
        /// A hashmap to store all name to owners mapping.
        name_to_owner: Mapping<Hash, AccountId>,
        /// The default address.
        default_address: AccountId,
        /// Simple storage vec that contains every registered domain
        domains: StorageVec<Hash>,
        /// Another invariant testing
        dangerous_number: i32,
    }

    impl Default for DomainNameService {
        fn default() -> Self {
            let mut name_to_address = Mapping::new();
            name_to_address.insert(Hash::default(), &zero_address());
            let mut name_to_owner = Mapping::new();
            name_to_owner.insert(Hash::default(), &zero_address());
            let mut domains = StorageVec::new();
            domains.push(&Hash::default());
            Self {
                name_to_address,
                name_to_owner,
                default_address: zero_address(),
                domains,
                dangerous_number: 42_i32,
            }
        }
    }

    /// Errors that can occur upon calling this contract.
    #[derive(Debug, PartialEq, Eq)]
    #[ink::scale_derive(Encode, Decode, TypeInfo)]
    pub enum Error {
        /// Returned if the name already exists upon registration.
        NameAlreadyExists,
        /// Returned if caller is not owner while required to.
        CallerIsNotOwner,
        /// Forbidden domain, we can't register that one... except if ?
        ForbiddenDomain,
    }

    /// Type alias for the contract's result type.
    pub type Result<T> = core::result::Result<T, Error>;

    impl DomainNameService {
        /// Creates a new domain name service contract.
        #[ink(constructor)]
        pub fn new() -> Self {
            Default::default()
        }

        /// Register specific name with caller as owner.
        #[ink(message)]
        pub fn register(&mut self, name: Hash) -> Result<()> {
            let caller = self.env().caller();

            if self.name_to_owner.contains(name) {
                return Err(Error::NameAlreadyExists);
            }

            // We effectively check that we can't register the forbidden domain
            if name.clone().as_mut() == FORBIDDEN_DOMAIN {
                return Err(Error::ForbiddenDomain);
            }

            self.name_to_owner.insert(name, &caller);
            self.env().emit_event(Register { name, from: caller });
            self.domains.push(&name);

            Ok(())
        }

        /// Set address for specific name.
        #[ink(message)]
        pub fn set_address(&mut self, name: Hash, new_address: AccountId) -> Result<()> {
            let caller = self.env().caller();

            //Random code for coverage purposes below
            let a = 1;
            let b = 3;
            assert_eq!(a, b - 2);

            let owner = self.get_owner_or_default(name);
            if caller != owner {
                return Err(Error::CallerIsNotOwner);
            }

            let old_address = self.name_to_address.get(name);
            self.name_to_address.insert(name, &new_address);

            self.env().emit_event(SetAddress {
                name,
                from: caller,
                old_address,
                new_address,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn crash(&mut self, data: Vec<u8>) -> crate::dns::Result<()> {
            if data.len() < 5 {
                if data[0] == b'a' {
                    if data[1] == b'b' {
                        if data[2] == b'c' {
                            if data[3] == b'd' {
                                self.dangerous_number = 69; //panic!
                            }
                        }
                    }
                }
            }

            Ok(())
        }
        /// Transfer owner to another address.
        /// Don't tell anyone, but this contract is vulnerable!
        /// A user can push FORBIDDEN_DOMAIN, as the developer forgot to handle `Error::ForbiddenDomain`
        #[ink(message)]
        pub fn transfer(&mut self, name: Hash, to: AccountId, number: i32) -> Result<()> {
            let caller = self.env().caller();
            // Let's assume we still transfer if the caller isn't the owner

            // let owner = self.get_owner_or_default(name);
            // if caller != owner {
            //     return Err(Error::CallerIsNotOwner);
            // }2

            let old_owner = self.name_to_owner.get(name);
            self.name_to_owner.insert(name, &to);

            self.dangerous_number = number;
            self.domains.push(&name);

            self.env().emit_event(Transfer {
                name,
                from: caller,
                old_owner,
                new_owner: to,
            });

            Ok(())
        }

        /// Get address for specific name.
        #[ink(message)]
        pub fn get_address(&self, name: Hash) -> AccountId {
            self.get_address_or_default(name)
        }

        /// Get owner of specific name.
        #[ink(message)]
        pub fn get_owner(&self, name: Hash) -> AccountId {
            self.get_owner_or_default(name)
        }

        /// Returns the owner given the hash or the default address.
        fn get_owner_or_default(&self, name: Hash) -> AccountId {
            self.name_to_owner.get(name).unwrap_or(self.default_address)
        }

        /// Returns the address given the hash or the default address.
        fn get_address_or_default(&self, name: Hash) -> AccountId {
            self.name_to_address
                .get(name)
                .unwrap_or(self.default_address)
        }
    }

    fn zero_address() -> AccountId {
        [0u8; 32].into()
    }

    #[cfg(feature = "phink")]
    #[ink(impl)]
    impl DomainNameService {
        // This invariant ensures that `domains` doesn't contain the forbidden domain that nobody should regsiter

        #[cfg(feature = "phink")]
        #[ink(message)]
        pub fn phink_assert_hash42_cant_be_registered(&self) {
            for i in 0..self.domains.len() {
                if let Some(domain) = self.domains.get(i) {
                    // Invariant triggered! We caught an invalid domain in the storage...
                    assert_ne!(domain.clone().as_mut(), FORBIDDEN_DOMAIN);
                }
            }
        }

        // This invariant ensures that nobody registed the forbidden number
        #[cfg(feature = "phink")]
        #[ink(message)]
        pub fn phink_assert_dangerous_number(&self) {
            let FORBIDDEN_NUMBER = 69;
            assert_ne!(self.dangerous_number, FORBIDDEN_NUMBER);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn default_accounts() -> ink::env::test::DefaultAccounts<ink::env::DefaultEnvironment> {
            ink::env::test::default_accounts::<Environment>()
        }

        fn set_next_caller(caller: AccountId) {
            ink::env::test::set_caller::<Environment>(caller);
        }

        #[ink::test]
        fn register_works() {
            let default_accountxs: ink::env::test::DefaultAccounts<ink::env::DefaultEnvironment> =
                default_accounts();
            let hex_str = "7c00000101000e00a3e7e7e7e7e7e7e7e7e79f959596800000957d9580010101";

            // Convert hex string to byte array
            let bytes: [u8; 32] = hex::decode(hex_str)
                .expect("Decoding failed")
                .try_into()
                .expect("Invalid length");

            let name = Hash::from(bytes);

            set_next_caller(default_accounts.alice);
            let mut contract = DomainNameService::new();
            let x = contract.register(name);
            assert_eq!(x, Ok(()));
            assert_eq!(contract.register(name), Err(Error::NameAlreadyExists));
        }

        #[ink::test]
        fn set_address_works() {
            let accounts = default_accounts();
            let name = Hash::from([0x99; 32]);

            set_next_caller(accounts.alice);

            let mut contract = DomainNameService::new();
            assert_eq!(contract.register(name), Ok(()));

            // Caller is not owner, `set_address` should fail.
            set_next_caller(accounts.bob);
            assert_eq!(
                contract.set_address(name, accounts.bob),
                Err(Error::CallerIsNotOwner)
            );

            // Caller is owner, set_address will be successful
            set_next_caller(accounts.alice);
            assert_eq!(contract.set_address(name, accounts.bob), Ok(()));
            assert_eq!(contract.get_address(name), accounts.bob);
            // contract.phink_assert_hash42_cant_be_registered();
        }

        #[ink::test]
        fn should_panic() {
            let accounts = default_accounts();
            set_next_caller(accounts.alice);
            let mut contract = DomainNameService::new();
            let illegal = Hash::from(FORBIDDEN_DOMAIN);
            println!("{:?}", illegal);
            assert_eq!(contract.transfer(illegal, accounts.bob, 44), Ok(()));
            // contract.phink_assert_hash42_cant_be_registered();
        }

        #[ink::test]
        fn transfer_works() {
            let accounts = default_accounts();
            let name = Hash::from([0x99; 32]);

            set_next_caller(accounts.alice);

            let mut contract = DomainNameService::new();
            assert_eq!(contract.register(name), Ok(()));
            // contract.phink_assert_hash42_cant_be_registered();

            let illegal = Hash::from(FORBIDDEN_DOMAIN);

            // Test transfer of owner.
            assert_eq!(contract.transfer(illegal, accounts.bob, 43), Ok(()));

            // This should panic..
            // contract.phink_assert_hash42_cant_be_registered();

            // Owner is bob, alice `set_address` should fail.
            assert_eq!(
                contract.set_address(name, accounts.bob),
                Err(Error::CallerIsNotOwner)
            );

            set_next_caller(accounts.bob);
            // Now owner is bob, `set_address` should be successful.
            assert_eq!(contract.set_address(name, accounts.bob), Ok(()));
            assert_eq!(contract.get_address(name), accounts.bob);
        }
    }
}
