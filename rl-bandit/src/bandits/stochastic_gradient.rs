use crate::bandit::{Bandit, Action, Reward, update_average};

/**
 * Implements an Stochastic Gradient arm
 */
struct SGArm {
    /// action weight
    h: f64,
    /// probability to select the action
    p: f64,
}

impl SGArm {
    pub fn new(h: f64, p: f64) -> Self {
        Self { h:h, p:p }
    }
}

/**
 * Implements an Stochastic Gradient algorithm
 */
pub struct StochasticGradient {
    arms: Vec<SGArm>,
    use_baseline: bool,
    step_size: f64,
    baseline: f64,
    n: u64,
}

impl StochasticGradient {
    pub fn new(n: Action, step_size: f64, use_baseline: bool) -> Self {
        let mut arms = Vec::new();
        for _ in 0..n {
            arms.push(SGArm::new(0., 1./(n as f64)));
        }
        Self { arms:arms, use_baseline:use_baseline, step_size:step_size, baseline:0., n:0 }
    }
}

impl Bandit for StochasticGradient {

    fn choose(&self) -> Action {
        let r = rand::random::<f64>();
        let mut acc = 0.;
        for (i,e) in self.arms.iter().enumerate() {
            acc += e.p;
            if acc >= r {
                return i;
            }
        }
        // println!("r={}\tacc={}",r,acc);
        // assert_eq!(true, false);  // invariant violated (sum(p) = 1)
        return self.arms.len()-1;
    }

    fn update(&mut self, a: Action, r: Reward) {
        assert_eq!(a < self.arms.len(), true);
        // update baseline
        if self.use_baseline {
            self.n += 1;
            self.baseline = update_average(self.baseline, r, self.n)
        }
        // update h
        let mut sum_h = 0.;
        for i in 0..self.arms.len() {
            if i == a {
                self.arms[i].h += self.step_size * (r-self.baseline) * (1.-self.arms[i].p);
            } else {
                self.arms[i].h -= self.step_size * (r-self.baseline) * self.arms[i].p
            }
            sum_h += self.arms[i].h.exp();
        }
        // update p
        for i in 0..self.arms.len() {
            self.arms[i].p = self.arms[i].h.exp()/sum_h;
        }
    }

    fn str(&self) -> std::string::String {
        return format!("Stoc. Grad. α={} {}", self.step_size, if self.use_baseline { "baseline" } else { "" });
    }

}