use std::{
    fs,
    io::{
        self,
        Write,
    },
    path::{
        Path,
        PathBuf,
    },
    sync::Mutex,
};

use contract_transcode::ContractMessageTranscoder;
use frame_support::__private::BasicExternalities;
use sp_core::hexdisplay::AsBytesRef;

use crate::{
    cli::{
        config::Configuration,
        ziggy::ZiggyConfig,
    },
    contract::{
        payload::{
            PayloadCrafter,
            Selector,
        },
        remote::{
            ContractBridge,
            FullContractResponse,
        },
    },
    cover::coverage::InputCoverage,
    fuzzer::{
        bug::BugManager,
        engine::FuzzerEngine,
        fuzz::FuzzingMode::{
            ExecuteOneInput,
            Fuzz,
        },
        parser::{
            parse_input,
            OneInput,
        },
    },
    instrumenter::instrumentation::Instrumenter,
};

pub const CORPUS_DIR: &str = "./output/phink/corpus";
pub const DICT_FILE: &str = "./output/phink/selectors.dict";
pub const MAX_MESSAGES_PER_EXEC: usize = 4; // One execution contains maximum 4 messages.

pub enum FuzzingMode {
    ExecuteOneInput(PathBuf),
    Fuzz,
}

#[derive(Clone)]
pub struct Fuzzer {
    pub setup: ContractBridge,
    pub fuzzing_config: Configuration,
}

impl Fuzzer {
    pub fn new(setup: ContractBridge) -> Self {
        Self {
            setup,
            fuzzing_config: Default::default(),
        }
    }

    pub fn execute_harness(mode: FuzzingMode, config: ZiggyConfig) -> io::Result<()> {
        let finder = Instrumenter::new(config.contract_path).find().unwrap();
        let wasm = fs::read(&finder.wasm_path)?;
        let setup = ContractBridge::initialize_wasm(
            wasm,
            &finder.specs_path,
            config.config.clone(),
        );
        let mut fuzzer = Fuzzer::new(setup);

        match mode {
            Fuzz => {
                fuzzer.set_config(config.config);
                fuzzer.fuzz();
            }
            ExecuteOneInput(seed_path) => {
                fuzzer.exec_seed(seed_path);
            }
        }

        Ok(())
    }

    fn build_corpus_and_dict(selectors: &[Selector]) -> io::Result<()> {
        fs::create_dir_all(CORPUS_DIR)?;
        let mut dict_file = fs::File::create(DICT_FILE)?;

        write_dict_header(&mut dict_file)?;

        for (i, selector) in selectors.iter().enumerate() {
            write_corpus_file(i, selector)?;
            write_dict_entry(&mut dict_file, selector);
        }

        Ok(())
    }

    fn should_stop_now(bug_manager: &BugManager, decoded_msgs: &OneInput) -> bool {
        decoded_msgs.messages.is_empty()
            || decoded_msgs.messages.iter().any(|payload| {
                payload
                    .payload
                    .get(..4)
                    .and_then(|slice| slice.try_into().ok())
                    .map_or(false, |slice: &[u8; 4]| {
                        bug_manager.contains_selector(slice)
                    })
            })
    }

    fn set_config(&mut self, config: Configuration) {
        self.fuzzing_config = config;
    }
}

impl FuzzerEngine for Fuzzer {
    fn fuzz(self) {
        let (mut transcoder_loader, invariant_manager) = init_fuzzer(self.clone());

        ziggy::fuzz!(|data: &[u8]| {
            Self::harness(
                self.clone(),
                &mut transcoder_loader,
                &mut invariant_manager.clone(),
                data,
            );
        });
    }

    fn harness(
        client: Fuzzer,
        transcoder_loader: &mut Mutex<ContractMessageTranscoder>,
        bug_manager: &mut BugManager,
        input: &[u8],
    ) {
        let decoded_msgs: OneInput =
            parse_input(input, transcoder_loader, client.fuzzing_config.clone());

        if Self::should_stop_now(bug_manager, &decoded_msgs) {
            return;
        }

        let mut chain = BasicExternalities::new(client.setup.genesis.clone());
        chain.execute_with(|| <Fuzzer as FuzzerEngine>::timestamp(0));

        let mut coverage = InputCoverage::new();

        let all_msg_responses =
            execute_messages(&client.clone(), &decoded_msgs, &mut chain, &mut coverage);

        chain.execute_with(|| {
            check_invariants(
                bug_manager,
                &all_msg_responses,
                &decoded_msgs,
                transcoder_loader,
            )
        });

        // If we are not in fuzzing mode, we save the coverage
        // If you ever wish to have real-time coverage while fuzzing (and a lose
        // of performance) Simply comment out the following line :)
        // #[cfg(not(fuzzing))]
        // {
        println!("[🚧UPDATE] Adding to the coverage file...");
        coverage.save().expect("🙅 Cannot save the coverage");

        <Fuzzer as FuzzerEngine>::pretty_print(all_msg_responses, decoded_msgs);
        // }

        // We now fake the coverage
        coverage.redirect_coverage();
    }

