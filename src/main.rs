#![recursion_limit = "1024"]

extern crate core;

use std::{
    env::var,
    path::PathBuf,
};

use clap::Parser;

use crate::{
    cli::{
        config::Configuration,
        ziggy::ZiggyConfig,
    },
    cover::report::CoverageTracker,
    fuzzer::fuzz::{
        Fuzzer,
        FuzzingMode::{
            ExecuteOneInput,
            Fuzz,
        },
    },
    instrumenter::{
        cleaner::Cleaner,
        instrumentation::{
            ContractBuilder,
            ContractInstrumenter,
            Instrumenter,
        },
    },
};

mod cli;
mod contract;
mod cover;
mod fuzzer;
mod instrumenter;

/// This struct defines the command line arguments expected by Phink.
#[derive(Parser, Debug)]
#[clap(
    author,
    version,
    about = "🐙 Phink: An ink! smart-contract property-based and coverage-guided fuzzer",
    long_about = None
)]
struct Cli {
    /// Order to execute (if you start here, instrument then fuzz suggested) 🚀
    #[clap(subcommand)]
    command: Commands,

    /// Path to the Phink configuration file.
    #[clap(long, short, value_parser, default_value = "phink.toml")]
    config: PathBuf,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Starts the fuzzing process. Instrumentation required before!
    Fuzz(Contract),
    /// Instrument the ink! contract, and compile it with Phink features
    Instrument(Contract),
    /// Run all the seeds
    Run(Contract),
    /// Remove all the temporary files under /tmp/ink_fuzzed_*
    Clean,
    /// Generate a coverage report, only of the harness. You won't have your
    /// contract coverage here (mainly for debugging purposes only)
    HarnessCover(Contract),
    /// Generate a coverage report for your smart-contract
    Coverage(Contract),
    /// Execute one seed
    Execute {
        /// Seed to be run
        seed: PathBuf,
        /// Path where the contract is located. It must be the root directory
        /// of the contract
        contract_path: PathBuf,
    },
}

#[derive(clap::Args, Debug)]
struct Contract {
    /// Path where the contract is located. It must be the root directory of
    /// the contract
    #[clap(value_parser)]
    contract_path: PathBuf,
}

fn main() {
    // We execute `handle_cli()` first, then re-enter into `main()`
    if let Ok(config_str) = var("PHINK_START_FUZZING_WITH_CONFIG") {
        let config = ZiggyConfig::parse(config_str.clone());
        Fuzzer::execute_harness(Fuzz, config).unwrap();
    } else {
        handle_cli();
    }
}

fn handle_cli() {
    let cli = Cli::parse();
    let config = Configuration::load_config(&cli.config);

    match cli.command {
        Commands::Instrument(contract_path) => {
            let mut engine = Instrumenter::new(contract_path.contract_path.clone());
            engine.instrument().unwrap().build().unwrap();

            println!(
                "🤞 Contract {} has been instrumented and compiled!",
                contract_path.contract_path.display()
            );
        }
        Commands::Fuzz(contract_path) => {
            ZiggyConfig::new(config, contract_path.contract_path)
                .ziggy_fuzz()
                .unwrap();
        }
        Commands::Run(contract_path) => {
            ZiggyConfig::new(config, contract_path.contract_path)
                .ziggy_run()
                .unwrap();
        }
        Commands::Execute {
            seed,
            contract_path,
        } => {
            Fuzzer::execute_harness(
                ExecuteOneInput(seed),
                ZiggyConfig::new(config, contract_path),
            )
            .unwrap();
        }
        Commands::HarnessCover(contract_path) => {
            ZiggyConfig::new(config, contract_path.contract_path)
                .ziggy_cover()
                .unwrap();
        }
        Commands::Coverage(contract_path) => {
            CoverageTracker::generate(ZiggyConfig::new(
                config,
                contract_path.contract_path,
            ));
        }
        Commands::Clean => {
            Instrumenter::clean().unwrap();
        }
    }
}
