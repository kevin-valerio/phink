# 🐙 Phink  

**Phink** is a blazing-fast⚡, property-based, coverage-guided fuzzer for ink! smart contracts. It enables developers to embed inviolable properties into their smart contract testing workflows, equipping them with automatic tools to detect vulnerabilities and ensure contract reliability before deployment.

> ⚠️ This project is actively under development with new features and improvements being made regularly. Contributions and feedback are welcome!


## Install  
  
```bash
cargo install --force ziggy cargo-afl honggfuzz grcov cargo-contract --locked 
cargo afl config --build --plugins --verbose --force # don't use `--plugins` if you're on macOS
git clone https://github.com/kevin-valerio/phink
cd phink/
```

## Usage

```bash
cargo run -- instrumenter path/to/ink_contract
cargo run -- fuzz /tmp/ink_fuzzed_Bb9Zp # you can get this path by reading the output of the previous command
```
  
## Example  
#### Creating an invariant  
Below are some invariants created for the [dns](https://github.com/kevin-valerio/phink/blob/main/sample/dns/lib.rs) contract.


  ```rust
#[cfg(feature = "phink")]
#[ink(impl)]
impl DomainNameService {
    // This invariant ensures that `domains` doesn't contain the forbidden domain that nobody should regsiter 
    #[ink(message)]
    #[cfg(feature = "phink")]
    pub fn phink_assert_hash42_cant_be_registered(&self) {
        for i in 0..self.domains.len() {
            if let Some(domain) = self.domains.get(i) {
                // Invariant triggered! We caught an invalid domain in the storage...
                assert_ne!(domain.clone().as_mut(), FORBIDDEN_DOMAIN);
            }
        }
    }

    // This invariant ensures that nobody registed the forbidden number
    #[ink(message)]
    #[cfg(feature = "phink")]
    pub fn phink_assert_dangerous_number(&self) {
        let FORBIDDEN_NUMBER = 69;
        assert_ne!(self.dangerous_number, FORBIDDEN_NUMBER);
    }
}
```
#### Catching an invariant  

```bash
cargo run -- execute output/phink/crashes/1720191069751/id:000000,sig:06,src:000001,time:77,execs:2314,op:havoc,rep:4   /tmp/ink_fuzzed_XqUCn/
```
Below, the trace after executing the crash:
```
🚀 Now fuzzing `/tmp/ink_fuzzed_XqUCn/target/ink/transfer.json` (5H31F11yQUkqugbgC7ur4rT2WLKSkZKAZUfcmHkKoLkaRaZ4)!

🤯 An invariant got caught! Let's dive into it

🫵  This was caused by `phink_assert_cannot_transfer_1337`

🎉 Find below the trace that caused that invariant

🌱 Executing new seed

+---------+-------------------------------------------------------------------+
| Message | Details                                                           |
+---------+-------------------------------------------------------------------+
| pay_me  |  ⛽️ Gas required : Weight(ref_time: 591391866, proof_size: 28781) |
|         | 🔥 Gas consumed : Weight(ref_time: 582570121, proof_size: 12443)  |
|         | 💾 Storage deposit : StorageDeposit::Charge(0)                    |
|         | 💸 Message was payable, and 1809739 units were transferred        |
+---------+-------------------------------------------------------------------+
thread 'main' panicked at src/fuzzer/bug.rs:83:9:

Job is done! Please, don't matter the backtrace below/above 🫡
```
## Features and upcoming ideas  
  
 - [x] Integration of a custom runtime, using a generic one by default
 - [x] Invariants-based fuzzing
 - [x] Detection of incorrect arithmetic, reentrancy, and panic handlers
 - [x] Handling of ink! specific encoding and constructors
 - [x] Automatic contract instantiation
 - [x] Crafting multiple messages in a single transaction
 - [x] Visualization of ink! contract coverage
 - [x] Proper binary usage
 - [ ] Enabling multi-contract fuzzing and cross-contract interactions
 - [ ] Creation of default invariants common to every contract
 - [ ] Provision of a specified on-chain state
 - [ ] Implementation of a snapshot-based fuzzing approach
 - [ ] Development of a custom fuzzing dashboard (default options: Ziggy/AFL++/Honggfuzz dashboard)
 - [ ] Extraction of seeds and constants from the codebase (_research needed_)
 - [ ] Creation of LLM-based invariants using [rust-llama](https://github.com/mdrokz/rust-llama.cpp) (_research needed_) 
