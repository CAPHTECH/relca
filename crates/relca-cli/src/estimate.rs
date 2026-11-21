//! Dedicated local-file boundary; existing CLI usage is unchanged.
use relca_core::estimate::{self, EstimateReport};
use std::{collections::BTreeMap, io::Read};

fn execute(args: &[String]) -> EstimateReport {
    fn dispatch(args: &[String]) -> Result<EstimateReport, &'static str> {
        let [_, model, suffix @ ..] = args else {
            return Err("argument");
        };
        if !super::conform::local_path(model) {
            return Err("argument");
        }
        let mut flags = BTreeMap::new();
        let (pairs, rest) = suffix.as_chunks::<2>();
        if !rest.is_empty() {
            return Err("argument");
        }
        for [key, value] in pairs {
            let valid = match key.as_str() {
                "--log" | "--spec" => super::conform::local_path(value),
                "--configuration" => super::conform::value(value),
                _ => false,
            };
            if !valid || flags.insert(key.as_str(), value.as_str()).is_some() {
                return Err("argument");
            }
        }
        let log = flags.get("--log").ok_or("argument")?;
        let spec = flags.get("--spec").ok_or("argument")?;
        let configuration = flags.get("--configuration").copied();
        let mut src = String::new();
        super::conform::regular_file(model)
            .and_then(|mut f| f.read_to_string(&mut src))
            .map_err(|_| "input")?;
        let log = super::conform::regular_file(log).map_err(|_| "input")?;
        let spec = super::conform::regular_file(spec).map_err(|_| "spec_input")?;
        Ok(estimate::estimate(&src, log, spec, configuration))
    }
    dispatch(args).unwrap_or_else(EstimateReport::invalid)
}

pub fn run(args: &[String]) -> ! {
    let r = execute(args);
    for d in &r.diagnostics {
        super::print_diagnostic(format_args!(""), d);
    }
    println!(
        "{}",
        serde_json::to_string(&r).expect("estimate report serializes")
    );
    std::process::exit(r.verdict.exit_code());
}
