use crate::bandit::{Bandit, Action, Reward, UpdateType, update_average, update_step_average};
use rand::Rng;

/**
 * Implements an Epsilon-greedy arm
 */
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct EGreedyArm {
    /// number of trials so far
    n: u64,
    /// average reward
    q: Reward
}

impl EGreedyArm {
    pub fn new(n: u64, q: Reward) -> Self {
        Self { n:n, q:q }
    }
}

/**
 * Implements an Epsilon-Greedy algorithm
 */
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EGreedy {
    /// epsilon value (chance that a random value is selected instead of the current best)
    e: f64,
    init_val: f64,
    update_type: UpdateType,
    t: Vec<EGreedyArm>

}

impl EGreedy {
    pub fn new(n: Action, e: f64, initial_val: f64, update_type:UpdateType) -> Self {
        let mut t = Vec::new();
        for _ in 0..n {
            t.push(EGreedyArm::new(0, initial_val));
        }
        Self { e:e, t:t, init_val:initial_val, update_type:update_type }
    }
}

impl Bandit for EGreedy {

    fn choose(&self) -> Action {
        if rand::random::<f64>() < self.e {  // random choice
            return rand::thread_rng().gen_range(0, self.t.len());
        } else {  // greedy choice
            let mut maxi = 0;
            let mut maxi_val = self.t[0].q;
            for (i,e) in self.t.iter().enumerate() {
                if maxi_val < e.q {
                    maxi = i;
                    maxi_val = e.q;
                }
            }
            return maxi;
        }
    }

    fn update(&mut self, a: Action, r: Reward) {
        self.t[a].n += 1;
        match self.update_type {
            UpdateType::Average => self.t[a].q = update_average(self.t[a].q, r, self.t[a].n),
            UpdateType::Nonstationary(u) => self.t[a].q = update_step_average(self.t[a].q, r, u),
        }
        
    }

    fn str(&self) -> std::string::String {
        let u = match self.update_type {
            UpdateType::Average => "avg".to_string(),
            UpdateType::Nonstationary(v) => format!("step={:.2}",v),
        };
        return format!("ε-greedy ε={:.2}, init={:.2}, {}", self.e, self.init_val, u);
    }

}
