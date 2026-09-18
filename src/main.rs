use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};

use arjev::argent::ArgentClient;
use arjev::jev::{Answer, JevClient, NoulCriteria, Question};
use arjev::plan::Plan;
use arjev::run::{resolve_device, Runner};
use arjev::screen::Screen;

#[derive(Parser)]
#[command(name = "arjev", about = "Run Jev-judged UI verifications through Argent")]
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
    },
    Ask {
        udid: String,
        question: String,
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
    if let Command::Run { plan, .. } = command {
        if let Some(directory) = plan.parent() {
            let _ = dotenvy::from_path(directory.join(".env"));
        }
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
        Command::Run { plan, udid, json } => {
            let plan = Plan::load(&plan)?;
            let udid = match udid {
                Some(udid) => udid,
                None => resolve_device(&argent.list_devices()?, &plan.device)?,
            };
            let report = Runner::new(argent, plan, udid).run(|step| {
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
        Command::Ask { udid, question } => {
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
            let decision = JevClient::from_env()?
                .decide(&serde_json::Value::String(screen.state_text()), &questions)?;
            match decision.answer("ask")? {
                Answer::Noul { noul } => {
                    println!("{noul:.3}");
                    Ok(if *noul >= arjev::plan::default_threshold() {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    })
                }
                other => bail!("Jev answered with {other:?} instead of a noul"),
            }
        }
    }
}
