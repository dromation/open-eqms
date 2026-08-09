pub mod asset_model;
pub mod clock;
pub mod crypto;
pub mod ids;
pub mod outcome;
pub mod scenario;
pub mod storage;

#[cfg(test)]
mod tests;

const PRE_ALPHA_BANNER: &str =
    "Open-EQMS demo: pre-alpha, local, single-process, non-production reference application.";

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args[0] == "--help" || args[0] == "help" {
        print_help();
        return;
    }

    println!("{PRE_ALPHA_BANNER}");

    let mut app = scenario::DemoApp::new();
    let exit_code = match args[0].as_str() {
        "register-asset" => {
            let mut input = scenario::RegisterAssetInput::demo();
            if let Err(message) = input.apply_overrides(&args[1..]) {
                eprintln!("{message}");
                2
            } else {
                let outcome = app.register_asset(input);
                println!("{}", outcome.to_cli_report());
                i32::from(!outcome.is_complete())
            }
        }
        "record-calibration" => {
            let mut input =
                scenario::RecordCalibrationInput::accepted(ObjectIdArg::default_asset());
            if let Err(message) = input.apply_overrides(&args[1..]) {
                eprintln!("{message}");
                2
            } else {
                let outcome = app.record_calibration(input);
                println!("{}", outcome.to_cli_report());
                i32::from(!outcome.is_complete())
            }
        }
        command => {
            eprintln!("unknown command: {command}");
            print_help();
            2
        }
    };

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

struct ObjectIdArg;

impl ObjectIdArg {
    fn default_asset() -> open_eqms_runtime_contracts::ObjectId {
        open_eqms_runtime_contracts::ObjectId::new("asset-0001")
    }
}

fn print_help() {
    println!("{PRE_ALPHA_BANNER}");
    println!("Commands:");
    println!("  register-asset [--field=value]");
    println!("  show-asset <asset-id>      (added in a later VS-001 commit)");
    println!("  show-timeline <asset-id>   (added in a later VS-001 commit)");
    println!("  record-calibration         (added in a later VS-001 commit)");
    println!("  run-demo                   (added in a later VS-001 commit)");
    println!(
        "Reads that list history will use direct Event/Transaction range APIs and application-layer filtering pending Query Engine completion."
    );
}
