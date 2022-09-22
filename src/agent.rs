use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Standard,
    Exponential,
    Shift,
    Correct,
}

#[derive(Debug)]
pub struct Agent<'a> {
    pub buyer: bool,
    strat: &'a str,
    style: Style,
    shading: f64,
    pub value: f64,
    pub bid: f64,
    pub utility: f64,
    pub traded: bool,
    pub ce_traded: bool,
}

impl<'a> Agent<'a> {
    pub fn new(buyer: bool, strat: &'a str, style: Style, shading: f64) -> Agent<'a> {
        Agent {
            buyer,
            strat,
            style,
            shading,
            value: 0.0,
            bid: 0.0,
            utility: 0.0,
            traded: false,
            ce_traded: false,
        }
    }

    pub fn sign(&self) -> f64 {
        if self.buyer {
            1.0
        } else {
            -1.0
        }
    }

    pub fn transact(&mut self, price: f64) {
        self.utility = (self.value - price) * self.sign();
        self.traded = true;
    }

    fn reset(&mut self) {
        self.utility = 0.0;
        self.traded = false;
    }

    pub fn resample(&mut self) {
        self.value = rand::random();
        self.bid = self.value * self.sign();
        self.reset();
    }

    pub fn shade(&mut self) {
        self.bid = match (self.style, self.buyer) {
            (Style::Standard, _) | (Style::Correct, true) => {
                self.value * (self.sign() - self.shading)
            }
            (Style::Correct, false) => (self.value - 1.0) * self.shading - self.value,
            (Style::Exponential, _) => {
                self.sign() * self.value * (-self.sign() * self.shading).exp()
            }
            (Style::Shift, _) => self.sign() * self.value - self.shading,
        };
        self.reset();
    }
}

impl<'a> Serialize for Agent<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("role", if self.buyer { "buyers" } else { "sellers" })?;
        map.serialize_entry("strategy", self.strat)?;
        map.serialize_entry("payoff", &self.utility)?;
        map.end()
    }
}

impl FromStr for Style {
    type Err = String;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        match string {
            "Standard" => Ok(Style::Standard),
            "Exponential" => Ok(Style::Exponential),
            "Shift" => Ok(Style::Shift),
            "Correct" => Ok(Style::Correct),
            _ => Err(format!("unknwon style: \"{}\"", string)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shading_invariant() {
        let strat = "";
        for buyer in [false, true] {
            for style in [
                Style::Standard,
                Style::Exponential,
                Style::Shift,
                Style::Correct,
            ] {
                for shading in (0..11).map(|s| s as f64 / 10.0) {
                    let mut agent = Agent::new(buyer, &strat, style, shading);
                    for _ in 0..100 {
                        agent.reset();
                        agent.shade();
                        assert!(agent.bid <= agent.sign() * agent.value);
                    }
                }
            }
        }
    }

    #[test]
    fn test_inverse_enum() {
        for style in [
            Style::Standard,
            Style::Exponential,
            Style::Shift,
            Style::Correct,
        ] {
            let string = format!("{:?}", style);
            let copy: Style = string.parse().unwrap();
            assert_eq!(copy, style);
        }
    }
}
