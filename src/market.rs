use rand::seq::SliceRandom;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::Agent;

impl<'a> Ord for Agent<'a> {
    fn cmp(&self, other: &Agent<'a>) -> Ordering {
        self.partial_cmp(other).expect("got nan bids")
    }
}

impl<'a> PartialOrd for Agent<'a> {
    fn partial_cmp(&self, other: &Agent<'a>) -> Option<Ordering> {
        self.bid.partial_cmp(&other.bid)
    }
}

impl<'a> PartialEq for Agent<'a> {
    fn eq(&self, other: &Agent<'a>) -> bool {
        self.bid == other.bid
    }
}

impl<'a> Eq for Agent<'a> {}

pub trait Market {
    fn simulate(&self, agents: &mut [Agent<'_>]) -> Option<f64>;
}

pub struct Cda;

impl Market for Cda {
    fn simulate<'a>(&self, agents: &mut [Agent<'a>]) -> Option<f64> {
        let mut buys = BinaryHeap::<&'a mut Agent<'a>>::new();
        let mut sells = BinaryHeap::<&'a mut Agent<'a>>::new();

        // Random order
        agents.shuffle(&mut rand::thread_rng());

        // Bookkeeping
        let mut avg_price = 0.0;
        let mut num_trans = 0;

        {
            // Block to control scope of closure
            let mut trans = |buy: &mut Agent, sell: &mut Agent, price: f64| {
                buy.transact(price);
                sell.transact(price);
                num_trans += 1;
                avg_price += (price - avg_price) / num_trans as f64;
            };

            for agent in agents.iter_mut() {
                if agent.buyer {
                    if sells.peek().map(|s| -s.bid <= agent.bid).unwrap_or(false) {
                        let seller = sells.pop().unwrap();
                        let price = -seller.bid;
                        trans(agent, seller, price);
                    } else {
                        buys.push(agent);
                    }
                } else if buys.peek().map(|b| -agent.bid <= b.bid).unwrap_or(false) {
                    let buyer = buys.pop().unwrap();
                    let price = buyer.bid;
                    trans(buyer, agent, price);
                } else {
                    sells.push(agent);
                }
            }
        }

        if num_trans > 0 {
            Some(avg_price)
        } else {
            None
        }
    }
}

pub struct Call;

impl Market for Call {
    fn simulate<'a>(&self, agents: &mut [Agent<'a>]) -> Option<f64> {
        let mut buys = Vec::<&'a mut Agent<'a>>::new();
        let mut sells = Vec::<&'a mut Agent<'a>>::new();
        agents
            .iter_mut()
            .for_each(|a| if a.buyer { &mut buys } else { &mut sells }.push(a));
        buys.sort_unstable_by(|a, b| a.cmp(b).reverse());
        sells.sort_unstable_by(|a, b| a.cmp(b).reverse());
        let matched = buys
            .iter()
            .zip(sells.iter())
            .take_while(|(b, s)| -s.bid <= b.bid)
            .count();
        if matched > 0 {
            let price = (buys[matched - 1].bid - sells[matched - 1].bid) / 2.0;
            buys[..matched].iter_mut().for_each(|b| b.transact(price));
            sells[..matched].iter_mut().for_each(|b| b.transact(price));
            Some(price)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Call, Market};
    use crate::{Agent, Style};

    fn truthful(buyer: bool, value: f64) -> Agent<'static> {
        let mut agent = Agent::new(buyer, "", Style::Correct, 0.0);
        agent.value = value;
        agent.shade();
        agent
    }

    #[test]
    fn test_simple_call() {
        let mut agents = [
            truthful(true, 1.0),
            truthful(false, 0.7),
            truthful(true, 0.3),
            truthful(false, 0.0),
        ];
        let price = Call.simulate(&mut agents);
        let [one, two, three, four] = agents;

        assert_eq!(price, Some(0.5)); // NOTE This probably isn't portable
        assert!(one.traded);
        assert!(!two.traded);
        assert!(!three.traded);
        assert!(four.traded);
    }
}
