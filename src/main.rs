extern crate clap;
extern crate rand;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

mod agent;
mod market;


use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use clap::{Arg, App};

use agent::{Agent, Style};
use market::{cda, call};


#[derive(Deserialize)]
struct Config {
    style: Option<Style>,
    cda: Option<bool>,
}

#[derive(Deserialize)]
struct Roles {
    buyers: HashMap<String, u64>,
    sellers: HashMap<String, u64>,
}

#[derive(Deserialize)]
struct Spec {
    assignment: Roles,
    configuration: Config,
}

#[derive(Serialize, Debug)]
struct Features {
    surplus: f64,
    ce_surplus: f64,
    im_surplus: f64,
    em_surplus: f64,
    ce_price: Option<f64>,
}

#[derive(Serialize)]
struct Observation<'a, 'b: 'a> {
    players: &'a mut Vec<Agent<'b>>,
    features: Features,
}

fn main() {
    let matches = App::new("cdasim")
        .version("0.1")
        .author("Erik Brinkman <erik.brinkman@gmail.com>")
        .about("Egtaonline compatable cda simulatior")
        .arg(Arg::with_name("obs")
             .help("Number of observations per spec file to produce.")
             .default_value("1"))
        .arg(Arg::with_name("flush")
             .long("flush")
             .help("Flush stdout after every observation."))
        .after_help("Run an egtaonline style simulation of a simple market. \
                    Takes as input to stdin, lines of json simulation spec \
                    files. Each spec file must have the following structure: \
                    {assignment: {buyers: {[strat]: [count]}, sellers: \
                    {[strat]: [count]}}, configuraion: {cda?: true, style?: \
                    \"Standard\"}}. [count] is an integer for the number of \
                    players playing that strategy. [strat] is a float in \
                    [0, 1] representing the amount of shading, 1 being the \
                    highest. It can be optioanlly suffixed with an underscore \
                    and one of {Standard, Exponential, Shift, Correct}. \
                    Similarly \"style\" can be any of those four to set a \
                    default for agents. \"cda\" indicates if the market is a \
                    CDA or a call market.")
        .get_matches();

    let num_obs: u64 = matches.value_of("obs").unwrap()
        .parse().expect("number of observaions wasn't an integer");
    let flush = matches.is_present("flush");

    let stdin = io::stdin();
    let ihandle = stdin.lock();
    let stdout = io::stdout();
    let mut ohandle = stdout.lock();

    for line in ihandle.lines().map(|l| l.unwrap()) {
        let spec: Spec = serde_json::from_str(&line).unwrap();
        let default_style = spec.configuration.style.unwrap_or(Style::Standard);
        let market = if spec.configuration.cda.unwrap_or(true) { cda } else { call };

        let mut agents = Vec::<Agent>::new();
        for (map, bs) in vec![(&spec.assignment.buyers, true),
                              (&spec.assignment.sellers, false)].iter() {
            for (strat, num) in map.iter() {
                let mut iter = strat.splitn(2, '_');
                let shading: f64 = iter.next().unwrap().parse().expect("couldn't parse strategy");
                let style: Style = match iter.next() {
                    Some(string) => string.parse().expect("strategy style was unknown"),
                    None => default_style,
                };
                for _ in 0..*num {
                    agents.push(Agent::new(*bs, &strat, style, shading));
                }
            }
        }

        for _ in 0..num_obs {
            // Set values and value bids for agents
            let features = run_sim(&mut agents, market);
            serde_json::to_writer(&mut ohandle, &Observation{
                players: &mut agents,
                features: features,
            }).unwrap();
            ohandle.write("\n".as_bytes()).unwrap();
            if flush { ohandle.flush().unwrap() }
        }
    }
}

fn run_sim<'a, M>(mut agents: &mut Vec<Agent<'a>>, market: M) -> Features
    where M: Fn(&mut Vec<Agent<'a>>) -> Option<f64> {
    // Resample
    agents.iter_mut().for_each(Agent::resample);

    // Compute max social welfare
    let ce_price = call(&mut agents);
    agents.iter_mut().for_each(|a| a.ce_traded = a.traded);
    let ce_surplus = agents.iter().fold(0.0, |surp, a| surp + a.utility);

    // Set shading and trade
    agents.iter_mut().for_each(Agent::shade);
    market(&mut agents);

    // Compute features
    let surplus = agents.iter().fold(0.0, |sum, a| sum + a.utility);
    let mut im_surplus = 0.0;
    let mut em_surplus = 0.0;
    match ce_price {
        Some(price) => {
            for agent in agents.iter() {
                if agent.traded && !agent.ce_traded {
                    em_surplus += agent.sign() * (price - agent.value)
                } else if !agent.traded && agent.ce_traded {
                    im_surplus += agent.sign() * (agent.value - price)
                }
            }
        },
        None => em_surplus = ce_surplus - surplus
    };
    Features {
        surplus: surplus,
        ce_surplus: ce_surplus,
        im_surplus: im_surplus,
        em_surplus: em_surplus,
        ce_price: ce_price,
    }
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use super::*;

    const STRAT: &str = "";

    #[test]
    fn test_features() {
        let styles = vec![Style::Standard, Style::Exponential, Style::Shift, Style::Correct];
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let mut agents = Vec::<Agent>::new();
            let num = rng.gen_range(5, 10);
            for buyer in vec![false, true].iter() {
                for _ in 0..num {
                    agents.push(Agent::new(
                        *buyer, &STRAT, *rng.choose(&styles).unwrap(), rng.gen()));
                }
            }

            for _ in 0..100 {
                let features = run_sim(&mut agents, cda);
                let ce_surplus_other = features.surplus + features.im_surplus + features.em_surplus;
                assert!((features.ce_surplus - ce_surplus_other).abs() < 1e-6);
            }
        }
    }
}
