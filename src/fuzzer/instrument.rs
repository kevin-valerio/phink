use regex::Regex;
use std::{
    ffi::OsStr,
    fs,
    fs::{copy, File},
    io,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use quote::quote;
use rand::distributions::Alphanumeric;
use rand::Rng;
use syn::{parse_file, visit_mut::VisitMut};
use walkdir::WalkDir;

use crate::fuzzer::instrument::instrument::ContractCovUpdater;

/// The objective of this `struct` is to assist Phink in instrumenting ink! smart contracts.
/// In a fuzzing context, instrumenting a smart contract involves modifying the target (i.e., the WASM blob),
/// for example, by adding additional code to branches to obtain a coverage map during the execution of the smart contract.
/// By doing so, we can effectively generate a coverage map that will be provided to Ziggy
/// or LibAFL, transforming Phink from a basic brute-forcing tool into a powerful coverage-guided fuzzer.
///
/// Phink opted for a Rust AST approach. For each code instruction on the smart-contract, Phink will
/// automatically add a tracing code, which will then be fetched at the end of the input execution
/// in order to get coverage.
#[derive(Default, Clone)]
pub struct InstrumenterEngine {
    pub contract_dir: PathBuf,
}

#[derive(Debug)]
pub struct InkFilesPath {
    pub wasm_path: PathBuf,
    pub specs_path: PathBuf,
}

pub trait ContractBuilder {
    fn build(&self) -> Result<InkFilesPath, String>;
}

pub trait ContractForker {
    fn fork(&self) -> Result<PathBuf, String>;
}

pub trait ContractInstrumenter {
    fn instrument(&mut self) -> Result<&mut Self, String>
    where
        Self: Sized;
    fn parse_and_visit(code: &str, visitor: impl VisitMut) -> Result<String, ()>;
    fn save_and_format(source_code: String, lib_rs: PathBuf) -> Result<(), io::Error>;
    fn already_instrumented(code: &str) -> bool;
}

impl InstrumenterEngine {
    pub fn new(dir: PathBuf) -> Self {
        Self { contract_dir: dir }
    }

    fn get_dirs_to_remove(tmp_dir: &Path, pattern: &str) -> Result<Vec<PathBuf>, io::Error> {
        Ok(fs::read_dir(tmp_dir)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.is_dir() && path.file_name()?.to_string_lossy().starts_with(pattern) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>())
    }

    fn prompt_user_confirmation() -> Result<bool, io::Error> {
        print!("🗑️ Do you really want to remove these directories? (yes/no): ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(input.trim().eq_ignore_ascii_case("yes"))
    }

    fn remove_directories(dirs_to_remove: Vec<PathBuf>) -> Result<(), io::Error> {
        for dir in dirs_to_remove {
            fs::remove_dir_all(&dir)?;
            println!("✅ Removed directory: {}", dir.display());
        }
        Ok(())
    }

    pub fn clean() -> Result<(), io::Error> {
        let pattern = "ink_fuzzed_";
        let dirs_to_remove = Self::get_dirs_to_remove(Path::new("/tmp"), pattern)?;

        if dirs_to_remove.is_empty() {
            println!("❌  No directories found matching the pattern '{}'. There's nothing to be cleaned :)", pattern);
            return Ok(());
        }

        println!("🔍 Found the following instrumented ink! contracts:");
        for dir in &dirs_to_remove {
            println!("{}", dir.display());
        }

        if Self::prompt_user_confirmation()? {
            Self::remove_directories(dirs_to_remove)?;
        } else {
            println!("❌ Operation cancelled.");
        }

        Ok(())
    }

    pub fn find(&self) -> Result<InkFilesPath, String> {
        let wasm_path = fs::read_dir(self.contract_dir.join("target/ink/"))
            .map_err(|e| {
                format!(
                    "🙅 It seems that your contract is not compiled into `target/ink`.\
             Please, ensure that your the WASM blob and the JSON specs are stored into \
             '{}/target/ink/' (more infos: {})",
                    self.contract_dir.to_str().unwrap(),
                    e
                )
            })?
            .filter_map(|entry| {
                let path = entry.ok()?.path();
                if path.is_file() && path.extension().and_then(OsStr::to_str) == Some("wasm") {
                    Some(path)
                } else {
                    None
                }
            })
            .next()
            .ok_or("🙅 No .wasm file found in target directory")?;

        let specs_path = PathBuf::from(wasm_path.to_str().unwrap().replace(".wasm", ".json"));

        Ok(InkFilesPath {
            wasm_path,
            specs_path,
        })
    }
}

impl ContractBuilder for InstrumenterEngine {
    fn build(&self) -> Result<InkFilesPath, String> {
        let status = Command::new("cargo")
            .current_dir(&self.contract_dir)
            .args(["contract", "build", "--features=phink"])
            .status()
            .map_err(|e| {
                format!(
                    "🙅 Failed to execute cargo command: {}.\
            The command was simply 'cargo contract build --features=phink",
                    e
                )
            })?;

        if status.success() {
            self.find()
        } else {
            Err(format!(
                "🙅 It seems that your instrumented smart contract did not compile properly. \
                Please go to {}, edit the `lib.rs` file, and run cargo contract build again.\
                (more infos: {})",
                &self.contract_dir.display(),
                status
            ))
        }
    }
}

impl ContractForker for InstrumenterEngine {
    fn fork(&self) -> Result<PathBuf, String> {
        let random_string: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(5)
            .map(char::from)
            .collect();

        let new_dir = Path::new("/tmp").join(format!("ink_fuzzed_{}", random_string));
        println!("🏗️ Creating new directory: {:?}", new_dir);
        fs::create_dir_all(&new_dir)
            .map_err(|e| format!("🙅 Failed to create directory: {}", e))?;

        println!("📁 Starting to copy files from {:?}", self.contract_dir);

        for entry in WalkDir::new(&self.contract_dir) {
            let entry = entry.map_err(|e| format!("🙅 Failed to read entry: {}", e))?;
            let target_path = new_dir.join(
                entry
                    .path()
                    .strip_prefix(&self.contract_dir)
                    .map_err(|e| format!("🙅 Failed to strip prefix: {}", e))?,
            );

            if entry.path().is_dir() {
                println!("📂 Creating subdirectory: {:?}", target_path);
                fs::create_dir_all(&target_path)
                    .map_err(|e| format!("🙅 Failed to create subdirectory: {}", e))?;
            } else {
                println!("📄 Copying file: {:?} -> {:?}", entry.path(), target_path);
                copy(entry.path(), &target_path)
                    .map_err(|e| format!("🙅 Failed to copy file: {}", e))?;
            }
        }

        println!(
            "✅ Fork completed successfully! New directory: {:?}",
            new_dir
        );
        Ok(new_dir)
    }
}

impl ContractInstrumenter for InstrumenterEngine {
    fn instrument(&mut self) -> Result<&mut InstrumenterEngine, String> {
        let new_working_dir = self.fork()?;
        let lib_rs = new_working_dir.join("lib.rs");
        let code = fs::read_to_string(&lib_rs)
            .map_err(|e| format!("🙅 Failed to read lib.rs: {:?}", e))?;

        Command::new("rustfmt").arg(lib_rs.clone());
        self.contract_dir = new_working_dir;

        if Self::already_instrumented(&code) {
            return Err("🙅 Code already instrumented".to_string());
        }

        let modified_code = Self::parse_and_visit(&code, ContractCovUpdater)
            .map_err(|_| "🙅 Failed to parse and visit code".to_string())?;

        Self::save_and_format(modified_code, lib_rs.clone())
            .map_err(|e| format!("🙅 Failed to save and format code: {:?}", e))?;

        Ok(self)
    }

    fn parse_and_visit(code: &str, mut visitor: impl VisitMut) -> Result<String, ()> {
        let mut ast = parse_file(code).expect(
            "⚠️ This is most likely that your ink! contract\
        contains invalid syntax. Try to compile it first. Also, ensure that `cargo-contract` is installed.",
        );
        visitor.visit_file_mut(&mut ast);
        Ok(quote!(#ast).to_string())
    }

    fn save_and_format(source_code: String, lib_rs: PathBuf) -> Result<(), io::Error> {
        let mut file = File::create(lib_rs.clone())?;
        file.write_all(source_code.as_bytes())?;
        file.flush()?;
        Command::new("rustfmt").arg(lib_rs).status()?;
        Ok(())
    }

    /// Checks if the given code string is already instrumented.
    /// This function looks for the presence of the pattern `ink::env::debug_println!("COV=abc")`
    /// where `abc` can be any number. If this pattern is found, it means the code is instrumented.
    fn already_instrumented(code: &str) -> bool {
        let re = Regex::new(r#"\bink::env::debug_println!\("COV=\d+"\)"#).unwrap();
        re.is_match(code)
    }
}

mod instrument {
    use proc_macro2::Span;
    use syn::{parse_quote, spanned::Spanned, visit_mut::VisitMut, Expr, LitInt, Stmt, Token};

    pub struct ContractCovUpdater;

    impl VisitMut for ContractCovUpdater {
        fn visit_block_mut(&mut self, block: &mut syn::Block) {
            let mut new_stmts = Vec::new();
            // Temporarily replace block.stmts with an empty Vec to avoid borrowing issues
            let mut stmts = std::mem::take(&mut block.stmts);
            for mut stmt in stmts.drain(..) {
                let line_lit =
                    LitInt::new(&stmt.span().start().line.to_string(), Span::call_site());
                let insert_expr: Expr = parse_quote! {
                    ink::env::debug_println!("COV={}", #line_lit)
                };
                // Convert this expression into a statement
                let pre_stmt: Stmt = Stmt::Expr(insert_expr, Some(Token![;](Span::call_site())));
                new_stmts.push(pre_stmt);
                // Use recursive visitation to handle nested blocks and other statement types
                self.visit_stmt_mut(&mut stmt);
                new_stmts.push(stmt.clone());
            }
            block.stmts = new_stmts;
        }
    }
}

#[cfg(test)]
mod test {
    use std::{fs, fs::File, io::Write, path::PathBuf, process::Command};

    use quote::quote;
    use syn::{__private::ToTokens, parse_file, visit_mut::VisitMut};

    use crate::fuzzer::instrument::{ContractForker, ContractInstrumenter, InstrumenterEngine};

    #[test]
    fn test_already_instrumented_true() {
        let code = String::from(
            r#"
            fn main() {
                ink::env::debug_println!("COV=123");
            }
        "#,
        );
        assert!(InstrumenterEngine::already_instrumented(&code));
    }

    #[test]
    fn test_already_instrumented_false() {
        let code = String::from(
            r#"
            fn main() {
                println!("Hello, world!");
            }
        "#,
        );
        assert!(!InstrumenterEngine::already_instrumented(&code));
    }

    #[test]
    fn test_already_instrumented_multiple_lines() {
        let code = String::from(
            r#"
            fn main() {
                println!("This is a test.");
                ink::env::debug_println!("COV=456");
                println!("Another line.");
            }
        "#,
        );
        assert!(InstrumenterEngine::already_instrumented(&code));
    }

    #[test]
    fn adding_cov_insertion_works() {
        let signature = "ink::env::debug_println!(\"COV =";
        let code = fs::read_to_string("sample/dns/lib.rs").unwrap();
        let mut ast = parse_file(&code).expect("Unable to parse file");

        let mut visitor = crate::fuzzer::instrument::instrument::ContractCovUpdater;
        visitor.visit_file_mut(&mut ast);

        let modified_code = quote!(#ast).to_string();
        assert!(modified_code.contains(signature)); //spaces are required :shrug:
        export(modified_code);
    }

    #[test]
    fn do_fork() {
        let engine: InstrumenterEngine = InstrumenterEngine::new(PathBuf::from("sample/dns"));
        let fork = engine.fork().unwrap();
        println!("{:?}", fork);
        let exists = fork.exists();
        fs::remove_file(fork).unwrap(); //remove after test passed to avoid spam of /tmp
        assert!(exists);
    }

    /// This function simply saves some `modified_code` Rust code into /tmp/toz.rs
    /// Format it with `rustfmt` and `ccat` it into stdout
    /// Used only for debugging purposes
    fn export(modified_code: String) {
        let mut file = File::create("/tmp/toz.rs").expect("Unable to create file");
        write!(file, "{}", modified_code).expect("Unable to write data");

        Command::new("rustfmt")
            .arg("/tmp/toz.rs")
            .status()
            .expect("Failed to run rustfmt");
        Command::new("ccat")
            .arg("/tmp/toz.rs")
            .arg("--bg=dark")
            .status()
            .expect("Just install ccat... please");
    }
}