    fn exec_seed(self, seed: PathBuf) {
        let (mut transcoder_loader, mut invariant_manager) = init_fuzzer(self.clone());
        let data = fs::read(seed).unwrap();
        Self::harness(
            self,
            &mut transcoder_loader,
            &mut invariant_manager,
            data.as_bytes_ref(),
        );
    }
}

fn init_fuzzer(fuzzer: Fuzzer) -> (Mutex<ContractMessageTranscoder>, BugManager) {
    let transcoder_loader = Mutex::new(
        ContractMessageTranscoder::load(Path::new(&fuzzer.setup.path_to_specs))
            .expect("🙅 Failed to load `ContractMessageTranscoder`"),
    );

    let specs = &fuzzer.setup.json_specs;
    let selectors = PayloadCrafter::extract_all(specs);
    let invariants = PayloadCrafter::extract_invariants(specs)
        .expect("🙅 No invariants found, check your contract");

    let selectors_without_invariants: Vec<Selector> = selectors
        .into_iter()
        .filter(|s| !invariants.contains(s))
        .collect();

    let invariant_manager =
        BugManager::from(invariants, fuzzer.setup.clone(), fuzzer.fuzzing_config);

    Fuzzer::build_corpus_and_dict(&selectors_without_invariants)
        .expect("🙅 Failed to create initial corpus");

    println!(
        "\n🚀  Now fuzzing `{}` ({})!\n",
        fuzzer.setup.path_to_specs.as_os_str().to_str().unwrap(),
        fuzzer.setup.contract_address
    );

    (transcoder_loader, invariant_manager)
}

fn write_dict_header(dict_file: &mut fs::File) -> io::Result<()> {
    writeln!(dict_file, "# Dictionary file for selectors")?;
    writeln!(
        dict_file,
        "# Lines starting with '#' and empty lines are ignored."
    )?;

    writeln!(dict_file, "delimiter=\"\x2A\x2A\x2A\x2A\x2A\x2A\x2A\x2A\"")
}

fn write_corpus_file(index: usize, selector: &Selector) -> io::Result<()> {
    let file_path = PathBuf::from(CORPUS_DIR).join(format!("selector_{}.bin", index));
    fs::write(file_path, selector)
}

fn write_dict_entry(dict_file: &mut fs::File, selector: &Selector) {
    use std::fmt::Write;
    let selector_string = selector.iter().fold(String::new(), |mut acc, b| {
        write!(&mut acc, "\\x{:02X}", b).unwrap();
        acc
    });
    writeln!(dict_file, "\"{}\"", selector_string)
        .expect("😅 Failed to write to dict_file");
}

fn execute_messages(
    client: &Fuzzer,
    decoded_msgs: &OneInput,
    chain: &mut BasicExternalities,
    coverage: &mut InputCoverage,
) -> Vec<FullContractResponse> {
    let mut all_msg_responses = Vec::new();

    chain.execute_with(|| {
        for message in &decoded_msgs.messages {
            let transfer_value = if message.is_payable {
                message.value_token
            } else {
                0
            };

            let result: FullContractResponse = client.setup.clone().call(
                &message.payload,
                decoded_msgs.origin.into(),
                transfer_value,
                client.fuzzing_config.clone(),
            );

            coverage.add_cov(&result.debug_message);
            all_msg_responses.push(result);
        }
    });

    all_msg_responses
}

fn check_invariants(
    bug_manager: &mut BugManager,
    all_msg_responses: &[FullContractResponse],
    decoded_msgs: &OneInput,
    transcoder_loader: &mut Mutex<ContractMessageTranscoder>,
) {
    all_msg_responses
        .iter()
        .filter(|response| bug_manager.is_contract_trapped(response))
        .for_each(|response| {
            bug_manager.display_trap(decoded_msgs.messages[0].clone(), response.clone());
        });

    if let Err(invariant_tested) = bug_manager.are_invariants_passing(decoded_msgs.origin)
    {
        bug_manager.display_invariant(
            all_msg_responses.to_vec(),
            decoded_msgs.clone(),
            invariant_tested,
            transcoder_loader,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_parse_input() {
        let metadata_path = Path::new("sample/dns/target/ink/dns.json");
        let transcoder = Mutex::new(
            ContractMessageTranscoder::load(metadata_path)
                .expect("Failed to load ContractMessageTranscoder"),
        );

        let encoded_bytes = hex::decode(
            "229b553f9400000000000000000027272727272727272700002727272727272727272727",
        )
        .expect("Failed to decode hex string");

        let hex = transcoder
            .lock()
            .unwrap()
            .decode_contract_message(&mut &encoded_bytes[..])
            .expect("Failed to decode contract message");

        println!("{:#?}", hex);

        let binding = transcoder.lock().unwrap();
        let messages = binding.metadata().spec().messages();
        println!("{:#?}", messages);
    }
}
