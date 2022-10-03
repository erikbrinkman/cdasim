mod agent;
mod market;

use agent::{Agent, Style};
use clap::Parser;
use market::{Call, Cda, Market};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};

#[derive(Deserialize, Debug)]
struct Config {
    style: Option<Style>,
    cda: Option<bool>,
}

#[derive(Deserialize, Debug)]
struct Roles {
    buyers: HashMap<String, u64>,
    sellers: HashMap<String, u64>,
}

#[derive(Deserialize, Debug)]
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

#[derive(Serialize, Debug)]
struct Observation<'a, 'b: 'a> {
    players: &'a [Agent<'b>],
    features: Features,
}

#[derive(Parser)]
#[clap(version, about)]
/// Run an egtaonline style simulation of a simple market
///
/// Takes as input to stdin, lines of json simulation spec files. Each spec file must have the
/// following structure:
///
/// {
///     assignment: {
///         buyers: {[strat]: [count]},
///         sellers: {[strat]: [count]}
///     },
///     configuraion: {cda?: true, style?: "Standard"}
/// }
///
/// [count] is an integer for the number of players playing that strategy. [strat] is a float in
/// [0, 1] representing the amount of shading, 1 being the highest. It can be optioanlly suffixed
/// with an underscore and one of {Standard, Exponential, Shift, Correct}. Similarly "style" can be
/// any of those four to set a default for agents. "cda" indicates if the market is a CDA or a call
/// market.
struct Args {
    /// Number of observations per spec file to produce
    #[clap(long, value_parser, default_value_t = 1)]
    obs: u64,

    /// Flush stdout after every observation
    #[clap(long, value_parser)]
    flush: bool,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let stdin = io::stdin();
    let ihandle = stdin.lock();
    let stdout = io::stdout();
    let mut ohandle = stdout.lock();

    for line in ihandle.lines() {
        let spec: Spec = serde_json::from_str(&line?)?;
        let default_style = spec.configuration.style.unwrap_or(Style::Standard);
        let mut agents: Vec<Agent> = Vec::new();
        for (map, bs) in [
            (&spec.assignment.buyers, true),
            (&spec.assignment.sellers, false),
        ] {
            for (strat, num) in map {
                let mut iter = strat.splitn(2, '_');
                let shading: f64 = iter
                    .next()
                    .unwrap()
                    .parse()
                    .expect("couldn't parse strategy");
                let style: Style = match iter.next() {
                    Some(string) => string.parse().expect("strategy style was unknown"),
                    None => default_style,
                };
                for _ in 0..*num {
                    agents.push(Agent::new(bs, strat, style, shading));
                }
            }
        }

        if spec.configuration.cda.unwrap_or(true) {
            output_sim(&mut agents, &Cda, &mut ohandle, args.obs, args.flush)?
        } else {
            output_sim(&mut agents, &Call, &mut ohandle, args.obs, args.flush)?
        };
    }
    Ok(())
}

fn output_sim(
    agents: &mut [Agent<'_>],
    market: &impl Market,
    mut out: &mut impl Write,
    num_obs: u64,
    flush: bool,
) -> io::Result<()> {
    for _ in 0..num_obs {
        let features = run_sim(agents, market);
        serde_json::to_writer(
            &mut out,
            &Observation {
                players: agents,
                features,
            },
        )?;
        writeln!(&mut out)?;
        if flush {
            out.flush()?
        }
    }
    Ok(())
}

fn run_sim(agents: &mut [Agent<'_>], market: &impl Market) -> Features {
    // resample
    agents.iter_mut().for_each(Agent::resample);

    // compute max social welfare
    let ce_price = Call.simulate(agents);
    agents.iter_mut().for_each(|a| a.ce_traded = a.traded);
    let ce_surplus = agents.iter().fold(0.0, |surp, a| surp + a.utility);

    // set shading and trade
    agents.iter_mut().for_each(Agent::shade);
    market.simulate(agents);

    // compute features
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
        }
        None => em_surplus = ce_surplus - surplus,
    };

    Features {
        surplus,
        ce_surplus,
        im_surplus,
        em_surplus,
        ce_price,
    }
}

#[cfg(test)]
mod tests {
    use super::{Agent, Args, Cda, Style};
    use clap::CommandFactory;
    use rand::distributions::{Distribution, Uniform};
    use rand::seq::SliceRandom;

    #[test]
    fn test_features() {
        let styles = [
            Style::Standard,
            Style::Exponential,
            Style::Shift,
            Style::Correct,
        ];
        let mut rng = rand::thread_rng();
        let num_dist = Uniform::from(5..10);
        let shade_dist = Uniform::from(0.0..=1.0);
        for _ in 0..100 {
            let mut agents: Vec<Agent> = Vec::new();
            let num = num_dist.sample(&mut rng);
            for buyer in [false, true] {
                for _ in 0..num {
                    agents.push(Agent::new(
                        buyer,
                        "",
                        *styles.choose(&mut rng).unwrap(),
                        shade_dist.sample(&mut rng),
                    ));
                }
            }

            for _ in 0..100 {
                let features = super::run_sim(&mut agents, &Cda);
                let ce_surplus_other = features.surplus + features.im_surplus + features.em_surplus;
                assert!((features.ce_surplus - ce_surplus_other).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn test_cli() {
        Args::command().debug_assert()
    }
}
