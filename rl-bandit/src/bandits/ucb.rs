use crate::bandit::{update_average, Action, Bandit, Reward};
use rand::seq::SliceRandom;

/**
 * Implements a UCB arm
 */
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct UCBArm {
    /// number of trials so far
    n: u64,
    /// average reward
    q: Reward,
}

impl UCBArm {
    pub fn new(n: u64, q: Reward) -> Self {
        Self { n, q }
    }
}

/**
 * Implements a UCB (Upper Confidence Bound) algorithm
 */
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UCB {
    c: f64,
    nb_steps: u64,
    arms: Vec<UCBArm>,
}

impl UCB {
    pub fn new(n: Action, c: f64) -> Self {
        let arms = (0..n).map(|_| UCBArm::new(0, 0.0)).collect();
        Self {
            c,
            nb_steps: 0,
            arms,
        }
    }

    fn ucb_value(&self, arm: &UCBArm) -> f64 {
        if arm.n == 0 {
            return f64::INFINITY;
        }

        if self.nb_steps <= 1 {
            return arm.q;
        }

        arm.q + self.c * ((self.nb_steps as f64).ln() / (arm.n as f64)).sqrt()
    }
}
impl UCB {
    pub fn increment_arm(&mut self) {
        self.arms.push(UCBArm::new(0, 0.0))
    }
    pub fn choose_new(&mut self) -> Action {
        // Create shuffled indices for random tie-breaking
        let mut indices: Vec<usize> = (0..self.arms.len()).collect();
        indices.shuffle(&mut rand::thread_rng());

        let mut best_arm = indices[0];
        let mut best_value = self.ucb_value(&self.arms[indices[0]]);

        for &i in &indices[1..] {
            let value = self.ucb_value(&self.arms[i]);
            if value > best_value {
                best_arm = i;
                best_value = value;
            }
        }

        self.nb_steps += 1;
        self.arms[best_arm].n += 1;
        self.arms[best_arm].q = update_average(self.arms[best_arm].q, 0.0, self.arms[best_arm].n);
        best_arm
    }
}
impl Bandit for UCB {
    fn choose(&self) -> Action {
        // Create shuffled indices for random tie-breaking
        let mut indices: Vec<usize> = (0..self.arms.len()).collect();
        indices.shuffle(&mut rand::thread_rng());

        let mut best_arm = indices[0];
        let mut best_value = self.ucb_value(&self.arms[indices[0]]);

        for &i in &indices[1..] {
            let value = self.ucb_value(&self.arms[i]);
            if value > best_value {
                best_arm = i;
                best_value = value;
            }
        }

        best_arm
    }

    fn update(&mut self, a: Action, r: Reward) {
        self.nb_steps += 1;
        self.arms[a].n += 1;
        self.arms[a].q = update_average(self.arms[a].q, r, self.arms[a].n);
    }

    fn str(&self) -> std::string::String {
        format!("UCB c={:.2}", self.c)
    }
}
