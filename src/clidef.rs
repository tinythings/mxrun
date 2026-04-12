use clap::ArgMatches;
use clap::builder::styling;
use clap::{Arg, ArgAction, ColorChoice, Command};
use colored::Colorize;
use std::path::PathBuf;

pub static APPNAME: &str = "xrun";
pub static VERSION: &str = "0.1.0";

pub fn cli() -> Command {
    Command::new(APPNAME)
        .version(VERSION)
        .arg_required_else_help(true)
        .about(format!(
            "{} - {}",
            APPNAME.bright_magenta().bold(),
            "runs one build entry across local and remote targets"
        ))
        .override_usage(format!("{APPNAME} [--add-host|-a <HOST>] [OPTIONS] <COMMAND>"))
        .subcommand(
            Command::new("init")
                .about("Validate and initialise xrun targets from XRUN_CONFIG")
                .styles(styles()),
        )
        .subcommand(
            Command::new("run")
                .about("Run one xrun-aware make entry across configured targets")
                .styles(styles())
                .arg(
                    Arg::new("entry")
                        .help("Build entry to run, such as dev, release, modules, or test")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::new("mirror-results")
                        .long("mirror-results")
                        .action(ArgAction::SetTrue)
                        .help("After successful build, mirror known result roots back to the local xrun directory"),
                )
                .arg(
                    Arg::new("mirror-root")
                        .long("mirror-root")
                        .value_name("DIR")
                        .help("Override the local root directory used for mirrored results; default is ./target/xrun"),
                )
                .arg(
                    Arg::new("wrap-lines")
                        .long("wrap-lines")
                        .action(ArgAction::SetTrue)
                        .help("Wrap log lines in the viewport instead of trimming them with ..."),
                ),
        )
        .next_help_heading("Other")
        .arg(
            Arg::new("add-host")
                .short('a')
                .long("add-host")
                .value_name("HOST")
                .help("Add a remote host to the xrun config using the current user and current project path"),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("Specify an alternative xrun config instead of XRUN_CONFIG"),
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .action(ArgAction::Count)
                .help("Enable debug mode for more verbose output. Increase this flag for greater verbosity."),
        )
        .color(ColorChoice::Always)
        .styles(styles())
}

pub fn entry(am: &ArgMatches) -> String {
    am.subcommand_matches("run")
        .and_then(|sub| sub.get_one::<String>("entry"))
        .cloned()
        .unwrap_or_default()
}

pub fn mirror_results(am: &ArgMatches) -> bool {
    am.subcommand_matches("run")
        .and_then(|sub| sub.get_one::<bool>("mirror-results"))
        .copied()
        .unwrap_or(false)
}

pub fn mirror_root(am: &ArgMatches) -> Option<PathBuf> {
    am.subcommand_matches("run")
        .and_then(|sub| sub.get_one::<String>("mirror-root"))
        .map(PathBuf::from)
}

pub fn wrap_lines(am: &ArgMatches) -> bool {
    am.subcommand_matches("run")
        .and_then(|sub| sub.get_one::<bool>("wrap-lines"))
        .copied()
        .unwrap_or(false)
}

pub fn add_host(am: &ArgMatches) -> Option<String> {
    am.get_one::<String>("add-host").cloned()
}

fn styles() -> styling::Styles {
    styling::Styles::styled()
        .header(styling::AnsiColor::Yellow.on_default())
        .usage(styling::AnsiColor::Yellow.on_default())
        .literal(styling::AnsiColor::BrightGreen.on_default())
        .placeholder(styling::AnsiColor::BrightMagenta.on_default())
}
