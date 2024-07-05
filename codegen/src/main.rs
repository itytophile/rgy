mod fetcher;
mod format;
mod generator;

use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Fetch {
    #[structopt(short = "u", long = "url")]
    url: Option<String>,
    #[structopt(name = "OUTPUT", parse(from_os_str))]
    output: PathBuf,
}

#[derive(Debug, StructOpt)]
pub struct Generate {
    #[structopt(name = "OPLIST", parse(from_os_str))]
    oplist: PathBuf,
    #[structopt(name = "TEMPLATE", parse(from_os_str))]
    template: PathBuf,
    #[structopt(name = "OUTPUT", parse(from_os_str))]
    output: PathBuf,
}

#[derive(Debug, StructOpt)]
pub enum Opt {
    #[structopt(name = "fetch")]
    Fetch(Fetch),
    #[structopt(name = "generate")]
    Generate(Generate),
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();

    env_logger::init();

    match opt {
        Opt::Fetch(opt) => fetcher::run(&opt),
        Opt::Generate(opt) => generator::run(&opt),
    }
}
