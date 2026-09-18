use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

use arjev::argent::ArgentClient;
use arjev::decider::{Decider, DeciderKind, Selection};
use arjev::jev::{Answer, NoulCriteria, Question};
use arjev::plan::Plan;
use arjev::run::{resolve_device, Runner};
use arjev::screen::Screen;

#[derive(Parser)]
#[command(name = "arjev", about = "Run model-judged UI verifications through Argent")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Devices,
    Tools,
    Run {
        plan: PathBuf,
        #[arg(long)]
        udid: Option<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        decider: Option<DeciderKind>,
        #[arg(long)]
        model: Option<String>,
    },
    Ask {
        udid: String,
        question: String,
        #[arg(long)]
        decider: Option<DeciderKind>,
        #[arg(long)]
        model: Option<String>,
    },
}

fn main() -> ExitCode {
    match dispatch() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn load_env(command: &Command) {
    let _ = dotenvy::dotenv();
    let plan = match command {
        Command::Run { plan, .. } => Some(plan),
        _ => None,
    };
    if let Some(directory) = plan.and_then(|plan| plan.parent()) {
        let _ = dotenvy::from_path(directory.join(".env"));
    }
}

fn dispatch() -> Result<ExitCode> {
    let cli = Cli::parse();
    load_env(&cli.command);
    let argent = ArgentClient::discover()?;

    match cli.command {
        Command::Devices => {
            for device in argent.list_devices()? {
                println!(
                    "{:<10} {:<38} {:<24} {}",
                    device.platform, device.udid, device.name, device.state
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Tools => {
            for tool in argent.list_tools()? {
                println!("{}", tool.name);
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Run {
            plan,
            udid,
            json,
            decider,
            model,
        } => {
            let runner = build_runner(argent, &plan, udid, &Selection::resolve(decider, model))?;
            let report = runner.run(|step| {
                if !json {
                    println!("{}", step.line());
                }
            });
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
            Ok(if report.passed() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        Command::Ask {
            udid,
            question,
            decider,
            model,
        } => {
            let screen = Screen::parse(&argent.describe(&udid)?);
            let questions = BTreeMap::from([(
                "ask".to_string(),
                Question::Noul {
                    instructions: question,
                    criteria: NoulCriteria {
                        yes: "The screen matches what the question describes.".to_string(),
                        no: "The screen does not match what the question describes.".to_string(),
                    },
                },
            )]);
            let decider = Decider::new(&Selection::resolve(decider, model))?;
            let decision = decider.decide(
                &serde_json::Value::String(screen.state_text()),
                &questions,
            )?;
            match decision.answer("ask")? {
                Answer::Noul { noul } => {
                    println!("{noul:.3}");
                    Ok(if *noul >= arjev::plan::default_threshold() {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    })
                }
                other => bail!("the decider answered with {other:?} instead of a noul"),
            }
        }
    }
}

fn build_runner(
    argent: ArgentClient,
    plan: &Path,
    udid: Option<String>,
    selection: &Selection,
) -> Result<Runner> {
    let plan = Plan::load(plan)?;
    let udid = match udid {
        Some(udid) => udid,
        None => resolve_device(&argent.list_devices()?, &plan.device)?,
    };
    Ok(Runner::new(argent, plan, udid, selection))
}
