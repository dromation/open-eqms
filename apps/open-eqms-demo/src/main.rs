pub mod asset_model;
pub mod clock;
pub mod crypto;
pub mod ids;
pub mod outcome;
pub mod presentation;
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
        "show-asset" => match ObjectIdArg::required(args.get(1), "show-asset") {
            Ok(asset_id) => match app.show_asset(&asset_id) {
                Ok(report) => {
                    println!("{report}");
                    0
                }
                Err(message) => {
                    eprintln!("{message}");
                    1
                }
            },
            Err(message) => {
                eprintln!("{message}");
                2
            }
        },
        "show-timeline" => match ObjectIdArg::required(args.get(1), "show-timeline") {
            Ok(asset_id) => match app.show_timeline(&asset_id) {
                Ok(report) => {
                    println!("{report}");
                    0
                }
                Err(message) => {
                    eprintln!("{message}");
                    1
                }
            },
            Err(message) => {
                eprintln!("{message}");
                2
            }
        },
        "run-demo" => {
            println!("{}", app.run_demo());
            0
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

    fn required(
        value: Option<&String>,
        command: &str,
    ) -> Result<open_eqms_runtime_contracts::ObjectId, String> {
        let Some(value) = value else {
            return Err(format!("{command} requires <asset-id>"));
        };
        if value.is_empty() {
            return Err(format!("{command} requires a non-empty <asset-id>"));
        }
        Ok(open_eqms_runtime_contracts::ObjectId::new(value))
    }
}

fn print_help() {
    println!("{PRE_ALPHA_BANNER}");
    println!("Commands:");
    println!("  register-asset [--field=value]");
    println!("  record-calibration [--field=value]");
    println!("  show-asset <asset-id>");
    println!("  show-timeline <asset-id>");
    println!("  run-demo");
    println!(
        "Reads that list history will use direct Event/Transaction range APIs and application-layer filtering pending Query Engine completion."
    );
}
