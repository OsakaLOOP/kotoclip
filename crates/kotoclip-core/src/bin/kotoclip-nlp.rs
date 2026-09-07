use kotoclip_core::analysis::{AnalysisService, Request, ResourcePaths, Response};
use kotoclip_nlp::model::Register;
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut service = AnalysisService::new(ResourcePaths::development(&root));
    match args.first().map(String::as_str) {
        Some("stdio") => {
            let mut output = io::BufWriter::new(io::stdout().lock());
            for line in io::stdin().lock().lines() {
                let line = line?;
                let response = match serde_json::from_str::<Request>(&line) {
                    Ok(request) => service.dispatch(request),
                    Err(error) => Response {
                        result: None,
                        error: Some(error.to_string()),
                    },
                };
                serde_json::to_writer(&mut output, &response)?;
                writeln!(output)?;
                output.flush()?;
            }
        }
        Some("inspect") => {
            let register = if args.get(1).map(String::as_str) == Some("csj") {
                Register::Csj
            } else {
                Register::Cwj
            };
            let text = args
                .get(2)
                .ok_or("用法：kotoclip-nlp inspect cwj|csj 文本")?
                .clone();
            let response = service.dispatch(Request::Analyze { text, register });
            if let Some(error) = response.error {
                return Err(error.into());
            }
            println!("{}", serde_json::to_string_pretty(&response.result)?);
        }
        Some("repl") => {
            eprintln!("输入日文正文，空行退出。");
            for line in io::stdin().lock().lines() {
                let text = line?;
                if text.is_empty() {
                    break;
                }
                println!(
                    "{}",
                    serde_json::to_string(&service.dispatch(Request::Analyze {
                        text,
                        register: Register::Cwj
                    }))?
                );
            }
        }
        _ => eprintln!("用法：kotoclip-nlp inspect cwj|csj 文本 | repl | stdio"),
    }
    Ok(())
}
