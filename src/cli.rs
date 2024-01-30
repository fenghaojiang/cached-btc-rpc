use clap::Parser;
use reqwest::Url;
use std::str::FromStr;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(short, long, default_value = "127.0.0.1")]
    pub bind: String,

    #[arg(short, long, default_value = "8124")]
    pub port: u16,

    #[arg(short, long = "endpoint", value_parser = endpoint_parser)]
    pub endpoints: Vec<(String, Url)>,

    #[arg(short, long, default_value = "redis://127.0.0.1:6379", help = "Redis URL")]
    pub redis_url: String,
}

fn endpoint_parser(s: &str) -> Result<(String, Url), String> {
    let part = s.splitn(2, '=').collect::<Vec<_>>();

    if part.len() != 2 {
        return Err(format!("Invalid endpoint format: {}", part[0]));
    }

    let url = Url::from_str(part[1]).map_err(|e| e.to_string())?;
    let name = part[0].to_uppercase();

    Ok((name, url))
}
